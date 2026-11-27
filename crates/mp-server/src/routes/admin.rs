//! Administration. Server admins see and act on everything; organization owners and admins act
//! on their own organizations and the people in them.

use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, admin_orgs};
use crate::auth::{check_password_strength, hash_password, CurrentUser};
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{claim_from_row, event_from_row, Org, CLAIM_SELECT, EVENT_SELECT};
use crate::routes::account::session_json;
use crate::routes::projects::{project_detail, summary_json, summary_sql};
use crate::util::{hash_token, looks_like_email, new_token};
use crate::AppState;

const ROLES: &[&str] = &["owner", "admin", "member", "guest"];

/// For org admins: the org ids they administer. For server admins: `None` (everything).
async fn scope(s: &AppState, user: &CurrentUser) -> ApiResult<Option<Vec<Uuid>>> {
    admin_orgs(s, user).await
}

fn in_scope(scope: &Option<Vec<Uuid>>, org: Uuid) -> bool {
    scope.as_ref().map(|l| l.contains(&org)).unwrap_or(true)
}

/// May this admin act on that person? Server admins: anyone. Org admins: members of one of
/// their orgs who are not server admins.
async fn check_user_in_scope(s: &AppState, user: &CurrentUser, target: Uuid) -> ApiResult<()> {
    let sc = scope(s, user).await?;
    let is_admin: Option<bool> = sqlx::query_scalar("SELECT is_server_admin FROM users WHERE id = $1").bind(target).fetch_optional(&s.db).await?;
    let is_admin = is_admin.ok_or_else(|| ApiError::not_found("That person"))?;
    if let Some(orgs) = sc {
        if is_admin {
            return Err(ApiError::forbidden("Only server admins can change a server admin's account."));
        }
        let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM memberships WHERE user_id = $1 AND org_id = ANY($2))").bind(target).bind(&orgs).fetch_one(&s.db).await?;
        if !ok {
            return Err(ApiError::not_found("That person"));
        }
    }
    Ok(())
}

pub async fn overview(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    let sc = scope(&s, &user).await?;
    let orgs = sc.clone().unwrap_or_default();
    let all = sc.is_none();
    let r = sqlx::query(
        "SELECT
          (SELECT count(*) FROM users u WHERE $1 OR EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.org_id = ANY($2))) AS users,
          (SELECT count(*) FROM users u WHERE u.status = 'suspended' AND ($1 OR EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.org_id = ANY($2)))) AS suspended,
          (SELECT count(*) FROM orgs o WHERE $1 OR o.id = ANY($2)) AS orgs,
          (SELECT count(*) FROM projects p WHERE p.archived_at IS NULL AND ($1 OR p.org_id = ANY($2))) AS projects,
          (SELECT count(*) FROM claims c JOIN projects p ON p.id = c.project_id WHERE $1 OR p.org_id = ANY($2)) AS active_claims,
          (SELECT count(*) FROM claims c JOIN projects p ON p.id = c.project_id WHERE c.created_at < now() - interval '3 days' AND ($1 OR p.org_id = ANY($2))) AS stale_claims,
          (SELECT count(*) FROM invites i WHERE i.accepted_at IS NULL AND i.revoked_at IS NULL AND (i.expires_at IS NULL OR i.expires_at > now()) AND ($1 OR i.org_id = ANY($2))) AS open_invites,
          (SELECT count(*) FROM chunks) AS chunks,
          (SELECT COALESCE(sum(stored_size), 0)::BIGINT FROM chunks) AS storage_bytes,
          (SELECT COALESCE(sum(size), 0)::BIGINT FROM chunks) AS content_bytes",
    )
    .bind(all)
    .bind(&orgs)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(json!({
        "users": r.get::<i64,_>("users"), "suspended": r.get::<i64,_>("suspended"), "orgs": r.get::<i64,_>("orgs"),
        "projects": r.get::<i64,_>("projects"), "active_claims": r.get::<i64,_>("active_claims"), "stale_claims": r.get::<i64,_>("stale_claims"),
        "open_invites": r.get::<i64,_>("open_invites"),
        "chunks": if all { json!(r.get::<i64,_>("chunks")) } else { Value::Null },
        "storage_bytes": if all { json!(r.get::<i64,_>("storage_bytes")) } else { Value::Null },
        "content_bytes": if all { json!(r.get::<i64,_>("content_bytes")) } else { Value::Null },
        "storage": if all { json!(s.stores.main().description) } else { Value::Null },
        "storage_locations": s.stores.names().count(),
        "is_server_admin": all,
    })))
}

const USER_SELECT: &str = "SELECT u.id, u.email, u.name, u.status, u.is_server_admin, u.created_at, u.last_seen_at, u.suspended_reason,
    (SELECT count(*) FROM claims c WHERE c.user_id = u.id) AS claims,
    (SELECT count(*) FROM sessions se WHERE se.user_id = u.id AND se.revoked_at IS NULL) AS sessions,
    COALESCE((SELECT json_agg(json_build_object('org', json_build_object('id', o.id, 'slug', o.slug, 'name', o.name, 'created_at', o.created_at), 'role', m.role, 'expires_at', m.expires_at) ORDER BY o.name)
              FROM memberships m JOIN orgs o ON o.id = m.org_id WHERE m.user_id = u.id), '[]'::json) AS orgs
    FROM users u ";

fn user_json(r: &PgRow) -> Value {
    json!({
        "id": r.get::<Uuid,_>("id"), "email": r.get::<String,_>("email"), "name": r.get::<String,_>("name"),
        "status": r.get::<String,_>("status"), "is_server_admin": r.get::<bool,_>("is_server_admin"),
        "created_at": r.get::<DateTime<Utc>,_>("created_at"), "last_seen_at": r.get::<Option<DateTime<Utc>>,_>("last_seen_at"),
        "suspended_reason": r.get::<Option<String>,_>("suspended_reason"),
        "claims": r.get::<i64,_>("claims"), "sessions": r.get::<i64,_>("sessions"), "orgs": r.get::<Value,_>("orgs"),
    })
}

async fn load_user(s: &AppState, id: Uuid) -> ApiResult<Value> {
    let sql = format!("{USER_SELECT} WHERE u.id = $1");
    let r = sqlx::query(&sql).bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That person"))?;
    Ok(user_json(&r))
}

#[derive(Deserialize)]
pub struct UsersQuery {
    q: Option<String>,
    org: Option<Uuid>,
    status: Option<String>,
}

