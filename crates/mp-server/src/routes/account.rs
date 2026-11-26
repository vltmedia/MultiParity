use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::auth::{check_password_strength, create_session, hash_password, verify_password, CurrentUser};
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{load_me, Me};
use crate::util::hash_token;
use crate::AppState;

#[derive(Deserialize)]
pub struct LoginBody {
    email: String,
    password: String,
    device: Option<String>,
}

/// A hash checked when the email matches no account, so a wrong email takes as long as a wrong
/// password and nobody can find out which emails have accounts by timing.
fn dummy_hash() -> &'static str {
    static H: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    H.get_or_init(|| crate::auth::hash_password("no account has this password").unwrap_or_default())
}

pub async fn login(
    State(s): State<AppState>,
    headers: axum::http::HeaderMap,
    conn: Option<axum::extract::ConnectInfo<std::net::SocketAddr>>,
    Json(b): Json<LoginBody>,
) -> ApiResult<Json<Value>> {
    use crate::limits::{client_ip, minutes, PER_ACCOUNT, PER_ADDRESS};
    let email = b.email.trim().to_lowercase();
    let ip = client_ip(s.config.client_ip_header.as_deref(), &headers, conn.as_ref());
    let (ip_key, account_key) = (format!("ip:{ip}"), format!("account:{email}"));
    let wait = s.sign_in.blocked(&ip_key, PER_ADDRESS).into_iter().chain(s.sign_in.blocked(&account_key, PER_ACCOUNT)).max();
    if let Some(wait) = wait {
        return Err(ApiError::new(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            "too_many_attempts",
            format!("Too many wrong passwords. Try again in {} minute(s), or ask an admin to set a new password.", minutes(wait)),
        )
        .with_details(json!({ "retry_after_secs": wait.as_secs() })));
    }
    let row = sqlx::query("SELECT id, password_hash, status FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&s.db)
        .await?;
    let bad = || ApiError::new(axum::http::StatusCode::UNAUTHORIZED, "bad_credentials", "That email and password do not match an account on this server.");
    let hash: Option<String> = row.as_ref().and_then(|r| r.get("password_hash"));
    let password = b.password.clone();
    let ok = tokio::task::spawn_blocking(move || verify_password(&password, hash.as_deref().unwrap_or(dummy_hash())) && hash.is_some())
        .await
        .unwrap_or(false);
    let Some(row) = row.filter(|_| ok) else {
        s.sign_in.fail(&ip_key);
        if s.sign_in.fail(&account_key) == PER_ACCOUNT {
            // Recorded once, when the account starts waiting; the attempts themselves are not.
            crate::events::emit(&s.db, &s.events, None, None, None, "auth.locked", json!({ "email": email, "address": ip, "minutes": minutes(crate::limits::WINDOW) })).await;
        }
        return Err(bad());
    };
    s.sign_in.clear(&account_key);
    let status: String = row.get("status");
    if status != "active" {
        return Err(ApiError::new(axum::http::StatusCode::FORBIDDEN, "suspended", "This account is suspended. Ask an admin of your organization to reactivate it."));
    }
    let uid: Uuid = row.get("id");
    let token = create_session(&s.db, uid, "session", None, b.device.as_deref()).await?;
    let me = load_me(&s.db, uid).await?;
    Ok(Json(json!({ "token": token, "user": me })))
}

pub async fn logout(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1").bind(user.session_id).execute(&s.db).await?;
    Ok(Json(json!({})))
}

pub async fn me(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Me>> {
    Ok(Json(load_me(&s.db, user.id).await?))
}

#[derive(Deserialize)]
pub struct TokenBody {
    label: String,
}

/// Personal API tokens for scripts and CI.
pub async fn create_token(State(s): State<AppState>, user: CurrentUser, Json(b): Json<TokenBody>) -> ApiResult<Json<Value>> {
    let label = b.label.trim();
    if label.is_empty() {
        return Err(ApiError::invalid("Give the token a name so you can recognise it later."));
    }
    let token = create_session(&s.db, user.id, "token", Some(label), None).await?;
    Ok(Json(json!({ "token": token, "label": label })))
}

pub async fn list_tokens(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<Value>>> {
    let rows = sqlx::query("SELECT id, kind, label, device, created_at, last_used_at FROM sessions WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC")
        .bind(user.id)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows.iter().map(session_json).collect()))
}

pub fn session_json(r: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": r.get::<Uuid, _>("id"),
        "kind": r.get::<String, _>("kind"),
        "label": r.get::<Option<String>, _>("label"),
        "device": r.get::<Option<String>, _>("device"),
        "created_at": r.get::<DateTime<Utc>, _>("created_at"),
        "last_used_at": r.get::<Option<DateTime<Utc>>, _>("last_used_at"),
    })
}

