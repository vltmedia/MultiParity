use std::collections::HashMap;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::branches::claim_scope;
use crate::error::{ApiError, ApiResult};
use crate::models::{change_from_row, load_change, Change, UserRef, CHANGE_SELECT};
use crate::AppState;

#[derive(Deserialize)]
pub struct FilesQuery {
    prefix: Option<String>,
    tag: Option<Uuid>,
    q: Option<String>,
}

pub async fn list_files(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, Query(q): Query<FilesQuery>) -> ApiResult<Json<Vec<Value>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let mut conn = s.db.acquire().await?;
    let scope = claim_scope(&mut conn, b).await?;
    drop(conn);
    let prefix = q.prefix.as_deref().map(|x| x.trim_matches('/')).filter(|x| !x.is_empty()).map(|x| format!("{x}/"));
    let like = q.q.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(|x| format!("%{}%", x.replace('%', "\\%").replace('_', "\\_")));
    let rows = sqlx::query(
        "SELECT t.path, t.size, t.file_hash, t.change_number,
           cl.created_at AS cl_at, cl.device AS cl_device, u.id AS u_id, u.name AS u_name, u.email AS u_email
         FROM tree_entries t
         LEFT JOIN claims cl ON cl.project_id = $1 AND cl.scope_id = $3 AND cl.path = t.path
         LEFT JOIN users u ON u.id = cl.user_id
         WHERE t.branch_id = $2 AND t.file_hash IS NOT NULL
           AND ($4::text IS NULL OR t.path LIKE $4 || '%')
           AND ($5::text IS NULL OR t.path ILIKE $5)
           AND ($6::uuid IS NULL OR EXISTS (SELECT 1 FROM path_tags pt WHERE pt.project_id = $1 AND pt.tag_id = $6
                 AND (pt.path = t.path OR t.path LIKE pt.path || '/%')))
         ORDER BY t.path LIMIT 20000",
    )
    .bind(p)
    .bind(b)
    .bind(scope)
    .bind(&prefix)
    .bind(&like)
    .bind(q.tag)
    .fetch_all(&s.db)
    .await?;
    let tags = path_tags(&s, p).await?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                let path: String = r.get("path");
                let claim = UserRef::from_row(r, "u_").map(|u| json!({ "user": u, "since": r.get::<DateTime<Utc>,_>("cl_at"), "device": r.get::<Option<String>,_>("cl_device") }));
                let my_tags: Vec<&Value> = tags.iter().filter(|(tp, _)| *tp == path || path.starts_with(&format!("{tp}/"))).map(|(_, t)| t).collect();
                json!({ "path": path, "size": r.get::<i64,_>("size"), "file_hash": r.get::<String,_>("file_hash"),
                        "change": r.get::<i32,_>("change_number"), "kind": mp_core::kind::kind_of(&path), "claim": claim, "tags": my_tags })
            })
            .collect(),
    ))
}

async fn path_tags(s: &AppState, p: Uuid) -> ApiResult<Vec<(String, Value)>> {
    let rows = sqlx::query("SELECT pt.path, t.id, t.name, t.color FROM path_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.project_id = $1")
        .bind(p)
        .fetch_all(&s.db)
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get::<String, _>("path"), json!({ "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "color": r.get::<String,_>("color") })))
        .collect())
}

#[derive(Deserialize)]
pub struct TreeQuery {
    since: Option<i32>,
}

/// What changed on a branch after change `since` (all entries, including deletions, when
/// `since` is 0 or absent). Clients use this to get latest.
pub async fn tree(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, Query(q): Query<TreeQuery>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let head: i32 = sqlx::query_scalar("SELECT head_change FROM branches WHERE id = $1 AND project_id = $2")
        .bind(b)
        .bind(p)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That branch"))?;
    let since = q.since.unwrap_or(0);
    let rows = sqlx::query("SELECT path, file_hash, manifest_hash, size, change_number FROM tree_entries WHERE branch_id = $1 AND change_number > $2 ORDER BY path")
        .bind(b)
        .bind(if since <= 0 { -1 } else { since })
        .fetch_all(&s.db)
        .await?;
    let entries: Vec<Value> = rows
        .iter()
        .map(|r| json!({ "path": r.get::<String,_>("path"), "file_hash": r.get::<Option<String>,_>("file_hash"),
                         "manifest_hash": r.get::<Option<String>,_>("manifest_hash"), "size": r.get::<i64,_>("size"), "change": r.get::<i32,_>("change_number") }))
        .collect();
    Ok(Json(json!({ "head": head, "entries": entries })))
}

#[derive(Deserialize)]
pub struct ChangesQuery {
    branch: Option<Uuid>,
    before: Option<i32>,
    limit: Option<i64>,
}

pub async fn list_changes(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Query(q): Query<ChangesQuery>) -> ApiResult<Json<Vec<Change>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let sql = format!("{CHANGE_SELECT} WHERE c.project_id = $1 AND ($2::uuid IS NULL OR c.branch_id = $2) AND ($3::int IS NULL OR c.number < $3) ORDER BY c.number DESC LIMIT $4");
    let rows = sqlx::query(&sql).bind(p).bind(q.branch).bind(q.before).bind(q.limit.unwrap_or(50).clamp(1, 500)).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(change_from_row).collect()))
}

