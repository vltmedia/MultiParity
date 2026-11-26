use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::branches::{claim_scope, load_branch};
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{claim_from_row, Claim, CLAIM_SELECT};
use crate::AppState;

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Claim>>> {
    access::project(&s, &user, p, Level::Read).await?;
    let sql = format!("{CLAIM_SELECT} WHERE cl.project_id = $1 ORDER BY cl.created_at");
    Ok(Json(sqlx::query(&sql).bind(p).fetch_all(&s.db).await?.iter().map(|r| claim_from_row(r, false)).collect()))
}

#[derive(Deserialize)]
pub struct ClaimBody {
    branch: Uuid,
    paths: Vec<String>,
    device: Option<String>,
}

/// Claim files. Paths already held by someone else are reported back in `held`; the rest are
/// claimed (claiming a path you already hold is a no-op). New paths can be claimed too, which
/// reserves the name.
pub async fn claim(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<ClaimBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Write).await?;
    let paths: Vec<String> = b.paths.iter().filter_map(|x| mp_core::paths::normalize(x)).collect();
    if paths.is_empty() {
        return Err(ApiError::invalid("Name at least one file to claim."));
    }
    let mut tx = s.db.begin().await?;
    let br = load_branch(&mut tx, p, b.branch).await?;
    if br.archived {
        return Err(ApiError::conflict("This branch is archived."));
    }
    let scope = claim_scope(&mut tx, b.branch).await?;
    let mut claimed = Vec::new();
    for path in &paths {
        let n = sqlx::query("INSERT INTO claims (project_id, scope_id, path, branch_id, user_id, device) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING")
            .bind(p)
            .bind(scope)
            .bind(path)
            .bind(b.branch)
            .bind(user.id)
            .bind(&b.device)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if n == 1 {
            claimed.push(path.clone());
        }
    }
    let sql = format!("{CLAIM_SELECT} WHERE cl.project_id = $1 AND cl.scope_id = $2 AND cl.path = ANY($3) AND cl.user_id <> $4");
    let held: Vec<Claim> = sqlx::query(&sql).bind(p).bind(scope).bind(&paths).bind(user.id).fetch_all(&mut *tx).await?.iter().map(|r| claim_from_row(r, false)).collect();
    let ev = if claimed.is_empty() {
        None
    } else {
        Some(events::record(&mut *tx, Some(acc.org_id), Some(p), Some(user.id), "claim.added", json!({ "paths": claimed, "branch_id": b.branch })).await?)
    };
    tx.commit().await?;
    if let Some(ev) = ev {
        events::publish_saved(&s.db, &s.events, ev).await;
    }
    // Paths I already held count as claimed for the caller.
    let mine: Vec<String> = paths.into_iter().filter(|x| !held.iter().any(|h| &h.path == x)).collect();
    Ok(Json(json!({ "claimed": mine, "held": held })))
}

#[derive(Deserialize)]
pub struct ReleaseBody {
    branch: Uuid,
    paths: Vec<String>,
}

pub async fn release(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<ReleaseBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    let mut conn = s.db.acquire().await?;
    let scope = claim_scope(&mut conn, b.branch).await?;
    drop(conn);
    let released: Vec<String> = sqlx::query_scalar("DELETE FROM claims WHERE project_id = $1 AND scope_id = $2 AND path = ANY($3) AND user_id = $4 RETURNING path")
        .bind(p)
        .bind(scope)
        .bind(&b.paths)
        .bind(user.id)
        .fetch_all(&s.db)
        .await?;
    if !released.is_empty() {
        events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), "claim.released", json!({ "paths": released })).await;
    }
    Ok(Json(json!({ "released": released })))
}

#[derive(Deserialize)]
pub struct NudgeBody {
    branch: Uuid,
    path: String,
}

/// "Ask to release": tells the holder someone is waiting.
pub async fn nudge(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<NudgeBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, Level::Read).await?;
    let mut conn = s.db.acquire().await?;
    let scope = claim_scope(&mut conn, b.branch).await?;
    drop(conn);
    let holder: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM claims WHERE project_id = $1 AND scope_id = $2 AND path = $3")
        .bind(p)
        .bind(scope)
        .bind(&b.path)
        .fetch_optional(&s.db)
        .await?;
    let holder = holder.ok_or_else(|| ApiError::not_found("A claim on that file"))?;
    events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), "claim.nudged", json!({ "path": b.path, "holder": holder, "holder_id": holder, "from": user.name, "branch_id": b.branch })).await;
    Ok(Json(json!({})))
}