pub async fn list_users(State(s): State<AppState>, user: CurrentUser, Query(q): Query<UsersQuery>) -> ApiResult<Json<Vec<Value>>> {
    let sc = scope(&s, &user).await?;
    let like = q.q.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(|x| format!("%{x}%"));
    let sql = format!(
        "{USER_SELECT} WHERE ($1 OR EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.org_id = ANY($2)))
           AND ($3::uuid IS NULL OR EXISTS (SELECT 1 FROM memberships m WHERE m.user_id = u.id AND m.org_id = $3))
           AND ($4::text IS NULL OR u.status = $4)
           AND ($5::text IS NULL OR u.name ILIKE $5 OR u.email ILIKE $5)
         ORDER BY u.name LIMIT 1000"
    );
    let rows = sqlx::query(&sql).bind(sc.is_none()).bind(sc.unwrap_or_default()).bind(q.org).bind(q.status).bind(like).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(user_json).collect()))
}

#[derive(Deserialize)]
pub struct CreateUser {
    email: String,
    name: String,
    password: String,
    org: Option<Uuid>,
    role: Option<String>,
}

/// Create an account directly (for studios that do not use invites). Org admins must put the
/// person in one of their organizations.
pub async fn create_user(State(s): State<AppState>, user: CurrentUser, Json(b): Json<CreateUser>) -> ApiResult<Json<Value>> {
    let sc = scope(&s, &user).await?;
    let email = b.email.trim().to_lowercase();
    if !looks_like_email(&email) {
        return Err(ApiError::invalid("Enter a valid email address."));
    }
    let name = b.name.trim();
    if name.is_empty() {
        return Err(ApiError::invalid("Enter the person's name."));
    }
    check_password_strength(&b.password)?;
    let role = b.role.clone().unwrap_or_else(|| "member".into());
    if !ROLES.contains(&role.as_str()) {
        return Err(ApiError::invalid("Role must be owner, admin, member or guest."));
    }
    if sc.is_some() && b.org.map(|o| !in_scope(&sc, o)).unwrap_or(true) {
        return Err(ApiError::forbidden("Choose one of your organizations for this person."));
    }
    let mut tx = s.db.begin().await?;
    let uid: Uuid = sqlx::query_scalar("INSERT INTO users (email, name, password_hash) VALUES ($1,$2,$3) RETURNING id")
        .bind(&email)
        .bind(name)
        .bind(hash_password(&b.password)?)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict("An account with that email already exists."),
            err => err,
        })?;
    if let Some(org) = b.org {
        sqlx::query("INSERT INTO memberships (org_id, user_id, role) VALUES ($1,$2,$3)").bind(org).bind(uid).bind(&role).execute(&mut *tx).await?;
    }
    let ev = events::record(&mut *tx, b.org, None, Some(user.id), "member.added", json!({ "user_id": uid, "email": email, "role": role })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    load_user(&s, uid).await.map(Json)
}

pub async fn get_user(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    check_user_in_scope(&s, &user, id).await?;
    let mut v = load_user(&s, id).await?;
    let sessions: Vec<Value> = sqlx::query("SELECT id, kind, label, device, created_at, last_used_at FROM sessions WHERE user_id = $1 AND revoked_at IS NULL ORDER BY last_used_at DESC NULLS LAST")
        .bind(id)
        .fetch_all(&s.db)
        .await?
        .iter()
        .map(session_json)
        .collect();
    let sql = format!("{CLAIM_SELECT} WHERE cl.user_id = $1 ORDER BY cl.created_at");
    let claims: Vec<_> = sqlx::query(&sql).bind(id).fetch_all(&s.db).await?.iter().map(|r| claim_from_row(r, true)).collect();
    v["sessions"] = json!(sessions);
    v["claim_count"] = v["claims"].clone();
    v["claims"] = serde_json::to_value(claims).unwrap_or_default();
    Ok(Json(v))
}

#[derive(Deserialize, Default)]
pub struct SuspendBody {
    reason: Option<String>,
}

/// Suspend: no sign-in, no sync, sessions end now. Claims stay so nobody's work is overwritten.
pub async fn suspend_user(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, body: Option<Json<SuspendBody>>) -> ApiResult<Json<Value>> {
    check_user_in_scope(&s, &user, id).await?;
    if id == user.id {
        return Err(ApiError::invalid("You cannot suspend your own account."));
    }
    let reason = body.and_then(|Json(b)| b.reason).filter(|r| !r.trim().is_empty());
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE users SET status = 'suspended', suspended_reason = $2 WHERE id = $1").bind(id).bind(&reason).execute(&mut *tx).await?;
    let revoked = sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL").bind(id).execute(&mut *tx).await?.rows_affected();
    let ev = events::record(&mut *tx, None, None, Some(user.id), "member.suspended", json!({ "user_id": id, "reason": reason, "sessions_ended": revoked })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    load_user(&s, id).await.map(Json)
}

pub async fn reactivate_user(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    check_user_in_scope(&s, &user, id).await?;
    sqlx::query("UPDATE users SET status = 'active', suspended_reason = NULL WHERE id = $1").bind(id).execute(&s.db).await?;
    events::emit(&s.db, &s.events, None, None, Some(user.id), "member.reactivated", json!({ "user_id": id })).await;
    load_user(&s, id).await.map(Json)
}

pub async fn sign_out_user(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    check_user_in_scope(&s, &user, id).await?;
    let n = sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL").bind(id).execute(&s.db).await?.rows_affected();
    events::emit(&s.db, &s.events, None, None, Some(user.id), "admin.signed_out", json!({ "user_id": id, "sessions": n })).await;
    Ok(Json(json!({ "revoked": n })))
}

#[derive(Deserialize)]
pub struct PasswordBody {
    password: String,
}

pub async fn set_password(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<PasswordBody>) -> ApiResult<Json<Value>> {
    check_user_in_scope(&s, &user, id).await?;
    check_password_strength(&b.password)?;
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1").bind(id).bind(hash_password(&b.password)?).execute(&mut *tx).await?;
    sqlx::query("UPDATE sessions SET revoked_at = now() WHERE user_id = $1 AND kind = 'session' AND revoked_at IS NULL").bind(id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, None, None, Some(user.id), "admin.password_reset", json!({ "user_id": id })).await?;
    let email: String = sqlx::query_scalar("SELECT email FROM users WHERE id = $1").bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    // A new password ends any wait from wrong guesses at the old one.
    s.sign_in.clear(&format!("account:{email}"));
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct DeleteUserBody {
    confirm: String,
    transfer_to: Option<Uuid>,
}

/// Delete an account (server admins). Claims and drafts go to `transfer_to` when given, are
/// released otherwise. Published changes stay; their author shows as a deleted account.
pub async fn delete_user(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<DeleteUserBody>) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    if id == user.id {
        return Err(ApiError::invalid("You cannot delete your own account."));
    }
    let name: String = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That person"))?;
    if b.confirm.trim() != name {
        return Err(ApiError::invalid(format!("Type the person's name exactly ({name}) to confirm.")));
    }
    let mut tx = s.db.begin().await?;
    if let Some(to) = b.transfer_to {
        sqlx::query("UPDATE claims SET user_id = $2 WHERE user_id = $1").bind(id).bind(to).execute(&mut *tx).await?;
        sqlx::query("UPDATE drafts SET user_id = $2 WHERE user_id = $1").bind(id).bind(to).execute(&mut *tx).await?;
    }
    sqlx::query("DELETE FROM users WHERE id = $1").bind(id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, None, None, Some(user.id), "admin.user_deleted", json!({ "user_id": id, "name": name, "transfer_to": b.transfer_to })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct AddMember {
    user: Uuid,
    role: String,
}

/// Add someone who already has an account to this organization. Org admins can add people they
/// can already see (someone in one of their organizations); server admins can add anyone.
pub async fn add_member(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<AddMember>) -> ApiResult<Json<Value>> {
    may_add(&s, &user, org, &b.role).await?;
    if let Some(mine) = admin_orgs(&s, &user).await? {
        let known: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM memberships WHERE user_id = $1 AND org_id = ANY($2))")
            .bind(b.user)
            .bind(&mine)
            .fetch_one(&s.db)
            .await?;
        if !known {
            return Err(ApiError::not_found("That person"));
        }
    }
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM users WHERE id = $1").bind(b.user).fetch_optional(&s.db).await?;
    let Some(name) = name else { return Err(ApiError::not_found("That person")) };
    let mut tx = s.db.begin().await?;
    let added = sqlx::query("INSERT INTO memberships (org_id, user_id, role) VALUES ($1,$2,$3) ON CONFLICT (org_id, user_id) DO NOTHING")
        .bind(org)
        .bind(b.user)
        .bind(&b.role)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if added == 0 {
        return Err(ApiError::conflict(format!("{name} is already in this organization. Change their role instead.")));
    }
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "member.added", json!({ "user_id": b.user, "role": b.role, "via": "admin" })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "user": b.user, "role": b.role })))
}

