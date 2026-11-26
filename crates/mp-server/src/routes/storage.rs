//! Storage locations, and moving a project's data from one to another.
//!
//! A move runs in the background while people keep working:
//! 1. Copy every chunk the project uses to the new location.
//! 2. Switch the project to the new location, keeping the old one as "previous": uploads now
//!    go to the new location, and reads fall back to the old one for anything not copied yet.
//! 3. Copy what arrived in the old location during step 1, then drop "previous".
//!
//! The old copies stay until "Reclaim space" (garbage collection) removes them, so a move can
//! be checked before any data is gone.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::chunks;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::AppState;

/// Chunks copied at once during a move.
const PARALLEL: usize = 8;

/// Every chunk a project's data uses: all branches, all history, drafts and checkpoints.
const PROJECT_CHUNKS: &str = "WITH refm AS (
      SELECT t.manifest_hash AS h FROM tree_entries t JOIN branches b ON b.id = t.branch_id WHERE b.project_id = $1 AND t.manifest_hash IS NOT NULL
      UNION SELECT manifest_hash FROM change_files WHERE project_id = $1 AND manifest_hash IS NOT NULL
      UNION SELECT e->'manifest'->>'hash' FROM drafts d, jsonb_array_elements(d.entries) e WHERE d.project_id = $1 AND jsonb_typeof(e->'manifest') = 'object'
      UNION SELECT e->'manifest'->>'hash' FROM checkpoints k, jsonb_array_elements(k.entries) e WHERE k.project_id = $1 AND jsonb_typeof(e->'manifest') = 'object')
    SELECT DISTINCT jsonb_array_elements(m.chunks)->>0 AS h FROM manifests m JOIN refm ON refm.h = m.hash";

fn move_json(r: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": r.get::<Uuid,_>("id"), "project_id": r.get::<Uuid,_>("project_id"),
        "project_name": r.try_get::<String,_>("project_name").ok(),
        "from": r.get::<String,_>("from_location"), "to": r.get::<String,_>("to_location"), "status": r.get::<String,_>("status"),
        "total_chunks": r.get::<i64,_>("total_chunks"), "copied_chunks": r.get::<i64,_>("copied_chunks"), "copied_bytes": r.get::<i64,_>("copied_bytes"),
        "error": r.get::<Option<String>,_>("error"),
        "started_at": r.get::<DateTime<Utc>,_>("started_at"), "finished_at": r.get::<Option<DateTime<Utc>>,_>("finished_at"),
    })
}

const MOVE_SELECT: &str = "SELECT sm.*, p.name AS project_name FROM storage_moves sm JOIN projects p ON p.id = sm.project_id";

/// Storage locations with what they hold, and recent moves. Any admin may look (to choose a
/// location for a new project); only server admins move projects.
pub async fn locations(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let sc = access::admin_orgs(&s, &user).await?;
    let usage = sqlx::query("SELECT location, count(*) AS chunks, COALESCE(sum(stored_size), 0)::BIGINT AS bytes FROM chunks GROUP BY location").fetch_all(&s.db).await?;
    let projects = sqlx::query("SELECT p.id, p.name, p.storage, p.storage_previous, o.name AS org FROM projects p JOIN orgs o ON o.id = p.org_id WHERE $1 OR p.org_id = ANY($2) ORDER BY o.name, p.name")
        .bind(sc.is_none())
        .bind(sc.clone().unwrap_or_default())
        .fetch_all(&s.db)
        .await?;
    let list: Vec<Value> = s
        .stores
        .iter()
        .map(|(name, st)| {
            let u = usage.iter().find(|r| r.get::<String, _>("location") == *name);
            let ps: Vec<Value> = projects
                .iter()
                .filter(|p| p.get::<String, _>("storage") == *name)
                .map(|p| json!({ "id": p.get::<Uuid,_>("id"), "name": p.get::<String,_>("name"), "org": p.get::<String,_>("org") }))
                .collect();
            json!({
                // `main` marks the default for new projects (what the admin pages badge as Default).
                "name": name, "kind": st.kind, "main": *name == s.config.default_storage, "is_main_store": name == crate::store::MAIN,
                // Paths and buckets are server details: only server admins see them.
                "description": if sc.is_none() { json!(st.description) } else { Value::Null },
                "chunks": u.map(|r| r.get::<i64,_>("chunks")).unwrap_or(0), "stored_bytes": u.map(|r| r.get::<i64,_>("bytes")).unwrap_or(0),
                "projects": ps,
            })
        })
        .collect();
    // Projects pointing at a location the server is no longer configured with.
    let missing: Vec<Value> = projects
        .iter()
        .filter(|p| s.stores.get(&p.get::<String, _>("storage")).is_none())
        .map(|p| json!({ "id": p.get::<Uuid,_>("id"), "name": p.get::<String,_>("name"), "storage": p.get::<String,_>("storage") }))
        .collect();
    let moves = sqlx::query(&format!("{MOVE_SELECT} WHERE $1 OR p.org_id = ANY($2) ORDER BY sm.started_at DESC LIMIT 20"))
        .bind(sc.is_none())
        .bind(sc.unwrap_or_default())
        .fetch_all(&s.db)
        .await?;
    Ok(Json(json!({ "locations": list, "unavailable": missing, "moves": moves.iter().map(move_json).collect::<Vec<_>>() })))
}

