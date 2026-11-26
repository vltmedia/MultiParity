use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::PublishEntry;
use crate::routes::sync::check_entries;
use crate::AppState;

fn draft_json(r: &sqlx::postgres::PgRow) -> Value {
    json!({ "id": r.get::<Uuid,_>("id"), "title": r.get::<String,_>("title"), "branch_id": r.get::<Uuid,_>("branch_id"),
            "created_at": r.get::<DateTime<Utc>,_>("created_at"), "file_count": r.get::<i32,_>("file_count"), "size": r.get::<i64,_>("size") })
}

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let rows = sqlx::query("SELECT id, title, branch_id, created_at, file_count, size FROM drafts WHERE project_id = $1 AND user_id = $2 ORDER BY created_at DESC")
        .bind(p)
        .bind(user.id)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows.iter().map(draft_json).collect()))
}

#[derive(Deserialize)]
pub struct CreateDraft {
    branch: Uuid,
    title: String,
    entries: Vec<PublishEntry>,
}

/// Save unfinished work on the server. Only its author sees it.
pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<CreateDraft>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let title = b.title.trim();
    if title.is_empty() {
        return Err(ApiError::invalid("Give the draft a short description."));
    }
    if b.entries.is_empty() {
        return Err(ApiError::invalid("There is nothing to save in a draft."));
    }
    let mut tx = s.db.begin().await?;
    let checked = check_entries(&s, &mut tx, &acc.storage, &b.entries).await?;
    let size: i64 = checked.iter().map(|(_, e)| e.size).sum();
    let row = sqlx::query("INSERT INTO drafts (project_id, branch_id, user_id, title, entries, file_count, size) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id, title, branch_id, created_at, file_count, size")
        .bind(p)
        .bind(b.branch)
        .bind(user.id)
        .bind(title)
        .bind(serde_json::to_value(&b.entries).unwrap_or_default())
        .bind(checked.len() as i32)
        .bind(size)
        .fetch_one(&mut *tx)
        .await?;
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "draft.saved", json!({ "title": title, "files": checked.len() })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(draft_json(&row)))
}

pub async fn get(State(s): State<AppState>, user: CurrentUser, Path((p, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let r = sqlx::query("SELECT id, title, branch_id, created_at, file_count, size, entries FROM drafts WHERE id = $1 AND project_id = $2 AND user_id = $3")
        .bind(id)
        .bind(p)
        .bind(user.id)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That draft"))?;
    let mut v = draft_json(&r);
    v["entries"] = r.get("entries");
    Ok(Json(v))
}

pub async fn remove(State(s): State<AppState>, user: CurrentUser, Path((p, id)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let n = sqlx::query("DELETE FROM drafts WHERE id = $1 AND project_id = $2 AND user_id = $3").bind(id).bind(p).bind(user.id).execute(&s.db).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::not_found("That draft"));
    }
    Ok(Json(json!({})))
}
