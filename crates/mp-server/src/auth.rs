use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::{hash_token, new_token, slugify};
use crate::AppState;

/// The signed-in person making a request.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub is_server_admin: bool,
    pub session_id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or_else(ApiError::unauthorized)?;
        let row = sqlx::query(
            "SELECT s.id AS sid, s.revoked_at IS NOT NULL AS revoked, u.id, u.name, u.email, u.is_server_admin, u.status
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = $1",
        )
        .bind(hash_token(&token))
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
        // Suspension ends sessions; still tell the person why, rather than just "sign in".
        let status: String = row.get("status");
        if status != "active" {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "suspended", "This account is suspended. Ask an admin of your organization to reactivate it."));
        }
        if row.get::<bool, _>("revoked") {
            return Err(ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "You were signed out. Sign in again to continue."));
        }
        let user = CurrentUser {
            id: row.get("id"),
            name: row.get("name"),
            email: row.get("email"),
            is_server_admin: row.get("is_server_admin"),
            session_id: row.get("sid"),
        };
        // Touch timestamps at most once a minute, off the request path.
        let db = state.db.clone();
        let (sid, uid) = (user.session_id, user.id);
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE sessions SET last_used_at = now() WHERE id = $1 AND (last_used_at IS NULL OR last_used_at < now() - interval '1 minute')")
                .bind(sid)
                .execute(&db)
                .await;
            let _ = sqlx::query("UPDATE users SET last_seen_at = now() WHERE id = $1 AND (last_seen_at IS NULL OR last_seen_at < now() - interval '1 minute')")
                .bind(uid)
                .execute(&db)
                .await;
        });
        Ok(user)
    }
}

/// Token from `Authorization: Bearer`, or from `?token=` (EventSource cannot set headers).
fn bearer(parts: &Parts) -> Option<String> {
    if let Some(h) = parts.headers.get(axum::http::header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        if let Some(t) = h.strip_prefix("Bearer ").or_else(|| h.strip_prefix("bearer ")) {
            return Some(t.trim().to_string());
        }
    }
    parts.uri.query().and_then(|q| {
        q.split('&').find_map(|kv| kv.strip_prefix("token=").map(|t| t.to_string()))
    })
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hashing password: {e}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn check_password_strength(password: &str) -> Result<(), ApiError> {
    if password.chars().count() < 10 {
        return Err(ApiError::invalid("Use a password of at least 10 characters."));
    }
    Ok(())
}

pub async fn create_session(db: &PgPool, user: Uuid, kind: &str, label: Option<&str>, device: Option<&str>) -> sqlx::Result<String> {
    let token = new_token();
    sqlx::query("INSERT INTO sessions (user_id, token_hash, kind, label, device) VALUES ($1,$2,$3,$4,$5)")
        .bind(user)
        .bind(hash_token(&token))
        .bind(kind)
        .bind(label)
        .bind(device)
        .execute(db)
        .await?;
    Ok(token)
}

/// First start: create the server admin and a default organization from MP_ADMIN_EMAIL and
/// MP_ADMIN_PASSWORD. Does nothing once that account exists.
pub async fn bootstrap(state: &AppState) -> anyhow::Result<()> {
    let cfg = &state.config;
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&state.db).await?;
    let (Some(email), Some(password)) = (&cfg.admin_email, &cfg.admin_password) else {
        if users == 0 {
            tracing::warn!("No accounts exist yet. Set MP_ADMIN_EMAIL and MP_ADMIN_PASSWORD and restart to create the first admin.");
        }
        return Ok(());
    };
    let email = email.trim().to_lowercase();
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1").bind(&email).fetch_optional(&state.db).await?;
    if exists.is_some() {
        return Ok(());
    }
    anyhow::ensure!(password.chars().count() >= 10, "MP_ADMIN_PASSWORD must be at least 10 characters");
    let mut tx = state.db.begin().await?;
    let name = email.split('@').next().unwrap_or("Admin").to_string();
    let uid: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, password_hash, is_server_admin) VALUES ($1,$2,$3,TRUE) RETURNING id")
        .bind(&email)
        .bind(&name)
        .bind(hash_password(password)?)
        .fetch_one(&mut *tx)
        .await?;
    let orgs: i64 = sqlx::query_scalar("SELECT count(*) FROM orgs").fetch_one(&mut *tx).await?;
    if orgs == 0 {
        let oid: Uuid = sqlx::query_scalar("INSERT INTO orgs (slug, name) VALUES ($1,$2) RETURNING id")
            .bind(slugify(&cfg.default_org))
            .bind(&cfg.default_org)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO memberships (org_id, user_id, role) VALUES ($1,$2,'owner')").bind(oid).bind(uid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    tracing::info!(%email, "created the first server admin");
    Ok(())
}
