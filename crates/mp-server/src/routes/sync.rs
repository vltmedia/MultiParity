use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use mp_core::manifest::is_hash;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::chunks;
use crate::branches::{self, claim_scope, load_branch, lock_project, record_change_file, take_change_number, upsert_entry, Entry};
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{claim_from_row, load_change, PublishEntry, CLAIM_SELECT};
use crate::AppState;

const MAX_CHUNK: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
pub struct MissingBody {
    hashes: Vec<String>,
}

pub async fn missing_chunks(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<MissingBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    if b.hashes.len() > 100_000 {
        return Err(ApiError::invalid("Ask about at most 100,000 chunks at a time."));
    }
    let hashes: Vec<String> = b.hashes.into_iter().filter(|h| is_hash(h)).collect();
    let mut conn = s.db.acquire().await?;
    let present = chunks::present(&mut conn, &acc.storage, &hashes).await?;
    let mut missing: Vec<&String> = hashes.iter().filter(|h| !present.contains(*h)).collect();
    missing.dedup();
    Ok(Json(json!({ "missing": missing })))
}

/// Upload one zstd-compressed chunk. The server decompresses and checks the hash before storing.
pub async fn put_chunk(State(s): State<AppState>, user: CurrentUser, Path((p, hash)): Path<(Uuid, String)>, body: Bytes) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    if !is_hash(&hash) {
        return Err(ApiError::invalid("That is not a valid chunk hash."));
    }
    let raw = mp_core::decompress(&body, MAX_CHUNK).map_err(|_| ApiError::invalid("The chunk could not be decompressed. Upload it zstd-compressed."))?;
    if mp_core::hash_bytes(&raw) != hash {
        return Err(ApiError::invalid("The chunk's content does not match its hash. The upload may have been corrupted; try again."));
    }
    let mut conn = s.db.acquire().await?;
    chunks::put(&s, &mut conn, &acc.storage, &hash, body, raw.len()).await?;
    Ok(Json(json!({})))
}

pub async fn get_chunk(State(s): State<AppState>, user: CurrentUser, Path((p, hash)): Path<(Uuid, String)>) -> ApiResult<impl IntoResponse> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    if !is_hash(&hash) {
        return Err(ApiError::invalid("That is not a valid chunk hash."));
    }
    let data = chunks::get(&s, &acc.storage, &hash).await?;
    Ok(([(header::CONTENT_TYPE, "application/zstd"), (header::CACHE_CONTROL, "private, max-age=31536000, immutable")], data))
}

pub async fn get_manifest(State(s): State<AppState>, user: CurrentUser, Path((p, hash)): Path<(Uuid, String)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let r = sqlx::query("SELECT hash, size, file_hash, chunks FROM manifests WHERE hash = $1")
        .bind(&hash)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That file version"))?;
    Ok(Json(json!({ "hash": r.get::<String,_>("hash"), "size": r.get::<i64,_>("size"), "file_hash": r.get::<String,_>("file_hash"), "chunks": r.get::<Value,_>("chunks") })))
}

/// Validate entries' manifests and that every chunk they name is stored in the project's
/// location (copying any that are still only in its previous one, during a move). Returns entries
/// ready to write.
pub async fn check_entries(s: &AppState, db: &mut sqlx::PgConnection, ps: &chunks::ProjectStore, entries: &[PublishEntry]) -> ApiResult<Vec<(String, Entry)>> {
    let mut out = Vec::with_capacity(entries.len());
    let mut seen = std::collections::HashSet::new();
    let mut all_chunks: Vec<String> = Vec::new();
    for e in entries {
        let path = mp_core::paths::normalize(&e.path).ok_or_else(|| ApiError::invalid(format!("{} is not a valid path inside a project.", e.path)))?;
        if !seen.insert(path.clone()) {
            return Err(ApiError::invalid(format!("{path} appears twice in one change.")));
        }
        let entry = match e.op.as_str() {
            "delete" => Entry::default(),
            "put" => {
                let m = e.manifest.as_ref().ok_or_else(|| ApiError::invalid(format!("{path} has no content description.")))?;
                if !m.verify() {
                    return Err(ApiError::invalid(format!("The content description for {path} is inconsistent.")));
                }
                all_chunks.extend(m.chunks.iter().map(|c| c.hash.clone()));
                branches::save_manifest(db, m).await?;
                Entry { file_hash: Some(m.file_hash.clone()), manifest_hash: Some(m.hash.clone()), size: m.size as i64 }
            }
            other => return Err(ApiError::invalid(format!("Unknown operation {other} for {path}."))),
        };
        out.push((path, entry));
    }
    all_chunks.sort();
    all_chunks.dedup();
    let missing = chunks::ensure_current(s, db, ps, &all_chunks).await?;
    if !missing.is_empty() {
        return Err(ApiError::new(axum::http::StatusCode::UNPROCESSABLE_ENTITY, "missing_chunks", "Some file data has not been uploaded yet. Upload the missing chunks and publish again."));
    }
    Ok(out)
}

