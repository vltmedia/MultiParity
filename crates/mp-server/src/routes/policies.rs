//! Policies (Admin > Policies): named sets of access rules an organization gives to people.
//! How they combine is in migrations/0014_policies.sql (mp_access); this module manages them and
//! explains, per project, who can see it and why.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::access;
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::AppState;

pub const LEVELS: [&str; 3] = ["none", "read", "write"];

fn level_num(l: &str) -> ApiResult<i16> {
    LEVELS.iter().position(|x| *x == l).map(|n| n as i16).ok_or_else(|| ApiError::invalid("A rule's level is none, read or write."))
}

async fn policy_org(s: &AppState, id: Uuid) -> ApiResult<(Uuid, bool)> {
    let r = sqlx::query("SELECT org_id, is_default FROM policies WHERE id = $1").bind(id).fetch_optional(&s.db).await?.ok_or_else(|| ApiError::not_found("That policy"))?;
    Ok((r.get("org_id"), r.get("is_default")))
}

/// Make the default policy of a new organization: members can change every project.
pub async fn create_default(tx: &mut Transaction<'_, Postgres>, org: Uuid) -> ApiResult<()> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO policies (org_id, name, description, is_default) VALUES ($1, 'All members', 'Every member of the organization has this without being given it. Guests do not.', TRUE) RETURNING id",
    )
    .bind(org)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO policy_rules (policy_id, scope_kind, scope_id, level) VALUES ($1, 'org', $2, 2)").bind(id).bind(org).execute(&mut **tx).await?;
    Ok(())
}