pub async fn revoke_token(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let n = sqlx::query("UPDATE sessions SET revoked_at = now() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL")
        .bind(id)
        .bind(user.id)
        .execute(&s.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::not_found("That token"));
    }
    Ok(Json(json!({})))
}

async fn find_invite(s: &AppState, token: &str) -> ApiResult<sqlx::postgres::PgRow> {
    let row = sqlx::query(
        "SELECT i.id, i.org_id, i.email, i.role, i.expires_at, i.accepted_at, i.revoked_at, o.id AS o_id, o.slug, o.name, o.created_at
         FROM invites i JOIN orgs o ON o.id = i.org_id WHERE i.token_hash = $1",
    )
    .bind(hash_token(token))
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::not_found("That invite"))?;
    let expired = row.get::<Option<DateTime<Utc>>, _>("expires_at").map(|e| e < Utc::now()).unwrap_or(false);
    if row.get::<Option<DateTime<Utc>>, _>("revoked_at").is_some() || expired {
        return Err(ApiError::new(axum::http::StatusCode::GONE, "invite_expired", "This invite has expired or was withdrawn. Ask for a new one."));
    }
    if row.get::<Option<DateTime<Utc>>, _>("accepted_at").is_some() {
        return Err(ApiError::new(axum::http::StatusCode::GONE, "invite_used", "This invite has already been used. Sign in instead."));
    }
    Ok(row)
}

pub async fn invite_info(State(s): State<AppState>, Path(token): Path<String>) -> ApiResult<Json<Value>> {
    let r = find_invite(&s, &token).await?;
    Ok(Json(json!({
        "org": { "id": r.get::<Uuid,_>("o_id"), "slug": r.get::<String,_>("slug"), "name": r.get::<String,_>("name"), "created_at": r.get::<DateTime<Utc>,_>("created_at") },
        "role": r.get::<String,_>("role"),
        "email": r.get::<Option<String>,_>("email"),
        "expires_at": r.get::<Option<DateTime<Utc>>,_>("expires_at"),
    })))
}

#[derive(Deserialize)]
pub struct AcceptBody {
    name: Option<String>,
    email: Option<String>,
    password: Option<String>,
}

/// Accept an invite. Signed in: join with the current account. Otherwise: create an account
/// (email comes from the invite when it names one).
pub async fn accept_invite(State(s): State<AppState>, Path(token): Path<String>, user: Option<CurrentUser>, Json(b): Json<AcceptBody>) -> ApiResult<Json<Value>> {
    let inv = find_invite(&s, &token).await?;
    let invite_id: Uuid = inv.get("id");
    let org_id: Uuid = inv.get("org_id");
    let role: String = inv.get("role");
    let invite_email: Option<String> = inv.get("email");
    let mut tx = s.db.begin().await?;
    let (uid, new_token) = match user {
        Some(u) => (u.id, None),
        None => {
            let email = invite_email.clone().or(b.email.clone()).map(|e| e.trim().to_lowercase()).filter(|e| crate::util::looks_like_email(e))
                .ok_or_else(|| ApiError::invalid("Enter your email address."))?;
            let name = b.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| ApiError::invalid("Enter your name."))?;
            let password = b.password.as_deref().ok_or_else(|| ApiError::invalid("Choose a password."))?;
            check_password_strength(password)?;
            let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(&email).fetch_optional(&mut *tx).await?;
            if exists.is_some() {
                return Err(ApiError::conflict("An account with that email already exists. Sign in first, then open the invite link again."));
            }
            let uid: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ($1,$2,$3) RETURNING id")
                .bind(&email)
                .bind(name)
                .bind(hash_password(password)?)
                .fetch_one(&mut *tx)
                .await?;
            (uid, Some(()))
        }
    };
    sqlx::query("INSERT INTO memberships (org_id, user_id, role) VALUES ($1,$2,$3) ON CONFLICT (org_id, user_id) DO UPDATE SET role = EXCLUDED.role, expires_at = NULL")
        .bind(org_id)
        .bind(uid)
        .bind(&role)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE invites SET accepted_at = now(), accepted_by = $2, token = NULL WHERE id = $1").bind(invite_id).bind(uid).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org_id), None, Some(uid), "member.added", json!({ "user_id": uid, "role": role, "via": "invite" })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    let token = match new_token {
        Some(()) => Some(create_session(&s.db, uid, "session", None, None).await?),
        None => None,
    };
    Ok(Json(json!({ "token": token, "user": load_me(&s.db, uid).await? })))
}