pub async fn get_change(State(s): State<AppState>, user: CurrentUser, Path((p, n)): Path<(Uuid, i32)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let change = load_change(&s.db, p, n).await?;
    let files: Vec<Value> = sqlx::query("SELECT path, op, size FROM change_files WHERE project_id = $1 AND change_number = $2 ORDER BY path")
        .bind(p)
        .bind(n)
        .fetch_all(&s.db)
        .await?
        .iter()
        .map(|r| json!({ "path": r.get::<String,_>("path"), "op": r.get::<String,_>("op"), "size": r.get::<i64,_>("size") }))
        .collect();
    let mut v = serde_json::to_value(change).unwrap_or_default();
    v["files"] = json!(files);
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    branch: Uuid,
    prefix: Option<String>,
    limit: Option<i64>,
}

/// The files-by-changes grid: the last `limit` changes on a branch and, for each file touched
/// by them, which of those changes touched it.
pub async fn history(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Query(q): Query<HistoryQuery>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let limit = q.limit.unwrap_or(10).clamp(1, 60);
    let numbers: Vec<i32> = sqlx::query_scalar("SELECT number FROM changes WHERE project_id = $1 AND branch_id = $2 ORDER BY number DESC LIMIT $3")
        .bind(p)
        .bind(q.branch)
        .bind(limit)
        .fetch_all(&s.db)
        .await?;
    let prefix = q.prefix.as_deref().map(|x| x.trim_matches('/')).filter(|x| !x.is_empty()).map(|x| format!("{x}/"));
    let rows = sqlx::query("SELECT path, change_number FROM change_files WHERE project_id = $1 AND change_number = ANY($2) AND ($3::text IS NULL OR path LIKE $3 || '%') ORDER BY path, change_number")
        .bind(p)
        .bind(&numbers)
        .bind(&prefix)
        .fetch_all(&s.db)
        .await?;
    let mut files: Vec<(String, Vec<i32>)> = Vec::new();
    let mut idx: HashMap<String, usize> = HashMap::new();
    for r in rows {
        let path: String = r.get("path");
        let n: i32 = r.get("change_number");
        let i = *idx.entry(path.clone()).or_insert_with(|| {
            files.push((path.clone(), Vec::new()));
            files.len() - 1
        });
        files[i].1.push(n);
    }
    let mut numbers = numbers;
    numbers.reverse();
    Ok(Json(json!({
        "changes": numbers,
        "files": files.into_iter().take(500).map(|(path, changes)| json!({ "path": path, "changes": changes })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    path: String,
    branch: Uuid,
}

/// Stream one file's current content (web UI download button and previews).
pub async fn download(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Query(q): Query<DownloadQuery>) -> ApiResult<Response> {
    access::project(&s, &user, p, Level::Read).await?;
    let r = sqlx::query("SELECT t.manifest_hash, m.chunks FROM tree_entries t JOIN manifests m ON m.hash = t.manifest_hash WHERE t.branch_id = $1 AND t.path = $2")
        .bind(q.branch)
        .bind(&q.path)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That file"))?;
    let chunks: Vec<mp_core::ChunkRef> = serde_json::from_value(r.get("chunks")).map_err(|e| anyhow::anyhow!("bad manifest: {e}"))?;
    let store = s.store.clone();
    let stream = futures::stream::iter(chunks).then(move |c| {
        let store = store.clone();
        async move {
            let data = store.get_chunk(&c.hash).await.map_err(std::io::Error::other)?;
            mp_core::decompress(&data, c.len as usize).map(bytes::Bytes::from).map_err(std::io::Error::other)
        }
    });
    let name = q.path.rsplit('/').next().unwrap_or("file").replace('"', "");
    Ok((
        [(header::CONTENT_TYPE, "application/octet-stream".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))],
        Body::from_stream(stream),
    )
        .into_response())
}
