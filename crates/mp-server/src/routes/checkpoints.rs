//! Checkpoints kept on the server. Each belongs to one person and only they can see or fetch it;
//! admins see how much space they take and can clear them.

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::models::PublishEntry;
use crate::routes::sync::check_entries;
use crate::AppState;

/// Automatic checkpoints kept per person, project and computer, and for how long.
const KEEP_AUTO: i64 = 50;
const KEEP_AUTO_DAYS: i32 = 30;
/// Undo checkpoints (taken before rewinding, discarding or picking up a draft).
const KEEP_SYSTEM_DAYS: i32 = 30;

const COLUMNS: &str = "id, client_id, branch_id, device, note, auto, system, file_count, size, stored, created_at";

fn checkpoint_json(r: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": r.get::<String,_>("client_id"), "server_id": r.get::<Uuid,_>("id"), "branch_id": r.get::<Uuid,_>("branch_id"),
        "device": r.get::<Option<String>,_>("device"), "note": r.get::<Option<String>,_>("note"),
        "auto": r.get::<bool,_>("auto"), "system": r.get::<bool,_>("system"),
        "file_count": r.get::<i32,_>("file_count"), "size": r.get::<i64,_>("size"), "stored": r.get::<i64,_>("stored"),
        "created_at": r.get::<DateTime<Utc>,_>("created_at"),
    })
}

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let sql = format!("SELECT {COLUMNS} FROM checkpoints WHERE project_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 1000");
    let rows = sqlx::query(&sql).bind(p).bind(user.id).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(checkpoint_json).collect()))
}

#[derive(Deserialize)]
pub struct CreateCheckpoint {
    id: String,
    branch: Uuid,
    device: Option<String>,
    note: Option<String>,
    #[serde(default)]
    auto: bool,
    #[serde(default)]
    system: bool,
    /// Bytes the client uploaded for this checkpoint (data the server did not have yet).
    #[serde(default)]
    stored: i64,
    entries: Vec<PublishEntry>,
}

/// Keep a checkpoint on the server. Its file data must already be uploaded (as for publishing).
pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<CreateCheckpoint>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Write).await?;
    let client_id = b.id.trim();
    if client_id.is_empty() || client_id.len() > 80 {
        return Err(ApiError::invalid("The checkpoint needs an id of up to 80 characters."));
    }
    if b.entries.is_empty() {
        return Err(ApiError::invalid("There is nothing in this checkpoint."));
    }
    let mut tx = s.db.begin().await?;
    let checked = check_entries(&s, &mut tx, &b.entries).await?;
    let size: i64 = checked.iter().map(|(_, e)| e.size).sum();
    let sql = format!(
        "INSERT INTO checkpoints (project_id, branch_id, user_id, client_id, device, note, auto, system, entries, file_count, size, stored)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
         ON CONFLICT (project_id, user_id, client_id) DO UPDATE SET entries = EXCLUDED.entries, file_count = EXCLUDED.file_count, size = EXCLUDED.size
         RETURNING {COLUMNS}"
    );
    let row = sqlx::query(&sql)
        .bind(p)
        .bind(b.branch)
        .bind(user.id)
        .bind(client_id)
        .bind(b.device.as_deref().map(str::trim).filter(|d| !d.is_empty()))
        .bind(b.note.as_deref().map(str::trim).filter(|n| !n.is_empty()))
        .bind(b.auto)
        .bind(b.system)
        .bind(serde_json::to_value(&b.entries).unwrap_or_default())
        .bind(checked.len() as i32)
        .bind(size)
        .bind(b.stored.max(0))
        .fetch_one(&mut *tx)
        .await?;
    // Thin out this person's automatic and undo checkpoints for this project and computer.
    sqlx::query(
        "DELETE FROM checkpoints c WHERE c.project_id = $1 AND c.user_id = $2 AND (
           (c.auto AND c.created_at < now() - make_interval(days => $4))
           OR (c.system AND c.created_at < now() - make_interval(days => $5))
           OR (c.auto AND c.id IN (
                SELECT id FROM (SELECT id, row_number() OVER (PARTITION BY device ORDER BY created_at DESC) AS n
                                FROM checkpoints WHERE project_id = $1 AND user_id = $2 AND auto) ranked WHERE n > $3)))",
    )
    .bind(p)
    .bind(user.id)
    .bind(KEEP_AUTO)
    .bind(KEEP_AUTO_DAYS)
    .bind(KEEP_SYSTEM_DAYS)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(checkpoint_json(&row)))
}