#[derive(Deserialize)]
pub struct PublishBody {
    message: String,
    entries: Vec<PublishEntry>,
}

pub async fn publish(State(s): State<AppState>, user: CurrentUser, Path((p, b)): Path<(Uuid, Uuid)>, Json(body): Json<PublishBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let message = body.message.trim();
    if message.is_empty() {
        return Err(ApiError::invalid("Say what you changed before publishing."));
    }
    if body.entries.is_empty() {
        return Err(ApiError::invalid("There is nothing to publish."));
    }
    let mut tx = s.db.begin().await?;
    lock_project(&mut tx, p).await?;
    let branch = load_branch(&mut tx, p, b).await?;
    if branch.archived {
        return Err(ApiError::conflict("This branch is archived. Switch to another branch to publish."));
    }
    if branch.protected && acc.level < Level::Admin {
        return Err(ApiError::forbidden(format!("{} is protected: work arrives by merging a branch into it.", branch.name)));
    }
    let entries = check_entries(&s, &mut tx, &acc.storage, &body.entries).await?;
    let paths: Vec<String> = entries.iter().map(|(p, _)| p.clone()).collect();

    // Stale: the server's version moved on since the client last got it.
    let current: Vec<(String, Option<String>)> = sqlx::query_as("SELECT path, file_hash FROM tree_entries WHERE branch_id = $1 AND path = ANY($2)")
        .bind(b)
        .bind(&paths)
        .fetch_all(&mut *tx)
        .await?;
    let current: std::collections::HashMap<String, Option<String>> = current.into_iter().collect();
    let mut stale = Vec::new();
    for (e, (path, _)) in body.entries.iter().zip(&entries) {
        let server = current.get(path).cloned().flatten();
        if server != e.prev_file_hash {
            stale.push(path.clone());
        }
    }
    // Claimed by someone else.
    let scope = claim_scope(&mut tx, b).await?;
    let sql = format!("{CLAIM_SELECT} WHERE cl.project_id = $1 AND cl.scope_id = $2 AND cl.path = ANY($3) AND cl.user_id <> $4");
    let claimed: Vec<_> = sqlx::query(&sql).bind(p).bind(scope).bind(&paths).bind(user.id).fetch_all(&mut *tx).await?.iter().map(|r| claim_from_row(r, false)).collect();
    if !stale.is_empty() || !claimed.is_empty() {
        let msg = if !claimed.is_empty() {
            format!("{} file(s) are claimed by someone else. Ask them to release, or leave those files out.", claimed.len())
        } else {
            format!("{} file(s) changed on the server since you last got latest. Get latest, then either discard your version or publish yours over theirs.", stale.len())
        };
        return Err(ApiError::conflict(msg).with_details(json!({ "stale": stale, "claimed": claimed })));
    }

    let n = take_change_number(&mut tx, p).await?;
    let mut total = 0i64;
    for (path, e) in &entries {
        let had = current.get(path).cloned().flatten().is_some();
        let op = match (&e.file_hash, had) {
            (None, _) => "delete",
            (Some(_), false) => "add",
            _ => "edit",
        };
        upsert_entry(&mut tx, b, path, e, n, None).await?;
        record_change_file(&mut tx, p, n, path, op, e).await?;
        total += e.size;
    }
    sqlx::query("INSERT INTO changes (project_id, number, branch_id, author_id, message, kind, file_count, size) VALUES ($1,$2,$3,$4,$5,'publish',$6,$7)")
        .bind(p)
        .bind(n)
        .bind(b)
        .bind(user.id)
        .bind(message)
        .bind(entries.len() as i32)
        .bind(total)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE branches SET head_change = $2 WHERE id = $1").bind(b).bind(n).execute(&mut *tx).await?;
    // Publishing hands claims back.
    sqlx::query("DELETE FROM claims WHERE project_id = $1 AND scope_id = $2 AND path = ANY($3) AND user_id = $4")
        .bind(p)
        .bind(scope)
        .bind(&paths)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    let ev = events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "change.published", json!({ "number": n, "branch_id": b, "message": message, "files": entries.len() })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "change": load_change(&s.db, p, n).await? })))
}