pub async fn list(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    access::org_admin(&s, &user, org).await?;
    let pols = sqlx::query("SELECT id, name, description, is_default, created_at FROM policies WHERE org_id = $1 ORDER BY is_default DESC, lower(name)")
        .bind(org)
        .fetch_all(&s.db)
        .await?;
    // Rules, with the name of what each covers (and its client/production for a path).
    let rules = sqlx::query(
        "SELECT r.policy_id, r.scope_kind, r.scope_id, r.level,
           CASE r.scope_kind
             WHEN 'org' THEN (SELECT name FROM orgs WHERE id = r.scope_id)
             WHEN 'client' THEN (SELECT name FROM clients WHERE id = r.scope_id)
             WHEN 'production' THEN (SELECT concat_ws(' › ', c.name, pr.name) FROM productions pr LEFT JOIN clients c ON c.id = pr.client_id WHERE pr.id = r.scope_id)
             ELSE (SELECT concat_ws(' › ', c.name, pr.name, p.name) FROM projects p LEFT JOIN clients c ON c.id = p.client_id LEFT JOIN productions pr ON pr.id = p.production_id WHERE p.id = r.scope_id)
           END AS scope_name
         FROM policy_rules r JOIN policies pol ON pol.id = r.policy_id WHERE pol.org_id = $1
         ORDER BY CASE r.scope_kind WHEN 'org' THEN 0 WHEN 'client' THEN 1 WHEN 'production' THEN 2 ELSE 3 END, 5",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    let people = sqlx::query(
        "SELECT up.policy_id, u.id, u.name, u.email, m.role FROM user_policies up JOIN users u ON u.id = up.user_id
           JOIN policies pol ON pol.id = up.policy_id
           LEFT JOIN memberships m ON m.user_id = u.id AND m.org_id = pol.org_id
         WHERE pol.org_id = $1 ORDER BY u.name",
    )
    .bind(org)
    .fetch_all(&s.db)
    .await?;
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE org_id = $1 AND role = 'member'").bind(org).fetch_one(&s.db).await?;
    let out = pols
        .iter()
        .map(|p| {
            let id: Uuid = p.get("id");
            json!({
                "id": id, "name": p.get::<String,_>("name"), "description": p.get::<String,_>("description"),
                "is_default": p.get::<bool,_>("is_default"), "created_at": p.get::<DateTime<Utc>,_>("created_at"),
                "rules": rules.iter().filter(|r| r.get::<Uuid,_>("policy_id") == id).map(|r| json!({
                    "scope_kind": r.get::<String,_>("scope_kind"), "scope_id": r.get::<Uuid,_>("scope_id"),
                    "scope_name": r.get::<Option<String>,_>("scope_name").unwrap_or_else(|| "Something since removed".into()),
                    "level": LEVELS[r.get::<i16,_>("level") as usize],
                })).collect::<Vec<_>>(),
                "people": people.iter().filter(|r| r.get::<Uuid,_>("policy_id") == id).map(|r| json!({
                    "id": r.get::<Uuid,_>("id"), "name": r.get::<String,_>("name"), "email": r.get::<String,_>("email"), "role": r.get::<Option<String>,_>("role"),
                })).collect::<Vec<_>>(),
                "member_count": if p.get::<bool,_>("is_default") { Some(members) } else { None },
            })
        })
        .collect();
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct RuleBody {
    scope_kind: String,
    scope_id: Uuid,
    level: String,
}

#[derive(Deserialize)]
pub struct PolicyBody {
    name: Option<String>,
    description: Option<String>,
    rules: Option<Vec<RuleBody>>,
}

/// Check each rule names something in this organization, and that no two cover the same thing.
async fn check_rules(s: &AppState, org: Uuid, rules: &[RuleBody]) -> ApiResult<Vec<(String, Uuid, i16)>> {
    let mut out: Vec<(String, Uuid, i16)> = Vec::new();
    for r in rules {
        let sql = match r.scope_kind.as_str() {
            "org" => "SELECT id = $2 FROM orgs WHERE id = $1",
            "client" => "SELECT org_id = $2 FROM clients WHERE id = $1",
            "production" => "SELECT org_id = $2 FROM productions WHERE id = $1",
            "project" => "SELECT org_id = $2 FROM projects WHERE id = $1",
            _ => return Err(ApiError::invalid("A rule covers the organization, a client, a production or a project.")),
        };
        let ok: Option<bool> = sqlx::query_scalar(sql).bind(r.scope_id).bind(org).fetch_optional(&s.db).await?;
        if ok != Some(true) {
            return Err(ApiError::invalid("A rule names something that is not in this organization (or no longer exists)."));
        }
        if out.iter().any(|(k, id, _)| *k == r.scope_kind && *id == r.scope_id) {
            return Err(ApiError::invalid("Two rules cover the same thing. Keep one."));
        }
        out.push((r.scope_kind.clone(), r.scope_id, level_num(&r.level)?));
    }
    Ok(out)
}

async fn write_rules(tx: &mut Transaction<'_, Postgres>, policy: Uuid, rules: &[(String, Uuid, i16)]) -> ApiResult<()> {
    sqlx::query("DELETE FROM policy_rules WHERE policy_id = $1").bind(policy).execute(&mut **tx).await?;
    for (k, id, l) in rules {
        sqlx::query("INSERT INTO policy_rules (policy_id, scope_kind, scope_id, level) VALUES ($1,$2,$3,$4)").bind(policy).bind(k).bind(id).bind(l).execute(&mut **tx).await?;
    }
    Ok(())
}

fn clean(name: &str) -> ApiResult<String> {
    let n = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() {
        return Err(ApiError::invalid("Give the policy a name."));
    }
    Ok(n)
}

fn taken(e: sqlx::Error, name: &str) -> ApiError {
    match ApiError::from(e) {
        err if err.code == "conflict" => ApiError::conflict(format!("There is already a policy called {name}.")),
        err => err,
    }
}

pub async fn create(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<PolicyBody>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let name = clean(b.name.as_deref().unwrap_or(""))?;
    let rules = check_rules(&s, org, b.rules.as_deref().unwrap_or(&[])).await?;
    let mut tx = s.db.begin().await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO policies (org_id, name, description) VALUES ($1,$2,$3) RETURNING id")
        .bind(org)
        .bind(&name)
        .bind(b.description.as_deref().unwrap_or("").trim())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| taken(e, &name))?;
    write_rules(&mut tx, id, &rules).await?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.policy_created", json!({ "id": id, "name": name, "rules": rules.len() })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub async fn update(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<PolicyBody>) -> ApiResult<Json<Value>> {
    let (org, _) = policy_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    let rules = match &b.rules {
        Some(r) => Some(check_rules(&s, org, r).await?),
        None => None,
    };
    let mut tx = s.db.begin().await?;
    if let Some(n) = b.name.as_deref() {
        let n = clean(n)?;
        sqlx::query("UPDATE policies SET name = $2 WHERE id = $1").bind(id).bind(&n).execute(&mut *tx).await.map_err(|e| taken(e, &n))?;
    }
    if let Some(d) = b.description.as_deref() {
        sqlx::query("UPDATE policies SET description = $2 WHERE id = $1").bind(id).bind(d.trim()).execute(&mut *tx).await?;
    }
    if let Some(r) = &rules {
        write_rules(&mut tx, id, r).await?;
    }
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.policy_changed", json!({ "id": id, "name": b.name, "rules": rules.as_ref().map(|r| r.len()) })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let (org, is_default) = policy_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    if is_default {
        return Err(ApiError::invalid("The members' policy cannot be deleted. Change its rules instead; with none, members only get what other policies give them."));
    }
    let mut tx = s.db.begin().await?;
    let name: String = sqlx::query_scalar("DELETE FROM policies WHERE id = $1 RETURNING name").bind(id).fetch_one(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.policy_removed", json!({ "id": id, "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PeopleBody {
    users: Vec<Uuid>,
}

/// Set exactly who has a policy (people in its organization).
pub async fn set_people(State(s): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(b): Json<PeopleBody>) -> ApiResult<Json<Value>> {
    let (org, is_default) = policy_org(&s, id).await?;
    access::org_admin(&s, &user, org).await?;
    if is_default {
        return Err(ApiError::invalid("Every member has the members' policy; it is not given to people one by one."));
    }
    let inside: i64 = sqlx::query_scalar("SELECT count(*) FROM memberships WHERE org_id = $1 AND user_id = ANY($2)").bind(org).bind(&b.users).fetch_one(&s.db).await?;
    if inside as usize != b.users.iter().collect::<std::collections::HashSet<_>>().len() {
        return Err(ApiError::invalid("Policies can only be given to people in this organization. Invite them first."));
    }
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM user_policies WHERE policy_id = $1 AND NOT (user_id = ANY($2))").bind(id).bind(&b.users).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO user_policies (user_id, policy_id) SELECT unnest($2::uuid[]), $1 ON CONFLICT DO NOTHING").bind(id).bind(&b.users).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org), None, Some(user.id), "admin.policy_people", json!({ "id": id, "users": b.users })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(json!({ "ok": true })))
}