pub async fn list_members(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::org_admin(&s, &user, org).await?;
    let rows = sqlx::query(
        "SELECT m.role, m.expires_at, m.created_at, u.id, u.name, u.email, u.status, u.last_seen_at
         FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.org_id = $1 ORDER BY u.name",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.iter().map(member_json).collect()))
}

fn member_json(r: &PgRow) -> Value {
    json!({
        "user": { "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "email": r.get::<String,_>("email"),
                  "status": r.get::<String,_>("status"), "last_seen_at": r.get::<Option<DateTime<Utc>>,_>("last_seen_at") },
        "role": r.get::<String,_>("role"), "expires_at": r.get::<Option<DateTime<Utc>>,_>("expires_at"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
    })
}

#[derive(Deserialize)]
pub struct UpdateMember {
    role: Option<String>,
    /// RFC 3339 time, or null to clear. Absent leaves it unchanged.
    #[serde(default, with = "double_option")]
    expires_at: Option<Option<DateTime<Utc>>>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
        Ok(Some(Option::deserialize(d)?))
    }
}

pub async fn update_member(State(s): State<AppState>, user: CurrentUser, Path((org, uid)): Path<(Uuid, Uuid)>, Json(b): Json<UpdateMember>) -> ApiResult<Json<Value>> {
    let my_role = access::org_admin(&s, &user, org).await?;
    let mut tx = s.db.begin().await?;
    let current: String = sqlx::query_scalar("SELECT role FROM memberships WHERE org_id = $1 AND user_id = $2 FOR UPDATE")
        .bind(org)
        .bind(uid)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::not_found("That member"))?;
    if let Some(role) = &b.role {
        if !ROLES.contains(&role.as_str()) {
            return Err(ApiError::invalid("Role must be owner, admin, member or guest."));
        }
        if (role == "owner" || current == "owner") && my_role != "owner" {
            return Err(ApiError::forbidden("Only owners can make or change owners."));
        }
        if current == "owner" && role != "owner" {
            let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE org_id = $1 AND role = 'owner'").bind(org).fetch_one(&mut *tx).await?;
            if owners <= 1 {
                return Err(ApiError::conflict("An organization needs at least one owner. Make someone else an owner first."));
            }
        }
        sqlx::query("UPDATE memberships SET role = $3 WHERE org_id = $1 AND user_id = $2").bind(org).bind(uid).bind(role).execute(&mut *tx).await?;
    }
    if let Some(exp) = b.expires_at {
        sqlx::query("UPDATE memberships SET expires_at = $3 WHERE org_id = $1 AND user_id = $2").bind(org).bind(uid).bind(exp).execute(&mut *tx).await?;
    }
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.member_changed", json!({ "user_id": uid, "role": b.role })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    let r = sqlx::query("SELECT m.role, m.expires_at, m.created_at, u.id, u.name, u.email, u.status, u.last_seen_at FROM memberships m JOIN users u ON u.id = m.user_id WHERE m.org_id = $1 AND m.user_id = $2")
        .bind(org)
        .bind(uid)
        .fetch_one(&s.db)
        .await?;
    Ok(Json(member_json(&r)))
}

/// Revoke from an organization: membership and project access go; their claims in this org's
/// projects are released; published work stays attributed to them.
pub async fn remove_member(State(s): State<AppState>, user: CurrentUser, Path((org, uid)): Path<(Uuid, Uuid)>) -> ApiResult<Json<Value>> {
    let my_role = access::org_admin(&s, &user, org).await?;
    let mut tx = s.db.begin().await?;
    let current: String = sqlx::query_scalar("SELECT role FROM memberships WHERE org_id = $1 AND user_id = $2")
        .bind(org)
        .bind(uid)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::not_found("That member"))?;
    if current == "owner" {
        if my_role != "owner" {
            return Err(ApiError::forbidden("Only owners can remove an owner."));
        }
        let owners: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE org_id = $1 AND role = 'owner'").bind(org).fetch_one(&mut *tx).await?;
        if owners <= 1 {
            return Err(ApiError::conflict("This is the organization's only owner. Make someone else an owner first."));
        }
    }
    sqlx::query("DELETE FROM memberships WHERE org_id = $1 AND user_id = $2").bind(org).bind(uid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM project_access pa USING projects p WHERE pa.project_id = p.id AND p.org_id = $1 AND pa.user_id = $2").bind(org).bind(uid).execute(&mut *tx).await?;
    let released = sqlx::query("DELETE FROM claims c USING projects p WHERE c.project_id = p.id AND p.org_id = $1 AND c.user_id = $2").bind(org).bind(uid).execute(&mut *tx).await?.rows_affected();
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "member.removed", json!({ "user_id": uid, "claims_released": released })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "claims_released": released })))
}

fn invite_url(s: &AppState, token: &str) -> String {
    format!("{}/invite/{}", s.config.public_url.trim_end_matches('/'), token)
}

