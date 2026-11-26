use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::branches::{self, load_branch, lock_project, MergeReport};
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{branch_from_row, load_change, Branch, BRANCH_SELECT};
use crate::AppState;

pub async fn load_branch_json(s: &AppState, id: Uuid) -> ApiResult<Branch> {
    let sql = format!("{BRANCH_SELECT} WHERE b.id = $1");
    Ok(branch_from_row(&sqlx::query(&sql).bind(id).fetch_one(&s.db).await?))
}

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Branch>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let sql = format!("{BRANCH_SELECT} WHERE b.project_id = $1 ORDER BY b.parent_id IS NOT NULL, b.archived, b.name");
    Ok(Json(sqlx::query(&sql).bind(p).fetch_all(&s.db).await?.iter().map(branch_from_row).collect()))
}

#[derive(Deserialize)]
pub struct CreateBranch {
    name: String,
    from: Uuid,
    shares_claims: Option<bool>,
    protected: Option<bool>,
}

/// New branch: copies the parent's tree rows (pointers to stored content, no data copied) and
/// records the parent's versions as the base.
pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<CreateBranch>) -> ApiResult<Json<Branch>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let name = b.name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(ApiError::invalid("Give the branch a name of up to 80 characters."));
    }
    if b.protected.unwrap_or(false) && acc.level < Level::Admin {
        return Err(ApiError::forbidden("Only admins can create protected branches."));
    }
    let mut tx = s.db.begin().await?;
    lock_project(&mut tx, p).await?;
    let parent = load_branch(&mut tx, p, b.from).await?;
    if parent.archived {
        return Err(ApiError::conflict("That branch is archived. Pick another branch to start from."));
    }
    let id: Uuid = sqlx::query_scalar("INSERT INTO branches (project_id, name, parent_id, shares_claims, protected, head_change, last_merged_change, created_by) VALUES ($1,$2,$3,$4,$5,$6,$6,$7) RETURNING id")
        .bind(p)
        .bind(name)
        .bind(parent.id)
        .bind(b.shares_claims.unwrap_or(true))
        .bind(b.protected.unwrap_or(false))
        .bind(parent.head_change)
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict(format!("This project already has a branch called {name}.")),
            err => err,
        })?;
    sqlx::query(
        "INSERT INTO tree_entries (branch_id, path, file_hash, manifest_hash, size, change_number, base_file_hash, base_manifest_hash, base_size)
         SELECT $1, path, file_hash, manifest_hash, size, change_number, file_hash, manifest_hash, size FROM tree_entries WHERE branch_id = $2",
    )
    .bind(id)
    .bind(parent.id)
    .execute(&mut *tx)
    .await?;
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "branch.created", json!({ "branch_id": id, "name": name, "from": parent.name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    load_branch_json(&s, id).await.map(Json)
}

#[derive(Deserialize, Default)]
pub struct UpdateBody {
    #[serde(default)]
    resolutions: HashMap<String, String>,
}

pub async fn update(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, body: Option<Json<UpdateBody>>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let mut tx = s.db.begin().await?;
    lock_project(&mut tx, p).await?;
    let child = load_branch(&mut tx, p, b).await?;
    let (report, change) = branches::update_from_parent(&s, &mut tx, &child, &body.resolutions, true, user.id).await?;
    if !report.conflicts.is_empty() {
        tx.rollback().await?;
        return Ok(Json(report_json(&report, None)));
    }
    let ev = match change {
        Some(n) => Some(events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "change.published", json!({ "number": n, "branch_id": b, "kind": "update" })).await?),
        None => None,
    };
    tx.commit().await?;
    if let Some(ev) = ev {
        events::publish_saved(&s.db, &s.events, ev).await;
    }
    let change = match change {
        Some(n) => Some(load_change(&s.db, p, n).await?),
        None => None,
    };
    let mut report = report;
    report.up_to_date = true;
    Ok(Json(report_json(&report, change)))
}

/// The update result is the merge report itself (as documented), plus the change it made.
fn report_json(report: &MergeReport, change: Option<crate::models::Change>) -> Value {
    let mut v = serde_json::to_value(report).unwrap_or_default();
    v["change"] = serde_json::to_value(change).unwrap_or(Value::Null);
    v
}

pub async fn merge_preview(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>) -> ApiResult<Json<MergeReport>> {
    access::project(&s, &user, p, Level::Read).await?;
    let mut tx = s.db.begin().await?;
    let child = load_branch(&mut tx, p, b).await?;
    let (report, _) = branches::update_from_parent(&s, &mut tx, &child, &HashMap::new(), false, user.id).await?;
    tx.rollback().await?;
    Ok(Json(report))
}

#[derive(Deserialize)]
pub struct MergeBody {
    message: Option<String>,
}

pub async fn merge(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, Json(body): Json<MergeBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let mut tx = s.db.begin().await?;
    lock_project(&mut tx, p).await?;
    let child = load_branch(&mut tx, p, b).await?;
    let parent_id = child.parent_id.ok_or_else(|| ApiError::invalid("Main has no parent branch to merge into."))?;
    let parent = load_branch(&mut tx, p, parent_id).await?;
    if parent.protected && acc.level < Level::Admin {
        return Err(ApiError::forbidden(format!("{} is protected. Ask an admin of this project to merge.", parent.name)));
    }
    let message = body.message.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| format!("Merged {} into {}", child.name, parent.name));
    let n = branches::merge_into_parent(&mut tx, &child, &message, user.id).await?;
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "change.merged", json!({ "number": n, "from": child.name, "into": parent.name, "branch_id": parent.id })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "change": load_change(&s.db, p, n).await? })))
}

pub async fn archive(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Branch>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let mut tx = s.db.begin().await?;
    let br = load_branch(&mut tx, p, b).await?;
    if br.parent_id.is_none() {
        return Err(ApiError::invalid("Main cannot be archived."));
    }
    sqlx::query("UPDATE branches SET archived = TRUE WHERE id = $1").bind(b).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM claims WHERE branch_id = $1").bind(b).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "branch.archived", json!({ "branch_id": b, "name": br.name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    load_branch_json(&s, b).await.map(Json)
}