/// One checkpoint with its files, by the id the person's computer gave it.
pub async fn get(State(s): State<AppState>, user: CurrentUser, Path((p, id)): Path<(Uuid, String)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let sql = format!("SELECT {COLUMNS}, entries FROM checkpoints WHERE project_id = $1 AND user_id = $2 AND client_id = $3");
    let r = sqlx::query(&sql).bind(p).bind(user.id).bind(&id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That checkpoint"))?;
    let mut v = checkpoint_json(&r);
    v["entries"] = r.get("entries");
    Ok(Json(v))
}

pub async fn remove(State(s): State<AppState>, user: CurrentUser, Path((p, id)): Path<(Uuid, String)>) -> ApiResult<Json<Value>> {
    access::project(&s, &user, p, Level::Read).await?;
    let n = sqlx::query("DELETE FROM checkpoints WHERE project_id = $1 AND user_id = $2 AND client_id = $3").bind(p).bind(user.id).bind(&id).execute(&s.db).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::not_found("That checkpoint"));
    }
    Ok(Json(json!({})))
}

// ---------------------------------------------------------------------------------------------
// Admin

#[derive(Deserialize)]
pub struct UsageQuery {
    org: Option<Uuid>,
}

/// How much checkpoint space each person uses in each project. Admins see sizes and counts,
/// never the contents.
pub async fn admin_usage(State(s): State<AppState>, user: CurrentUser, Query(q): Query<UsageQuery>) -> ApiResult<Json<Vec<Value>>> {
    let sc = access::admin_orgs(&s, &user).await?;
    let rows = sqlx::query(
        "SELECT u.id AS u_id, u.name AS u_name, u.email AS u_email, p.id AS p_id, p.name AS p_name, o.id AS o_id, o.name AS o_name,
                count(*) AS checkpoints, count(*) FILTER (WHERE c.auto) AS automatic,
                COALESCE(sum(c.stored), 0)::BIGINT AS stored, COALESCE(sum(c.size), 0)::BIGINT AS size,
                min(c.created_at) AS oldest, max(c.created_at) AS newest
         FROM checkpoints c JOIN users u ON u.id = c.user_id JOIN projects p ON p.id = c.project_id JOIN orgs o ON o.id = p.org_id
         WHERE ($1 OR p.org_id = ANY($2)) AND ($3::uuid IS NULL OR p.org_id = $3)
         GROUP BY u.id, p.id, o.id ORDER BY stored DESC, checkpoints DESC LIMIT 2000",
    )
    .bind(sc.is_none())
    .bind(sc.unwrap_or_default())
    .bind(q.org)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                json!({
                    "user": { "id": r.get::<Uuid,_>("u_id"), "name": r.get::<String,_>("u_name"), "email": r.get::<String,_>("u_email") },
                    "project": { "id": r.get::<Uuid,_>("p_id"), "name": r.get::<String,_>("p_name") },
                    "org": { "id": r.get::<Uuid,_>("o_id"), "name": r.get::<String,_>("o_name") },
                    "checkpoints": r.get::<i64,_>("checkpoints"), "automatic": r.get::<i64,_>("automatic"),
                    "stored": r.get::<i64,_>("stored"), "size": r.get::<i64,_>("size"),
                    "oldest": r.get::<DateTime<Utc>,_>("oldest"), "newest": r.get::<DateTime<Utc>,_>("newest"),
                })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct ClearBody {
    user: Uuid,
    project: Uuid,
    /// Only automatic and undo checkpoints; the ones the person made themselves stay.
    #[serde(default)]
    only_automatic: bool,
    /// Only checkpoints older than this many days.
    older_than_days: Option<i32>,
}

/// Remove a person's checkpoints in a project. The space comes back at the next clean-up of
/// unused data (Fixing > Storage).
pub async fn admin_clear(State(s): State<AppState>, user: CurrentUser, Json(b): Json<ClearBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, b.project, Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    let n = sqlx::query(
        "DELETE FROM checkpoints WHERE project_id = $1 AND user_id = $2 AND (NOT $3 OR auto OR system)
           AND ($4::int IS NULL OR created_at < now() - make_interval(days => $4))",
    )
    .bind(b.project)
    .bind(b.user)
    .bind(b.only_automatic)
    .bind(b.older_than_days)
    .execute(&s.db)
    .await?
    .rows_affected();
    crate::events::emit(
        &s.db,
        &s.events,
        Some(acc.org_id),
        Some(b.project),
        Some(user.id),
        "admin.checkpoints_cleared",
        json!({ "user_id": b.user, "removed": n, "only_automatic": b.only_automatic, "older_than_days": b.older_than_days }),
    )
    .await;
    Ok(Json(json!({ "removed": n })))
}