pub async fn list_invites(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::org_admin(&s, &user, org).await?;
    let rows = sqlx::query(
        "SELECT i.id, i.email, i.role, i.expires_at, i.created_at, i.accepted_at, i.revoked_at, i.token, i.also, u.id AS u_id, u.name AS u_name, u.email AS u_email
         FROM invites i LEFT JOIN users u ON u.id = i.created_by WHERE i.org_id = $1 AND i.revoked_at IS NULL ORDER BY i.created_at DESC",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.iter().map(|r| invite_json(r, r.get::<Option<String>, _>("token").map(|t| invite_url(&s, &t)))).collect()))
}

fn invite_json(r: &PgRow, url: Option<String>) -> Value {
    json!({
        "id": r.get::<Uuid,_>("id"), "email": r.get::<Option<String>,_>("email"), "role": r.get::<String,_>("role"),
        "expires_at": r.get::<Option<DateTime<Utc>>,_>("expires_at"), "created_at": r.get::<DateTime<Utc>,_>("created_at"),
        "accepted_at": r.get::<Option<DateTime<Utc>>,_>("accepted_at"),
        "created_by": crate::models::UserRef::from_row(r, "u_"),
        "also": r.try_get::<Value, _>("also").unwrap_or_else(|_| json!([])),
        "url": url,
    })
}

#[derive(Deserialize)]
pub struct CreateInvites {
    #[serde(default)]
    emails: Vec<String>,
    role: String,
    expires_in_days: Option<i64>,
    /// More organizations to join with the same link, each with its own role.
    #[serde(default)]
    also: Vec<AlsoOrg>,
}

#[derive(Deserialize)]
pub struct AlsoOrg {
    org: Uuid,
    role: String,
}

/// Check the caller may add people to `org` as `role`, and say what they may not.
async fn may_add(s: &AppState, user: &CurrentUser, org: Uuid, role: &str) -> ApiResult<()> {
    let mine = access::org_admin(s, user, org).await?;
    if !ROLES.contains(&role) {
        return Err(ApiError::invalid("Role must be owner, admin, member or guest."));
    }
    if role == "owner" && mine != "owner" {
        return Err(ApiError::forbidden("Only owners can add owners."));
    }
    Ok(())
}

/// Invite by email (one link per address) or create one open link when no emails are given.
pub async fn create_invites(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<CreateInvites>) -> ApiResult<Json<Vec<Value>>> {
    may_add(&s, &user, org, &b.role).await?;
    let mut also: Vec<Value> = Vec::new();
    for a in &b.also {
        if a.org == org || also.iter().any(|x| x["org_id"] == json!(a.org)) {
            continue;
        }
        may_add(&s, &user, a.org, &a.role).await?;
        also.push(json!({ "org_id": a.org, "role": a.role }));
    }
    let also = Value::Array(also);
    // 0 means the link never expires.
    let days = b.expires_in_days.unwrap_or(14);
    let expires = (days > 0).then(|| Utc::now() + Duration::days(days.clamp(1, 365)));
    let mut emails: Vec<Option<String>> = Vec::new();
    for e in &b.emails {
        for part in e.split([',', ';', '\n', ' ', '\t']) {
            let p = part.trim().to_lowercase();
            if p.is_empty() {
                continue;
            }
            if !looks_like_email(&p) {
                return Err(ApiError::invalid(format!("{p} does not look like an email address.")));
            }
            if !emails.contains(&Some(p.clone())) {
                emails.push(Some(p));
            }
        }
    }
    if emails.is_empty() {
        emails.push(None);
    }
    if emails.len() > 500 {
        return Err(ApiError::invalid("Invite at most 500 people at a time."));
    }
    let mut out = Vec::new();
    let mut tx = s.db.begin().await?;
    for email in emails {
        let token = new_token();
        let r = sqlx::query(
            "WITH ins AS (INSERT INTO invites (org_id, email, role, token_hash, expires_at, created_by, token, also) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, email, role, expires_at, created_at, accepted_at, revoked_at, created_by, also)
             SELECT ins.*, u.id AS u_id, u.name AS u_name, u.email AS u_email FROM ins LEFT JOIN users u ON u.id = ins.created_by",
        )
        .bind(org)
        .bind(&email)
        .bind(&b.role)
        .bind(hash_token(&token))
        .bind(expires)
        .bind(user.id)
        .bind(&token)
        .bind(&also)
        .fetch_one(&mut *tx)
        .await?;
        let mut j = invite_json(&r, Some(invite_url(&s, &token)));
        j["emailed"] = json!(email.is_some() && s.config.email.is_some());
        out.push((j, email.map(|e| (e, invite_url(&s, &token)))));
    }
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.invites_created", json!({ "count": out.len(), "role": b.role })).await?;
    let mut ids = vec![org];
    ids.extend(b.also.iter().map(|a| a.org).filter(|o| *o != org));
    let names: Vec<String> = sqlx::query_scalar("SELECT name FROM orgs WHERE id = ANY($1) ORDER BY array_position($1, id)").bind(&ids).fetch_all(&mut *tx).await?;
    let org_name = match names.as_slice() {
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        [] => String::new(),
    };
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    if let Some(mail) = &s.config.email {
        for (_, to) in &out {
            if let Some((addr, url)) = to {
                mail.send_later(
                    addr.clone(),
                    format!("{} invited you to {org_name} on MultiParity", user.name),
                    format!("{} invited you to join {org_name} on MultiParity, where the team shares its project files.

Open this link to make your account{}:

{url}

Then get the desktop app from the account menu to keep projects on your computer.", user.name,
                        if days > 0 { format!(" (it works for {days} days)") } else { String::new() }),
                );
            }
        }
    }
    Ok(Json(out.into_iter().map(|(j, _)| j).collect()))
}

pub async fn revoke_invite(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let org: Uuid = sqlx::query_scalar("SELECT org_id FROM invites WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That invite"))?;
    access::org_admin(&s, &user, org).await?;
    sqlx::query("UPDATE invites SET revoked_at = now(), token = NULL WHERE id = $1").bind(id).execute(&s.db).await?;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct UpdateOrg {
    name: Option<String>,
    /// Theme ids for light and dark mode; "" clears one (back to the apps' default).
    theme_light: Option<String>,
    theme_dark: Option<String>,
}

/// A theme id from the apps' list (letters, digits, dashes), or None for "".
fn theme_value(v: &str) -> ApiResult<Option<String>> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if v.len() > 40 || !v.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err(ApiError::invalid(format!("{v} is not a theme.")));
    }
    Ok(Some(v.to_string()))
}