/// Where one project's data is, and its latest move.
pub async fn project_storage(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    // The move first, then the project: a finished move is then always shown with its result.
    let last = sqlx::query(&format!("{MOVE_SELECT} WHERE sm.project_id = $1 ORDER BY sm.started_at DESC LIMIT 1")).bind(p).fetch_optional(&s.db).await?;
    let mut conn = s.db.acquire().await?;
    let ps = chunks::ProjectStore::load(&mut conn, p).await?;
    Ok(Json(json!({ "storage": ps.current, "previous": ps.previous, "move": last.as_ref().map(move_json) })))
}

#[derive(Deserialize)]
pub struct MoveBody {
    to: String,
}

/// Start moving a project's data to another location (server admins). Also finishes a move that
/// was interrupted: start it again with the same target.
pub async fn start_move(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<MoveBody>) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let acc = access::project(&s, &user, p, Level::Read).await?;
    let to = b.to.trim().to_lowercase();
    chunks::store(&s, &to).map_err(|_| ApiError::invalid(format!("There is no storage location called {to} on this server.")))?;
    let running: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM storage_moves WHERE project_id = $1 AND status = 'running')").bind(p).fetch_one(&s.db).await?;
    if running {
        return Err(ApiError::conflict(format!("{} is already being moved. Wait for that to finish.", acc.name)));
    }
    let ps = &acc.storage;
    // Resuming: the switch already happened, only the final copy is left.
    let (from, resume) = match &ps.previous {
        Some(prev) if ps.current == to => (prev.clone(), true),
        Some(_) => return Err(ApiError::conflict("This project's last move did not finish. Finish it first by moving it to the same place again.")),
        None if ps.current == to => return Err(ApiError::invalid(format!("{} is already stored in {to}.", acc.name))),
        None => (ps.current.clone(), false),
    };
    chunks::store(&s, &from)?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO storage_moves (project_id, from_location, to_location, started_by) VALUES ($1,$2,$3,$4) RETURNING id")
        .bind(p)
        .bind(&from)
        .bind(&to)
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), "admin.storage_move_started", json!({ "from": from, "to": to })).await;
    let state = s.clone();
    let org = acc.org_id;
    let actor = user.id;
    tokio::spawn(async move {
        let result = run_move(&state, id, p, &from, &to, resume).await;
        let (status, err) = match &result {
            Ok(()) => ("done", None),
            Err(e) => ("failed", Some(e.message.clone())),
        };
        let _ = sqlx::query("UPDATE storage_moves SET status = $2, error = $3, finished_at = now() WHERE id = $1").bind(id).bind(status).bind(&err).execute(&state.db).await;
        let kind = if result.is_ok() { "project.storage_moved" } else { "admin.storage_move_failed" };
        events::emit(&state.db, &state.events, Some(org), Some(p), Some(actor), kind, json!({ "from": from, "to": to, "error": err })).await;
    });
    let r = sqlx::query(&format!("{MOVE_SELECT} WHERE sm.id = $1")).bind(id).fetch_one(&s.db).await?;
    Ok(Json(move_json(&r)))
}