/// Who in the organization can see a project, at what level, and why.
pub async fn who(State(s): State<AppState>, user: CurrentUser, Path(p): Path<Uuid>) -> ApiResult<Json<Vec<Value>>> {
    let acc = access::project(&s, &user, p, access::Level::Read).await?;
    access::org_admin(&s, &user, acc.org_id).await?;
    let people = sqlx::query(
        "SELECT u.id, u.name, u.email, m.role, mp_access(u.id, $1) AS level
         FROM memberships m JOIN users u ON u.id = m.user_id
         WHERE m.org_id = $2 AND (m.expires_at IS NULL OR m.expires_at > now()) ORDER BY u.name",
    )
    .bind(p)
    .bind(acc.org_id)
    .fetch_all(&s.db)
    .await?;
    // The rules that cover this project, per person, with where they come from.
    let rules = sqlx::query(
        "SELECT m.user_id, pol.name, pol.is_default, r.scope_kind, r.level,
           CASE r.scope_kind WHEN 'org' THEN 0 WHEN 'client' THEN 1 WHEN 'production' THEN 2 ELSE 3 END AS spec
         FROM memberships m JOIN policies pol ON pol.org_id = m.org_id JOIN policy_rules r ON r.policy_id = pol.id
           JOIN projects p ON p.id = $1
         WHERE m.org_id = $2
           AND ((pol.is_default AND m.role = 'member') OR EXISTS (SELECT 1 FROM user_policies up WHERE up.policy_id = pol.id AND up.user_id = m.user_id))
           AND ((r.scope_kind = 'org' AND r.scope_id = p.org_id) OR (r.scope_kind = 'client' AND r.scope_id = p.client_id)
             OR (r.scope_kind = 'production' AND r.scope_id = p.production_id) OR (r.scope_kind = 'project' AND r.scope_id = p.id))
         UNION ALL
         SELECT pa.user_id, NULL, FALSE, 'project', (CASE WHEN pa.can_write THEN 2 ELSE 1 END)::SMALLINT, 3 FROM project_access pa WHERE pa.project_id = $1",
    )
    .bind(p)
    .bind(acc.org_id)
    .fetch_all(&s.db)
    .await?;
    let mut by_user: HashMap<Uuid, Vec<&sqlx::postgres::PgRow>> = HashMap::new();
    for r in &rules {
        by_user.entry(r.get("user_id")).or_default().push(r);
    }
    let what = |k: &str| match k {
        "org" => "the whole organization",
        "client" => "its client",
        "production" => "its production",
        _ => "this project",
    };
    let out = people
        .iter()
        .map(|r| {
            let uid: Uuid = r.get("id");
            let role: String = r.get("role");
            let level: i16 = r.get("level");
            let because = if role == "owner" || role == "admin" {
                format!("Organization {role}: everything in the organization")
            } else {
                let mine = by_user.get(&uid).cloned().unwrap_or_default();
                match mine.iter().map(|x| x.get::<i32, _>("spec")).max() {
                    None if role == "guest" => "Guest with no policy covering this project".to_string(),
                    None => "No policy covers this project".to_string(),
                    Some(top) => {
                        let mut names: Vec<String> = mine
                            .iter()
                            .filter(|x| x.get::<i32, _>("spec") == top && x.get::<i16, _>("level") == level)
                            .map(|x| x.get::<Option<String>, _>("name").map(|n| format!("{n} policy")).unwrap_or_else(|| "Added to this project directly".into()))
                            .collect();
                        names.dedup();
                        let kind: String = mine.iter().find(|x| x.get::<i32, _>("spec") == top).map(|x| x.get("scope_kind")).unwrap_or_default();
                        format!("{} (rule on {})", names.join(", "), what(&kind))
                    }
                }
            };
            let level_name = ["none", "read", "write", "admin"][level.clamp(0, 3) as usize];
            json!({
                "user": { "id": uid, "name": r.get::<String,_>("name"), "email": r.get::<String,_>("email") },
                "role": role, "level": level_name, "because": because,
            })
        })
        .collect();
    Ok(Json(out))
}