pub async fn update_org(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<UpdateOrg>) -> ApiResult<Json<Org>> {
    access::org_admin(&s, &user, org).await?;
    if let Some(name) = b.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        sqlx::query("UPDATE orgs SET name = $2 WHERE id = $1").bind(org).bind(name).execute(&s.db).await?;
    }
    if let Some(t) = b.theme_light.as_deref() {
        sqlx::query("UPDATE orgs SET theme_light = $2 WHERE id = $1").bind(org).bind(theme_value(t)?).execute(&s.db).await?;
    }
    if let Some(t) = b.theme_dark.as_deref() {
        sqlx::query("UPDATE orgs SET theme_dark = $2 WHERE id = $1").bind(org).bind(theme_value(t)?).execute(&s.db).await?;
    }
    let r = sqlx::query("SELECT id, slug, name, created_at, theme_light, theme_dark FROM orgs WHERE id = $1").bind(org).fetch_one(&s.db).await?;
    Ok(Json(Org::from_row(&r)))
}

/// Delete an organization that has no projects left (server admins).
pub async fn delete_org(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE org_id = $1").bind(org).fetch_one(&s.db).await?;
    if projects > 0 {
        return Err(ApiError::conflict(format!("This organization still has {projects} project(s). Move or delete them first.")));
    }
    sqlx::query("DELETE FROM orgs WHERE id = $1").bind(org).execute(&s.db).await?;
    events::emit(&s.db, &s.events, None, None, Some(user.id), "admin.org_deleted", json!({ "org_id": org })).await;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct ProjectsQuery {
    org: Option<Uuid>,
}

pub async fn list_projects(State(s): State<AppState>, user: CurrentUser, Query(q): Query<ProjectsQuery>) -> ApiResult<Json<Vec<Value>>> {
    let sc = scope(&s, &user).await?;
    let sql = summary_sql("($2 OR p.org_id = ANY($3)) AND ($4::uuid IS NULL OR p.org_id = $4)");
    let rows = sqlx::query(&sql).bind(user.id).bind(sc.is_none()).bind(sc.unwrap_or_default()).bind(q.org).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(summary_json).collect()))
}

#[derive(Deserialize)]
pub struct UpdateProject {
    name: Option<String>,
    org: Option<Uuid>,
    /// Where checkpoints are kept: "auto" (by project size), "server" or "local".
    checkpoints: Option<String>,
    /// Take checkpoints automatically every few minutes while people work (off by default).
    auto_checkpoints: Option<bool>,
    /// Theme ids for light and dark mode; "" clears one (back to the organization's).
    theme_light: Option<String>,
    theme_dark: Option<String>,
}

pub async fn update_project(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<UpdateProject>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, access::Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    let mut tx = s.db.begin().await?;
    if let Some(on) = b.auto_checkpoints {
        sqlx::query("UPDATE projects SET auto_checkpoints = $2 WHERE id = $1").bind(p).bind(on).execute(&mut *tx).await?;
    }
    if let Some(mode) = b.checkpoints.as_deref() {
        let value = match mode {
            "auto" => None,
            "server" => Some(true),
            "local" => Some(false),
            _ => return Err(ApiError::invalid("Checkpoints can be kept: auto, server or local.")),
        };
        sqlx::query("UPDATE projects SET checkpoints_on_server = $2 WHERE id = $1").bind(p).bind(value).execute(&mut *tx).await?;
    }
    if let Some(name) = b.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        sqlx::query("UPDATE projects SET name = $2 WHERE id = $1").bind(p).bind(name).execute(&mut *tx).await?;
    }
    if let Some(t) = b.theme_light.as_deref() {
        sqlx::query("UPDATE projects SET theme_light = $2 WHERE id = $1").bind(p).bind(theme_value(t)?).execute(&mut *tx).await?;
    }
    if let Some(t) = b.theme_dark.as_deref() {
        sqlx::query("UPDATE projects SET theme_dark = $2 WHERE id = $1").bind(p).bind(theme_value(t)?).execute(&mut *tx).await?;
    }
    if let Some(to) = b.org.filter(|o| *o != acc.org_id) {
        access::org_admin(&s, &user, to).await?;
        sqlx::query("UPDATE projects SET org_id = $2 WHERE id = $1").bind(p).bind(to).execute(&mut *tx).await.map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict("The other organization already has a project with this short name. Rename one first."),
            err => err,
        })?;
        // Guests' explicit access belonged to the old organization.
        sqlx::query("DELETE FROM project_access WHERE project_id = $1").bind(p).execute(&mut *tx).await?;
        sqlx::query("UPDATE tags SET org_id = $2 WHERE project_id = $1").bind(p).bind(to).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM path_tags pt USING tags t WHERE pt.tag_id = t.id AND pt.project_id = $1 AND t.project_id IS NULL").bind(p).execute(&mut *tx).await?;
    }
    let ev = events::record(&mut *tx, Some(b.org.unwrap_or(acc.org_id)), Some(p), Some(user.id), "admin.project_changed", json!({ "name": b.name, "org": b.org, "checkpoints": b.checkpoints, "auto_checkpoints": b.auto_checkpoints })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    project_detail(&s, &user, p).await.map(Json)
}

async fn set_archived(s: &AppState, user: &CurrentUser, p: Uuid, archived: bool) -> ApiResult<Value> {
    let acc = access::project(s, user, p, access::Level::Read).await?;
    access::org_admin(s, user, acc.org_id).await?;
    sqlx::query(if archived { "UPDATE projects SET archived_at = now() WHERE id = $1" } else { "UPDATE projects SET archived_at = NULL WHERE id = $1" })
        .bind(p)
        .execute(&s.db)
        .await?;
    events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), if archived { "project.archived" } else { "project.restored" }, json!({})).await;
    project_detail(s, user, p).await
}