async fn copy_all(s: &AppState, id: Uuid, from: &str, to: &str, hashes: Vec<String>) -> ApiResult<()> {
    let done = Arc::new(AtomicU64::new(0));
    let bytes = Arc::new(AtomicU64::new(0));
    let mut stream = futures::stream::iter(hashes)
        .map(|h| {
            let (s, from, to) = (s.clone(), from.to_string(), to.to_string());
            async move {
                let mut conn = s.db.acquire().await?;
                chunks::copy(&s, &mut conn, &from, &to, &h).await
            }
        })
        .buffer_unordered(PARALLEL);
    while let Some(r) = stream.next().await {
        bytes.fetch_add(r?, Ordering::Relaxed);
        let n = done.fetch_add(1, Ordering::Relaxed) + 1;
        if n % 200 == 0 {
            sqlx::query("UPDATE storage_moves SET copied_chunks = copied_chunks + 200, copied_bytes = copied_bytes + $2 WHERE id = $1")
                .bind(id)
                .bind(bytes.swap(0, Ordering::Relaxed) as i64)
                .execute(&s.db)
                .await?;
        }
    }
    let rest = done.load(Ordering::Relaxed) % 200;
    sqlx::query("UPDATE storage_moves SET copied_chunks = copied_chunks + $2, copied_bytes = copied_bytes + $3 WHERE id = $1")
        .bind(id)
        .bind(rest as i64)
        .bind(bytes.load(Ordering::Relaxed) as i64)
        .execute(&s.db)
        .await?;
    Ok(())
}

/// Chunks the project uses that are in `location` and not yet in `target`.
async fn project_chunks_in(s: &AppState, project: Uuid, location: &str, target: &str, extra_since: Option<DateTime<Utc>>) -> ApiResult<Vec<String>> {
    let mut hashes: Vec<String> = sqlx::query_scalar(&format!(
        "WITH used AS ({PROJECT_CHUNKS}) SELECT c.hash FROM chunks c JOIN used ON used.h = c.hash
         WHERE c.location = $2 AND NOT EXISTS (SELECT 1 FROM chunks t WHERE t.location = $3 AND t.hash = c.hash)"
    ))
    .bind(project)
    .bind(location)
    .bind(target)
    .fetch_all(&s.db)
    .await?;
    if let Some(since) = extra_since {
        // Uploaded during the move but not published yet (a publish in progress).
        let recent: Vec<String> = sqlx::query_scalar(
            "SELECT c.hash FROM chunks c WHERE c.location = $1 AND c.created_at >= $2 AND NOT EXISTS (SELECT 1 FROM chunks t WHERE t.location = $3 AND t.hash = c.hash)",
        )
        .bind(location)
        .bind(since)
        .bind(target)
        .fetch_all(&s.db)
        .await?;
        hashes.extend(recent);
        hashes.sort();
        hashes.dedup();
    }
    Ok(hashes)
}

async fn run_move(s: &AppState, id: Uuid, project: Uuid, from: &str, to: &str, resume: bool) -> ApiResult<()> {
    let started: DateTime<Utc> = sqlx::query_scalar("SELECT started_at FROM storage_moves WHERE id = $1").bind(id).fetch_one(&s.db).await?;
    if !resume {
        let hashes = project_chunks_in(s, project, from, to, None).await?;
        sqlx::query("UPDATE storage_moves SET total_chunks = $2 WHERE id = $1").bind(id).bind(hashes.len() as i64).execute(&s.db).await?;
        copy_all(s, id, from, to, hashes).await?;
        // Switch: from now on uploads go to the new location; reads fall back to the old one.
        let mut tx = s.db.begin().await?;
        crate::branches::lock_project(&mut tx, project).await?;
        sqlx::query("UPDATE projects SET storage = $2, storage_previous = $3 WHERE id = $1").bind(project).bind(to).bind(from).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    // Whatever reached the old location while copying.
    let since = if resume { DateTime::<Utc>::MIN_UTC } else { started };
    let rest = project_chunks_in(s, project, from, to, Some(since)).await?;
    sqlx::query("UPDATE storage_moves SET total_chunks = total_chunks + $2 WHERE id = $1").bind(id).bind(rest.len() as i64).execute(&s.db).await?;
    copy_all(s, id, from, to, rest).await?;
    sqlx::query("UPDATE projects SET storage_previous = NULL WHERE id = $1 AND storage = $2").bind(project).bind(to).execute(&s.db).await?;
    Ok(())
}

/// A move cannot survive a restart (it runs inside the server). Mark it so an admin can start
/// it again; the project keeps working meanwhile, reading from both locations.
pub async fn resume_after_restart(s: &AppState) -> anyhow::Result<()> {
    let n = sqlx::query("UPDATE storage_moves SET status = 'interrupted', finished_at = now(), error = 'The server restarted during the move. Start it again to finish.' WHERE status = 'running'")
        .execute(&s.db)
        .await?
        .rows_affected();
    if n > 0 {
        tracing::warn!(moves = n, "storage moves were interrupted by a restart; start them again from Admin");
    }
    Ok(())
}
