//! Exporting a project (Admin > Projects > Export…): start one, follow it, download its .zip.
//! The work itself is in crate::export.

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::access;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::export;
use crate::util::{hash_token, new_token};
use crate::AppState;

/// Who may export a project: owners and admins of its organization (and server admins).
async fn project_admin(s: &AppState, user: &CurrentUser, p: Uuid) -> ApiResult<access::ProjectAccess> {
    let acc = access::project(s, user, p, access::Level::Read).await?;
    access::org_admin(s, user, acc.org_id).await?;
    Ok(acc)
}

fn export_json(r: &PgRow) -> Value {
    json!({
        "id": r.get::<Uuid,_>("id"), "project_id": r.get::<Uuid,_>("project_id"), "branch_id": r.get::<Uuid,_>("branch_id"),
        "branch_name": r.try_get::<String,_>("branch_name").ok(), "change": r.get::<i32,_>("change_number"),
        "target": r.get::<String,_>("target"), "format": r.get::<String,_>("format"), "name": r.get::<String,_>("name"),
        "archive_after": r.get::<bool,_>("archive_after"), "status": r.get::<String,_>("status"),
        "files_total": r.get::<i32,_>("files_total"), "files_done": r.get::<i32,_>("files_done"),
        "bytes_total": r.get::<i64,_>("bytes_total"), "bytes_done": r.get::<i64,_>("bytes_done"),
        "error": r.get::<Option<String>,_>("error"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
        "finished_at": r.get::<Option<DateTime<Utc>>,_>("finished_at"),
        "created_by": crate::models::UserRef::from_row(r, "u_"),
        "downloadable": r.get::<String,_>("target") == "server" && r.get::<String,_>("format") == "zip" && r.get::<String,_>("status") == "done",
    })
}

const SELECT: &str = "SELECT e.*, b.name AS branch_name, u.id AS u_id, u.name AS u_name, u.email AS u_email
     FROM exports e JOIN branches b ON b.id = e.branch_id LEFT JOIN users u ON u.id = e.created_by";

pub async fn targets(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::admin_orgs(&s, &user).await?;
    Ok(Json(export::targets_json()))
}

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    project_admin(&s, &user, p).await?;
    let rows = sqlx::query(&format!("{SELECT} WHERE e.project_id = $1 ORDER BY e.created_at DESC LIMIT 50")).bind(p).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(export_json).collect()))
}

#[derive(Deserialize)]
pub struct CreateExport {
    /// Default: the project's main branch.
    branch: Option<Uuid>,
    /// "server" or a name from MP_EXPORT_TARGETS.
    target: String,
    /// "zip" or "files".
    format: String,
    #[serde(default)]
    archive_after: bool,
}

pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<CreateExport>) -> ApiResult<Json<Value>> {
    project_admin(&s, &user, p).await?;
    if b.format != "zip" && b.format != "files" {
        return Err(ApiError::invalid("Choose zip or files."));
    }
    if !export::targets().iter().any(|(n, _)| *n == b.target) {
        return Err(ApiError::invalid("That export target is not set up on this server."));
    }
    let busy: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM exports WHERE project_id = $1 AND status IN ('queued', 'running'))").bind(p).fetch_one(&s.db).await?;
    if busy {
        return Err(ApiError::conflict("An export of this project is already running. Wait for it to finish."));
    }
    let branch = match b.branch {
        Some(id) => sqlx::query("SELECT id, name, head_change FROM branches WHERE id = $1 AND project_id = $2").bind(id).bind(p).fetch_optional(&s.db).await?,
        None => sqlx::query("SELECT id, name, head_change FROM branches WHERE project_id = $1 AND parent_id IS NULL ORDER BY created_at LIMIT 1").bind(p).fetch_optional(&s.db).await?,
    }
    .ok_or_else(|| ApiError::not_found("That branch"))?;
    let slug: String = sqlx::query_scalar("SELECT slug FROM projects WHERE id = $1").bind(p).fetch_one(&s.db).await?;
    let clean = |t: &str| t.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect::<String>();
    let change: i32 = branch.get("head_change");
    let name = format!("{}-{}-change{}-{}", clean(&slug), clean(&branch.get::<String, _>("name")).to_lowercase(), change, Utc::now().format("%Y%m%d-%H%M"));
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO exports (project_id, branch_id, change_number, target, format, name, archive_after, created_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id",
    )
    .bind(p)
    .bind(branch.get::<Uuid, _>("id"))
    .bind(change)
    .bind(&b.target)
    .bind(&b.format)
    .bind(&name)
    .bind(b.archive_after)
    .bind(user.id)
    .fetch_one(&s.db)
    .await?;
    export::spawn(s.clone(), id);
    let r = sqlx::query(&format!("{SELECT} WHERE e.id = $1")).bind(id).fetch_one(&s.db).await?;
    Ok(Json(export_json(&r)))
}

/// A link that downloads a finished .zip once, for ten minutes (big files go straight from the
/// server to the browser's downloads, not through the page's memory).
pub async fn link(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let r = sqlx::query("SELECT project_id, target, format, status FROM exports WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That export"))?;
    project_admin(&s, &user, r.get("project_id")).await?;
    if r.get::<String, _>("target") != "server" || r.get::<String, _>("format") != "zip" || r.get::<String, _>("status") != "done" {
        return Err(ApiError::invalid("Only a finished .zip on this server can be downloaded."));
    }
    let token = new_token();
    sqlx::query("UPDATE exports SET link_hash = $2, link_expires = now() + interval '10 minutes' WHERE id = $1").bind(id).bind(hash_token(&token)).execute(&s.db).await?;
    Ok(Json(json!({ "url": format!("/api/exports/{id}/zip?token={token}") })))
}

#[derive(Deserialize)]
pub struct LinkQuery {
    token: String,
}

pub async fn download(State(s): State<AppState>, Path(id): Path<Uuid>, Query(q): Query<LinkQuery>) -> ApiResult<Response> {
    let r = sqlx::query("UPDATE exports SET link_hash = NULL WHERE id = $1 AND link_hash = $2 AND link_expires > now() RETURNING name")
        .bind(id)
        .bind(hash_token(&q.token))
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::new(StatusCode::GONE, "link_expired", "This download link has been used or has expired. Get a new one from Admin > Projects."))?;
    let name: String = r.get("name");
    let path = export::zip_on_disk(&name);
    let f = tokio::fs::File::open(&path).await.map_err(|_| ApiError::not_found("That .zip (it may have been removed from the server)"))?;
    let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
    let body = axum::body::Body::from_stream(tokio_util::io::ReaderStream::with_capacity(f, 1 << 20));
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (header::CONTENT_LENGTH, len.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}.zip\"")),
        ],
        body,
    )
        .into_response())
}