pub async fn archive_project(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Value>> {
    set_archived(&s, &user, p, true).await.map(Json)
}

pub async fn restore_project(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Value>> {
    set_archived(&s, &user, p, false).await.map(Json)
}

pub async fn list_project_access(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    let acc = access::project(&s, &user, p, access::Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    let rows = sqlx::query("SELECT u.id, u.name, u.email, pa.can_write FROM project_access pa JOIN users u ON u.id = pa.user_id WHERE pa.project_id = $1 ORDER BY u.name")
        .bind(p)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows.iter().map(|r| json!({ "user": { "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "email": r.get::<String,_>("email") }, "can_write": r.get::<bool,_>("can_write") })).collect()))
}

#[derive(Deserialize)]
pub struct AccessBody {
    user: Uuid,
    /// `write`, `read` or `none`.
    access: String,
}

/// Give a guest access to one project (members already see every project in the org).
pub async fn set_project_access(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>, Json(b): Json<AccessBody>) -> ApiResult<Json<Value>> {
    let acc = access::project(&s, &user, p, access::Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    match b.access.as_str() {
        "none" => {
            sqlx::query("DELETE FROM project_access WHERE project_id = $1 AND user_id = $2").bind(p).bind(b.user).execute(&s.db).await?;
        }
        "read" | "write" => {
            sqlx::query("INSERT INTO project_access (project_id, user_id, can_write) VALUES ($1,$2,$3) ON CONFLICT (project_id, user_id) DO UPDATE SET can_write = EXCLUDED.can_write")
                .bind(p)
                .bind(b.user)
                .bind(b.access == "write")
                .execute(&s.db)
                .await?;
        }
        _ => return Err(ApiError::invalid("Access must be write, read or none.")),
    }
    events::emit(&s.db, &s.events, Some(acc.org_id), Some(p), Some(user.id), "admin.access_changed", json!({ "user_id": b.user, "access": b.access })).await;
    Ok(Json(json!({})))
}

#[derive(Deserialize)]
pub struct ClaimsQuery {
    older_than_hours: Option<i64>,
    org: Option<Uuid>,
    suspended_only: Option<bool>,
}

/// Claims to look at: old ones, and ones held by suspended people.
pub async fn list_claims(State(s): State<AppState>, user: CurrentUser, Query(q): Query<ClaimsQuery>) -> ApiResult<Json<Vec<Value>>> {
    let sc = scope(&s, &user).await?;
    let sql = format!(
        "{CLAIM_SELECT} WHERE ($1 OR p.org_id = ANY($2)) AND ($3::uuid IS NULL OR p.org_id = $3)
           AND ($4::bigint IS NULL OR cl.created_at < now() - make_interval(hours => $4::int))
           AND (NOT $5 OR u.status = 'suspended')
         ORDER BY cl.created_at LIMIT 2000"
    );
    let rows = sqlx::query(&sql)
        .bind(sc.is_none())
        .bind(sc.unwrap_or_default())
        .bind(q.org)
        .bind(q.older_than_hours)
        .bind(q.suspended_only.unwrap_or(false))
        .fetch_all(&s.db)
        .await?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                let mut v = serde_json::to_value(claim_from_row(r, true)).unwrap_or_default();
                v["age_hours"] = json!((Utc::now() - r.get::<DateTime<Utc>, _>("created_at")).num_hours());
                v
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct ReleaseItem {
    project: Uuid,
    branch: Uuid,
    path: String,
}

#[derive(Deserialize)]
pub struct ReleaseClaims {
    claims: Vec<ReleaseItem>,
}

pub async fn release_claims(State(s): State<AppState>, user: CurrentUser, Json(b): Json<ReleaseClaims>) -> ApiResult<Json<Value>> {
    let sc = scope(&s, &user).await?;
    let mut released = 0u64;
    let mut by_project: HashMap<Uuid, Vec<String>> = HashMap::new();
    for c in &b.claims {
        let org: Option<Uuid> = sqlx::query_scalar("SELECT org_id FROM projects WHERE id = $1").bind(c.project).fetch_optional(&s.db).await?;
        let Some(org) = org else { continue };
        if !in_scope(&sc, org) {
            continue;
        }
        let n = sqlx::query("DELETE FROM claims WHERE project_id = $1 AND branch_id = $2 AND path = $3").bind(c.project).bind(c.branch).bind(&c.path).execute(&s.db).await?.rows_affected();
        if n > 0 {
            released += n;
            by_project.entry(c.project).or_default().push(c.path.clone());
        }
    }
    for (p, paths) in by_project {
        let org: Option<Uuid> = sqlx::query_scalar("SELECT org_id FROM projects WHERE id = $1").bind(p).fetch_optional(&s.db).await?;
        events::emit(&s.db, &s.events, org, Some(p), Some(user.id), "claim.released", json!({ "paths": paths, "by_admin": true })).await;
    }
    Ok(Json(json!({ "released": released })))
}

#[derive(Deserialize)]
pub struct CheckBody {
    project: Option<Uuid>,
}

/// Verify that every chunk referenced by a project's history (or every project's) exists in
/// storage. Reports missing chunks and the changes they belong to.
pub async fn storage_check(State(s): State<AppState>, user: CurrentUser, Json(b): Json<CheckBody>) -> ApiResult<Json<Value>> {
    let sc = scope(&s, &user).await?;
    if let Some(p) = b.project {
        let acc = access::project(&s, &user, p, access::Level::Read).await?;
        access::org_admin(&s, &user, acc.org_id).await?;
    } else if sc.is_some() {
        return Err(ApiError::invalid("Choose a project to check."));
    }
    let rows = sqlx::query(
        "WITH refm AS (
           SELECT DISTINCT project_id AS p, manifest_hash AS h FROM change_files WHERE manifest_hash IS NOT NULL AND ($1::uuid IS NULL OR project_id = $1)
           UNION SELECT DISTINCT b.project_id, t.manifest_hash FROM tree_entries t JOIN branches b ON b.id = t.branch_id WHERE t.manifest_hash IS NOT NULL AND ($1::uuid IS NULL OR b.project_id = $1))
         SELECT DISTINCT pr.storage AS loc, pr.storage_previous AS prev, jsonb_array_elements(m.chunks)->>0 AS chunk
         FROM manifests m JOIN refm ON refm.h = m.hash JOIN projects pr ON pr.id = refm.p",
    )
    .bind(b.project)
    .fetch_all(&s.db)
    .await?;
    let chunks: Vec<(String, Option<String>, String)> = rows.iter().map(|r| (r.get("loc"), r.get("prev"), r.get("chunk"))).collect();
    let checked = chunks.len();
    let state = s.clone();
    let results: Vec<(String, bool)> = futures::stream::iter(chunks)
        .map(|(loc, prev, h)| {
            let state = state.clone();
            async move {
                let mut ok = false;
                for l in std::iter::once(loc).chain(prev) {
                    if let Some(st) = state.stores.get(&l) {
                        if st.has_chunk(&h).await.unwrap_or(false) {
                            ok = true;
                            break;
                        }
                    }
                }
                (h, ok)
            }
        })
        .buffer_unordered(32)
        .collect()
        .await;
    let mut missing: Vec<String> = results.into_iter().filter(|(_, ok)| !ok).map(|(h, _)| h).collect();
    missing.sort();
    missing.dedup();
    let mut details = Vec::new();
    for h in missing.iter().take(200) {
        let changes: Vec<(Uuid, i32, String)> = sqlx::query_as(
            "SELECT DISTINCT cf.project_id, cf.change_number, cf.path FROM change_files cf JOIN manifests m ON m.hash = cf.manifest_hash
             WHERE m.chunks @> jsonb_build_array(jsonb_build_array($1::text)) AND ($2::uuid IS NULL OR cf.project_id = $2) ORDER BY cf.change_number LIMIT 50",
        )
        .bind(h)
        .bind(b.project)
        .fetch_all(&s.db)
        .await?;
        details.push(json!({ "hash": h, "changes": changes.iter().map(|(p, n, path)| json!({ "project": p, "number": n, "path": path })).collect::<Vec<_>>() }));
    }
    events::emit(&s.db, &s.events, None, b.project, Some(user.id), "admin.storage_checked", json!({ "checked": checked, "missing": missing.len() })).await;
    Ok(Json(json!({ "checked": checked, "missing": details, "missing_count": missing.len(), "ok": missing.is_empty() })))
}

/// Copies of chunks nothing needs: in a location where no project stored there (or still being
/// moved out of it) uses them, and older than a day so uploads in progress are kept. After a
/// project moves, its old copies show up here.
const UNREFERENCED: &str = "WITH refm AS (
      SELECT b.project_id AS p, t.manifest_hash AS h FROM tree_entries t JOIN branches b ON b.id = t.branch_id WHERE t.manifest_hash IS NOT NULL
      UNION SELECT project_id, manifest_hash FROM change_files WHERE manifest_hash IS NOT NULL
      UNION SELECT d.project_id, e->'manifest'->>'hash' FROM drafts d, jsonb_array_elements(d.entries) e WHERE e ? 'manifest' AND jsonb_typeof(e->'manifest') = 'object'
      UNION SELECT k.project_id, e->'manifest'->>'hash' FROM checkpoints k, jsonb_array_elements(k.entries) e WHERE e ? 'manifest' AND jsonb_typeof(e->'manifest') = 'object'),
    locs AS (SELECT id, storage AS loc FROM projects UNION SELECT id, storage_previous FROM projects WHERE storage_previous IS NOT NULL),
    refc AS (SELECT DISTINCT locs.loc, jsonb_array_elements(m.chunks)->>0 AS h FROM refm JOIN manifests m ON m.hash = refm.h JOIN locs ON locs.id = refm.p)
    SELECT c.location, c.hash, c.stored_size FROM chunks c
    WHERE c.created_at < now() - interval '1 day'
      AND NOT EXISTS (SELECT 1 FROM refc WHERE refc.loc = c.location AND refc.h = c.hash)
      AND NOT EXISTS (SELECT 1 FROM storage_moves sm WHERE sm.status = 'running' AND (sm.from_location = c.location OR sm.to_location = c.location))";

/// Dry run: how much space unreferenced chunks use. Chunks younger than a day are kept, so
/// uploads that are still in progress are never collected.
pub async fn gc_preview(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let rows = sqlx::query(UNREFERENCED).fetch_all(&s.db).await?;
    let bytes: i64 = rows.iter().map(|r| r.get::<i64, _>("stored_size")).sum();
    let mut by_location: std::collections::BTreeMap<String, (usize, i64)> = Default::default();
    for r in &rows {
        let e = by_location.entry(r.get("location")).or_default();
        e.0 += 1;
        e.1 += r.get::<i64, _>("stored_size");
    }
    let by: Vec<Value> = by_location.into_iter().map(|(l, (n, b))| json!({ "location": l, "chunks": n, "bytes": b })).collect();
    Ok(Json(json!({ "unreferenced_chunks": rows.len(), "bytes": bytes, "by_location": by })))
}

pub async fn gc_run(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let rows = sqlx::query(UNREFERENCED).fetch_all(&s.db).await?;
    let mut deleted = 0usize;
    let mut bytes = 0i64;
    for r in &rows {
        let h: String = r.get("hash");
        let loc: String = r.get("location");
        // A location no longer configured cannot be cleaned; leave its rows for when it returns.
        let Some(store) = s.stores.get(&loc) else { continue };
        store.delete_chunk(&h).await?;
        sqlx::query("DELETE FROM chunks WHERE location = $1 AND hash = $2").bind(&loc).bind(&h).execute(&s.db).await?;
        deleted += 1;
        bytes += r.get::<i64, _>("stored_size");
    }
    sqlx::query(
        "DELETE FROM manifests m WHERE m.created_at < now() - interval '1 day'
           AND NOT EXISTS (SELECT 1 FROM tree_entries t WHERE t.manifest_hash = m.hash)
           AND NOT EXISTS (SELECT 1 FROM change_files cf WHERE cf.manifest_hash = m.hash)
           AND NOT EXISTS (SELECT 1 FROM drafts d, jsonb_array_elements(d.entries) e WHERE e->'manifest'->>'hash' = m.hash)
           AND NOT EXISTS (SELECT 1 FROM checkpoints k, jsonb_array_elements(k.entries) e WHERE e->'manifest'->>'hash' = m.hash)",
    )
    .execute(&s.db)
    .await?;
    events::emit(&s.db, &s.events, None, None, Some(user.id), "admin.gc", json!({ "deleted": deleted, "bytes": bytes })).await;
    Ok(Json(json!({ "deleted": deleted, "bytes": bytes })))
}

#[derive(Deserialize)]
pub struct AuditQuery {
    org: Option<Uuid>,
    project: Option<Uuid>,
    actor: Option<Uuid>,
    kind: Option<String>,
    before: Option<i64>,
    limit: Option<i64>,
}

pub async fn audit(State(s): State<AppState>, user: CurrentUser, Query(q): Query<AuditQuery>) -> ApiResult<Json<Vec<Value>>> {
    let sc = scope(&s, &user).await?;
    let sql = format!(
        "{EVENT_SELECT} WHERE ($1 OR e.org_id = ANY($2) OR e.project_id IN (SELECT id FROM projects WHERE org_id = ANY($2)))
           AND ($3::uuid IS NULL OR e.org_id = $3 OR e.project_id IN (SELECT id FROM projects WHERE org_id = $3))
           AND ($4::uuid IS NULL OR e.project_id = $4)
           AND ($5::uuid IS NULL OR e.actor_id = $5)
           AND ($6::text IS NULL OR e.kind LIKE $6 || '%')
           AND ($7::bigint IS NULL OR e.id < $7)
         ORDER BY e.id DESC LIMIT $8"
    );
    let rows = sqlx::query(&sql)
        .bind(sc.is_none())
        .bind(sc.unwrap_or_default())
        .bind(q.org)
        .bind(q.project)
        .bind(q.actor)
        .bind(q.kind)
        .bind(q.before)
        .bind(q.limit.unwrap_or(100).clamp(1, 1000))
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows.iter().map(|r| serde_json::to_value(event_from_row(r)).unwrap_or_default()).collect()))
}

// ---------------------------------------------------------------------------------------------
// Email

/// Whether and how the server sends email (server admins).
pub async fn email_status(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    Ok(Json(match &s.config.email {
        Some(e) => json!({ "configured": true, "kind": e.kind(), "via": e.describe(), "from": e.from.to_string() }),
        None => json!({ "configured": false }),
    }))
}

/// Send a test email to the admin asking, and say what happened.
pub async fn email_test(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Value>> {
    access::server_admin(&user)?;
    let mail = s.config.email.as_ref().ok_or_else(|| ApiError::invalid("Email is not set up on this server. See docs/EMAIL.md."))?;
    mail.send(&user.email, "MultiParity test email", &format!("This is a test from MultiParity at {}, sent through {}. Email works.", s.config.public_url, mail.describe()))
        .await
        .map_err(|e| ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "email_failed", format!("The email was not sent: {e:#}")))?;
    Ok(Json(json!({ "sent_to": user.email })))
}

// ---------------------------------------------------------------------------------------------
// Webhooks (organization admins)

fn webhook_json(r: &PgRow) -> Value {
    let secret: String = r.get("secret");
    json!({
        "id": r.get::<Uuid,_>("id"), "url": r.get::<String,_>("url"), "events": r.get::<Vec<String>,_>("events"),
        "project_id": r.get::<Option<Uuid>,_>("project_id"), "active": r.get::<bool,_>("active"),
        "secret_hint": format!("…{}", &secret[secret.len().saturating_sub(4)..]),
        "created_at": r.get::<DateTime<Utc>,_>("created_at"), "last_at": r.get::<Option<DateTime<Utc>>,_>("last_at"),
        "last_status": r.get::<Option<i32>,_>("last_status"), "last_error": r.get::<Option<String>,_>("last_error"),
        "failures": r.get::<i32,_>("failures"),
    })
}

const WEBHOOK_COLS: &str = "id, url, events, project_id, active, secret, created_at, last_at, last_status, last_error, failures";

pub async fn list_webhooks(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::org_admin(&s, &user, org).await?;
    let rows = sqlx::query(&format!("SELECT {WEBHOOK_COLS} FROM webhooks WHERE org_id = $1 ORDER BY created_at")).bind(org).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(webhook_json).collect()))
}

#[derive(Deserialize)]
pub struct WebhookBody {
    url: Option<String>,
    events: Option<Vec<String>>,
    project_id: Option<Uuid>,
    active: Option<bool>,
}

fn check_webhook_url(url: &str) -> ApiResult<()> {
    let ok = url.parse::<reqwest::Url>().map(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some()).unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(ApiError::invalid("Give the full address the events go to, starting with https://"))
    }
}

fn clean_events(events: Vec<String>) -> ApiResult<Vec<String>> {
    let list: Vec<String> = events.into_iter().map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect();
    if let Some(bad) = list.iter().find(|e| !e.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '_' || c == '*')) {
        return Err(ApiError::invalid(format!("{bad} is not an event name; use names like change.published or change.*")));
    }
    Ok(list)
}

/// Add a webhook. The answer holds the signing secret; it is shown only this once.
pub async fn create_webhook(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<WebhookBody>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let url = b.url.unwrap_or_default().trim().to_string();
    check_webhook_url(&url)?;
    let events = clean_events(b.events.unwrap_or_default())?;
    if let Some(p) = b.project_id {
        let same: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1 AND org_id = $2)").bind(p).bind(org).fetch_one(&s.db).await?;
        if !same {
            return Err(ApiError::invalid("That project is not in this organization."));
        }
    }
    let secret = new_token();
    let r = sqlx::query(&format!("INSERT INTO webhooks (org_id, url, events, project_id, secret, created_by) VALUES ($1,$2,$3,$4,$5,$6) RETURNING {WEBHOOK_COLS}"))
        .bind(org)
        .bind(&url)
        .bind(&events)
        .bind(b.project_id)
        .bind(&secret)
        .bind(user.id)
        .fetch_one(&s.db)
        .await?;
    events::emit(&s.db, &s.events, Some(org), None, Some(user.id), "admin.webhook_added", json!({ "url": url })).await;
    let mut v = webhook_json(&r);
    v["secret"] = json!(secret);
    Ok(Json(v))
}

