use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{tag_from_row, Tag};
use crate::AppState;

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Tag>>> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    let rows = sqlx::query("SELECT id, org_id, project_id, name, color, group_name FROM tags WHERE org_id = $1 AND (project_id IS NULL OR project_id = $2) ORDER BY group_name NULLS LAST, name")
        .bind(acc.org_id)
        .bind(p)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows.iter().map(tag_from_row).collect()))
}

#[derive(Deserialize)]
pub struct CreateTag {
    name: String,
    color: Option<String>,
    group: Option<String>,
    /// When false (default for admins), the tag is shared across the organization.
    project_only: Option<bool>,
}

pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<CreateTag>) -> ApiResult<Json<Tag>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let name = b.name.trim();
    if name.is_empty() || name.len() > 40 {
        return Err(ApiError::invalid("Give the tag a name of up to 40 characters."));
    }
    let color = b.color.filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())).unwrap_or_else(|| "#7a3cff".into());
    // Members create project tags; admins may create organization-wide ones.
    let org_wide = acc.level >= Level::Admin && !b.project_only.unwrap_or(false);
    let row = sqlx::query("INSERT INTO tags (org_id, project_id, name, color, group_name) VALUES ($1,$2,$3,$4,$5) RETURNING id, org_id, project_id, name, color, group_name")
        .bind(acc.org_id)
        .bind(if org_wide { None } else { Some(p) })
        .bind(name)
        .bind(color)
        .bind(b.group.as_deref().map(str::trim).filter(|g| !g.is_empty()))
        .fetch_one(&s.db)
        .await
        .map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict(format!("A tag called {name} already exists.")),
            err => err,
        })?;
    Ok(Json(tag_from_row(&row)))
}

#[derive(Deserialize)]
pub struct ApplyBody {
    tag: Uuid,
    paths: Vec<String>,
}

pub async fn apply(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<ApplyBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let tag = check_tag(&s, acc.org_id, p, b.tag).await?;
    let paths: Vec<String> = b.paths.iter().filter_map(|x| mp_core::paths::normalize(x)).collect();
    let mut tx = s.db.begin().await?;
    // A tag on a folder applies to what is inside. If the tag is in a group, applying it
    // removes the group's other tags from the same paths (status-like tags).
    if let Some(group) = &tag.group {
        sqlx::query("DELETE FROM path_tags pt USING tags t WHERE pt.tag_id = t.id AND pt.project_id = $1 AND pt.path = ANY($2) AND t.group_name = $3 AND t.id <> $4")
            .bind(p)
            .bind(&paths)
            .bind(group)
            .bind(tag.id)
            .execute(&mut *tx)
            .await?;
    }
    for path in &paths {
        sqlx::query("INSERT INTO path_tags (project_id, path, tag_id, created_by) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
            .bind(p)
            .bind(path)
            .bind(tag.id)
            .bind(user.id)
            .execute(&mut *tx)
            .await?;
    }
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "tag.applied", json!({ "tag": tag.name, "tag_id": tag.id, "paths": paths })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({})))
}

pub async fn remove(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<ApplyBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let tag = check_tag(&s, acc.org_id, p, b.tag).await?;
    sqlx::query("DELETE FROM path_tags WHERE project_id = $1 AND tag_id = $2 AND path = ANY($3)")
        .bind(p)
        .bind(tag.id)
        .bind(&b.paths)
        .execute(&s.db)
        .await?;
    events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), "tag.removed", json!({ "tag": tag.name, "tag_id": tag.id, "paths": b.paths })).await;
    Ok(Json(json!({})))
}

async fn check_tag(s: &AppState, org: Uuid, p: Uuid, tag: Uuid) -> ApiResult<Tag> {
    let row = sqlx::query("SELECT id, org_id, project_id, name, color, group_name FROM tags WHERE id = $1 AND org_id = $2 AND (project_id IS NULL OR project_id = $3)")
        .bind(tag)
        .bind(org)
        .bind(p)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That tag"))?;
    Ok(tag_from_row(&row))
}
