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
    let files: Vec<Value> = sqlx::query("SELECT path, op, size, file_hash FROM change_files WHERE project_id = $1 AND change_number = $2 ORDER BY path")
        .bind(p)
        .bind(n)
        .fetch_all(&s.db)
        .await?
        .iter()
        .map(|r| json!({ "path": r.get::<String,_>("path"), "op": r.get::<String,_>("op"), "size": r.get::<i64,_>("size"),
                         "file_hash": r.get::<Option<String>,_>("file_hash") }))
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
pub struct VersionsQuery {
    path: String,
    branch: Uuid,
}

/// Every published version of one file, newest first: each change that added, edited or
/// deleted it, on any branch. `current` marks the version the branch has now.
pub async fn versions(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Query(q): Query<VersionsQuery>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let path = mp_core::paths::normalize(&q.path).ok_or_else(|| ApiError::invalid("That is not a valid path inside a project."))?;
    let now: Option<String> = sqlx::query_scalar("SELECT file_hash FROM tree_entries WHERE branch_id = $1 AND path = $2")
        .bind(q.branch)
        .bind(&path)
        .fetch_optional(&s.db)
        .await?
        .flatten();
    let rows = sqlx::query(
        "SELECT cf.change_number, cf.op, cf.size, cf.file_hash, cf.manifest_hash, c.message, c.created_at, c.kind, c.branch_id, b.name AS branch_name,
                u.id AS u_id, u.name AS u_name, u.email AS u_email
         FROM change_files cf
         JOIN changes c ON c.project_id = cf.project_id AND c.number = cf.change_number
         JOIN branches b ON b.id = c.branch_id
         LEFT JOIN users u ON u.id = c.author_id
         WHERE cf.project_id = $1 AND cf.path = $2
         ORDER BY cf.change_number DESC LIMIT 500",
    )
    .bind(p)
    .bind(&path)
    .fetch_all(&s.db)
    .await?;
    let mut marked = false;
    let list: Vec<Value> = rows
        .iter()
        .map(|r| {
            let hash: Option<String> = r.get("file_hash");
            let branch: Uuid = r.get("branch_id");
            let current = !marked && hash.is_some() && hash == now && branch == q.branch;
            marked |= current;
            json!({
                "change": r.get::<i32,_>("change_number"), "op": r.get::<String,_>("op"), "size": r.get::<i64,_>("size"),
                "file_hash": hash, "manifest_hash": r.get::<Option<String>,_>("manifest_hash"), "message": r.get::<String,_>("message"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
                "kind": r.get::<String,_>("kind"), "branch_id": branch, "branch_name": r.get::<String,_>("branch_name"),
                "author": UserRef::from_row(r, "u_"), "current": current,
            })
        })
        .collect();
    Ok(Json(json!({ "path": path, "versions": list })))
}

#[derive(Deserialize)]
pub struct RestoreBody {
    path: String,
    change: i32,
    message: Option<String>,
}

/// Make an older version of a file the latest on a branch, as a new change. Nothing is lost:
/// the version it replaces stays in the history.
pub async fn restore_version(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, Json(body): Json<RestoreBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let path = mp_core::paths::normalize(&body.path).ok_or_else(|| ApiError::invalid("That is not a valid path inside a project."))?;
    let mut tx = s.db.begin().await?;
    crate::branches::lock_project(&mut tx, p).await?;
    let branch = crate::branches::load_branch(&mut tx, p, b).await?;
    if branch.archived {
        return Err(ApiError::conflict("This branch is archived. Switch to another branch to bring a version back."));
    }
    if branch.protected && acc.level < Level::Admin {
        return Err(ApiError::forbidden(format!("{} is protected: work arrives by merging a branch into it.", branch.name)));
    }
    let old = sqlx::query("SELECT op, file_hash, manifest_hash, size FROM change_files WHERE project_id = $1 AND change_number = $2 AND path = $3")
        .bind(p)
        .bind(body.change)
        .bind(&path)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::not_found(&format!("A version of {path} in change {}", body.change)))?;
    let entry = crate::branches::Entry { file_hash: old.get("file_hash"), manifest_hash: old.get("manifest_hash"), size: old.get("size") };
    let Some(manifest) = entry.manifest_hash.clone() else {
        return Err(ApiError::invalid(format!("Change {} deleted {path}, so there is no file to bring back from it. Pick an earlier version.", body.change)));
    };
    let now: Option<String> = sqlx::query_scalar("SELECT file_hash FROM tree_entries WHERE branch_id = $1 AND path = $2")
        .bind(b)
        .bind(&path)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
    if now == entry.file_hash {
        return Err(ApiError::invalid(format!("The version from change {} is already the latest on {}.", body.change, branch.name)));
    }
    let scope = crate::branches::claim_scope(&mut tx, b).await?;
    let sql = format!("{} WHERE cl.project_id = $1 AND cl.scope_id = $2 AND cl.path = $3 AND cl.user_id <> $4", crate::models::CLAIM_SELECT);
    if let Some(r) = sqlx::query(&sql).bind(p).bind(scope).bind(&path).bind(user.id).fetch_optional(&mut *tx).await? {
        let c = crate::models::claim_from_row(&r, false);
        let who = c.user.as_ref().map(|u| u.name.clone()).unwrap_or_else(|| "someone else".into());
        return Err(ApiError::conflict(format!("{path} is claimed by {who}. Ask them to release it first.")).with_details(json!({ "claimed": [c] })));
    }
    // The old version's parts must be where the project keeps its data now (a storage move may
    // have left some only in the old location).
    let chunks_json: Value = sqlx::query_scalar("SELECT chunks FROM manifests WHERE hash = $1").bind(&manifest).fetch_one(&mut *tx).await?;
    let chunks: Vec<mp_core::ChunkRef> = serde_json::from_value(chunks_json).map_err(|e| anyhow::anyhow!("bad manifest: {e}"))?;
    let mut hashes: Vec<String> = chunks.into_iter().map(|c| c.hash).collect();
    hashes.sort();
    hashes.dedup();
    if !crate::chunks::ensure_current(&s, &mut tx, &acc.storage, &hashes).await?.is_empty() {
        return Err(ApiError::conflict("Some of that version's file data is missing from storage. Ask an admin to run a storage check (Admin > Fixing)."));
    }
    let message = body.message.as_deref().map(str::trim).filter(|m| !m.is_empty()).map(str::to_string).unwrap_or_else(|| format!("Brought back {} as of change {}", path.rsplit('/').next().unwrap_or(&path), body.change));
    let n = crate::branches::take_change_number(&mut tx, p).await?;
    let op = if now.is_some() { "edit" } else { "add" };
    crate::branches::upsert_entry(&mut tx, b, &path, &entry, n, None).await?;
    crate::branches::record_change_file(&mut tx, p, n, &path, op, &entry).await?;
    sqlx::query("INSERT INTO changes (project_id, number, branch_id, author_id, message, kind, file_count, size) VALUES ($1,$2,$3,$4,$5,'publish',1,$6)")
        .bind(p)
        .bind(n)
        .bind(b)
        .bind(user.id)
        .bind(&message)
        .bind(entry.size)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE branches SET head_change = $2 WHERE id = $1").bind(b).bind(n).execute(&mut *tx).await?;
    let ev = crate::events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "change.published", json!({ "number": n, "branch_id": b, "message": message, "files": 1, "restored": { "path": path, "from": body.change } })).await?;
    tx.commit().await?;
    crate::events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "change": load_change(&s.db, p, n).await? })))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    path: String,
    branch: Uuid,
    /// An older version: the change that published it.
    change: Option<i32>,
}