async fn webhook_org(s: &AppState, user: &CurrentUser, id: Uuid) -> ApiResult<Uuid> {
    let org: Uuid = sqlx::query_scalar("SELECT org_id FROM webhooks WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That webhook"))?;
    access::org_admin(s, user, org).await?;
    Ok(org)
}

/// Change a webhook's address or events, or pause and resume it.
pub async fn update_webhook(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<WebhookBody>) -> ApiResult<Json<Value>> {
    webhook_org(&s, &user, id).await?;
    if let Some(u) = &b.url {
        check_webhook_url(u.trim())?;
    }
    let events = b.events.map(clean_events).transpose()?;
    let r = sqlx::query(&format!(
        "UPDATE webhooks SET url = COALESCE($2, url), events = COALESCE($3, events), active = COALESCE($4, active),
           failures = CASE WHEN $4 THEN 0 ELSE failures END WHERE id = $1 RETURNING {WEBHOOK_COLS}"
    ))
    .bind(id)
    .bind(b.url.map(|u| u.trim().to_string()))
    .bind(events)
    .bind(b.active)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(webhook_json(&r)))
}

pub async fn delete_webhook(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    webhook_org(&s, &user, id).await?;
    sqlx::query("DELETE FROM webhooks WHERE id = $1").bind(id).execute(&s.db).await?;
    Ok(Json(json!({})))
}

