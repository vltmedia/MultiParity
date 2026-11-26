//! Backup status and "Back up now" for Admin > Fixing (server admins).

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;

use crate::access;
use crate::auth::CurrentUser;
use crate::backup;
use crate::error::{ApiError, ApiResult};
use crate::store::BlobStore;
use crate::AppState;

pub async fn status(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let Some(cfg) = s.config.backup.clone() else {
        return Ok(Json(json!({ "configured": false })));
    };
    let runs = sqlx::query("SELECT id, started_at, finished_at, status, error, snapshot, db_bytes, new_chunks, new_chunk_bytes, total_chunks FROM backup_runs ORDER BY started_at DESC LIMIT 20")
        .fetch_all(&s.db)
        .await?;
    let runs: Vec<Value> = runs
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<uuid::Uuid,_>("id"), "started_at": r.get::<DateTime<Utc>,_>("started_at"), "finished_at": r.get::<Option<DateTime<Utc>>,_>("finished_at"),
                "status": r.get::<String,_>("status"), "error": r.get::<Option<String>,_>("error"), "snapshot": r.get::<Option<String>,_>("snapshot"),
                "db_bytes": r.get::<Option<i64>,_>("db_bytes"), "new_chunks": r.get::<Option<i64>,_>("new_chunks"),
                "new_chunk_bytes": r.get::<Option<i64>,_>("new_chunk_bytes"), "total_chunks": r.get::<Option<i64>,_>("total_chunks"),
            })
        })
        .collect();
    // The target is read on demand; a missing drive or bad credentials show up here.
    let (snapshots, target_error) = match BlobStore::open(&cfg.target).await {
        Ok(t) => match backup::snapshots(&t).await {
            Ok(list) => (list, None),
            Err(e) => (Vec::new(), Some(format!("{e:#}"))),
        },
        Err(e) => (Vec::new(), Some(format!("{e:#}"))),
    };
    let keep = backup::keep_set(&snapshots, &cfg);
    Ok(Json(json!({
        "configured": true,
        "target": cfg.target.describe(),
        "target_error": target_error,
        "schedule": format!("{:02}:{:02} UTC", cfg.at.0, cfg.at.1),
        "next_run": backup::next_run(&cfg, Utc::now()),
        "keep_daily": cfg.keep_daily, "keep_weekly": cfg.keep_weekly,
        "running": s.backup_running.load(std::sync::atomic::Ordering::SeqCst),
        "runs": runs,
        "snapshots": snapshots.iter().map(|m| json!({
            "id": m.id, "created_at": m.created_at, "server_version": m.server_version, "tables": m.tables.len(),
            "rows": m.tables.iter().map(|t| t.rows).sum::<i64>(), "chunks": m.chunks, "db_bytes": m.db_bytes,
            "new_chunks": m.new_chunks, "new_chunk_bytes": m.new_chunk_bytes, "kept_because": if keep.contains(&m.id) { "kept" } else { "next to go" },
        })).collect::<Vec<_>>(),
    })))
}

/// Start a backup now. It runs in the background; poll the status.
pub async fn run_now(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let cfg = s.config.backup.clone().ok_or_else(|| ApiError::invalid("Backups are not set up on this server. Set MP_BACKUP_TARGET in its settings."))?;
    if s.backup_running.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(ApiError::conflict("A backup is already running."));
    }
    let state = s.clone();
    let by = user.id;
    tokio::spawn(async move {
        let _ = backup::run_recorded(&state, &cfg, Some(by)).await;
    });
    crate::events::emit(&s.db, &s.events, None, None, Some(user.id), "admin.backup_started", json!({})).await;
    Ok(Json(json!({ "started": true })))
}