/// Stream one file's content (web UI download button and previews): the branch's current
/// version, or the one published in `change`.
pub async fn download(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Query(q): Query<DownloadQuery>) -> ApiResult<Response> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    let r = match q.change {
        Some(n) => sqlx::query("SELECT m.chunks FROM change_files cf JOIN manifests m ON m.hash = cf.manifest_hash WHERE cf.project_id = $1 AND cf.change_number = $2 AND cf.path = $3")
            .bind(p)
            .bind(n)
            .bind(&q.path)
            .fetch_optional(&s.db)
            .await?
            .ok_or_else(|| ApiError::not_found(&format!("That file in change {n}")))?,
        None => sqlx::query("SELECT m.chunks FROM tree_entries t JOIN manifests m ON m.hash = t.manifest_hash WHERE t.branch_id = $1 AND t.path = $2")
            .bind(q.branch)
            .bind(&q.path)
            .fetch_optional(&s.db)
            .await?
            .ok_or_else(|| ApiError::not_found("That file"))?,
    };
    let chunks: Vec<mp_core::ChunkRef> = serde_json::from_value(r.get("chunks")).map_err(|e| anyhow::anyhow!("bad manifest: {e}"))?;
    let (state, ps) = (s.clone(), acc.storage.clone());
    let stream = futures::stream::iter(chunks).then(move |c| {
        let (state, ps) = (state.clone(), ps.clone());
        async move {
            let data = crate::chunks::get(&state, &ps, &c.hash).await.map_err(|e| std::io::Error::other(e.message))?;
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