/// Post a `webhook.test` event to the hook now and say how its server answered.
pub async fn test_webhook(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let org = webhook_org(&s, &user, id).await?;
    let r = sqlx::query("SELECT url, secret FROM webhooks WHERE id = $1").bind(id).fetch_one(&s.db).await?;
    let (url, secret): (String, String) = (r.get("url"), r.get("secret"));
    let body = serde_json::to_vec(&json!({
        "id": 0, "kind": "webhook.test", "org_id": org, "project_id": null,
        "actor": { "id": user.id, "name": user.name, "email": user.email }, "data": { "message": "A test from MultiParity" },
        "created_at": Utc::now(), "server": s.config.public_url,
    }))
    .unwrap_or_default();
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build().map_err(|e| anyhow::anyhow!(e))?;
    let res = http
        .post(&url)
        .header("content-type", "application/json")
        .header("x-multiparity-event", "webhook.test")
        .header("x-multiparity-delivery", "0")
        .header("x-multiparity-signature", crate::webhooks::sign(&secret, &body))
        .body(body)
        .send()
        .await;
    let (status, error) = match res {
        Ok(r) if r.status().is_success() => (Some(r.status().as_u16() as i32), None),
        Ok(r) => (Some(r.status().as_u16() as i32), Some(format!("answered {}", r.status()))),
        Err(e) => (None, Some(e.to_string())),
    };
    sqlx::query("UPDATE webhooks SET last_at = now(), last_status = $2, last_error = $3 WHERE id = $1").bind(id).bind(status).bind(&error).execute(&s.db).await?;
    Ok(Json(json!({ "ok": error.is_none(), "status": status, "error": error })))
}
