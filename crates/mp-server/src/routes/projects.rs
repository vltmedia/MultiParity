use axum::extract::{Path, State};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::access::{self, Level, VISIBLE_PROJECTS};
use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::events;
use crate::models::{branch_from_row, Org, UserRef, BRANCH_SELECT};
use crate::util::slugify;
use crate::AppState;

pub async fn list_orgs(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<Org>>> {
    let rows = sqlx::query(
        "SELECT o.id, o.slug, o.name, o.created_at, o.theme_light, o.theme_dark,
           (SELECT count(*) FROM memberships m WHERE m.org_id = o.id) AS member_count,
           (SELECT count(*) FROM projects p WHERE p.org_id = o.id AND p.archived_at IS NULL) AS project_count
         FROM orgs o WHERE $2 OR EXISTS (SELECT 1 FROM memberships m WHERE m.org_id = o.id AND m.user_id = $1) ORDER BY o.name",
    )
    .bind(user.id)
    .bind(user.is_server_admin)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.iter().map(Org::from_row).collect()))
}

#[derive(Deserialize)]
pub struct CreateOrg {
    name: String,
    slug: Option<String>,
}

pub async fn create_org(State(s): State<AppState>, user: CurrentUser, Json(b): Json<CreateOrg>) -> ApiResult<Json<Org>> {
    access::server_admin(&user)?;
    let name = b.name.trim();
    if name.is_empty() {
        return Err(ApiError::invalid("Give the organization a name."));
    }
    let slug = slugify(b.slug.as_deref().unwrap_or(name));
    let mut tx = s.db.begin().await?;
    let row = sqlx::query("INSERT INTO orgs (slug, name) VALUES ($1,$2) RETURNING id, slug, name, created_at")
        .bind(&slug)
        .bind(name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict(format!("An organization with the short name {slug} already exists.")),
            err => err,
        })?;
    let org = Org::from_row(&row);
    sqlx::query("INSERT INTO memberships (org_id, user_id, role) VALUES ($1,$2,'owner')").bind(org.id).bind(user.id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org.id), None, Some(user.id), "admin.org_created", json!({ "name": name })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    Ok(Json(org))
}

#[derive(Deserialize)]
pub struct CreateProject {
    name: String,
    slug: Option<String>,
    /// Storage location for its file data (`main` unless given).
    storage: Option<String>,
}

pub async fn create_project(State(s): State<AppState>, user: CurrentUser, Path(org): Path<Uuid>, Json(b): Json<CreateProject>) -> ApiResult<Json<Value>> {
    access::org_admin(&s, &user, org).await?;
    let name = b.name.trim();
    if name.is_empty() {
        return Err(ApiError::invalid("Give the project a name."));
    }
    let slug = slugify(b.slug.as_deref().unwrap_or(name));
    let storage = b.storage.as_deref().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).unwrap_or_else(|| s.config.default_storage.clone());
    if s.stores.get(&storage).is_none() {
        return Err(ApiError::invalid(format!("There is no storage location called {storage} on this server.")));
    }
    let mut tx = s.db.begin().await?;
    let pid: Uuid = sqlx::query_scalar("INSERT INTO projects (org_id, slug, name, storage) VALUES ($1,$2,$3,$4) RETURNING id")
        .bind(org)
        .bind(&slug)
        .bind(name)
        .bind(&storage)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match ApiError::from(e) {
            err if err.code == "conflict" => ApiError::conflict(format!("This organization already has a project called {slug}.")),
            err => err,
        })?;
    sqlx::query("INSERT INTO branches (project_id, name, created_by) VALUES ($1,'Main',$2)").bind(pid).bind(user.id).execute(&mut *tx).await?;
    let ev = events::record(&mut *tx, Some(org), Some(pid), Some(user.id), "project.created", json!({ "name": name, "storage": storage })).await?;
    tx.commit().await?;
    events::publish_saved(&s.db, &s.events, ev).await;
    project_detail(&s, &user, pid).await.map(Json)
}

/// SQL for project summaries. Binds: $1 user id, $2 is server admin, $3 org id list (may be
/// empty; used by admin listings). `where_sql` filters alias `p`.
pub fn summary_sql(where_sql: &str) -> String {
    format!(
        "SELECT p.id, p.slug, p.name, p.archived_at IS NOT NULL AS archived, p.next_change - 1 AS head_change, p.checkpoints_on_server, p.auto_checkpoints, p.storage, p.storage_previous,
           p.theme_light, p.theme_dark, o.theme_light AS o_theme_light, o.theme_dark AS o_theme_dark,
           o.id AS o_id, o.slug AS o_slug, o.name AS o_name, o.created_at AS o_created_at,
           COALESCE(st.size, 0)::BIGINT AS size, COALESCE(st.files, 0)::BIGINT AS file_count,
           (SELECT count(*) FROM claims c WHERE c.project_id = p.id) AS claims,
           (SELECT count(*) FROM claims c WHERE c.project_id = p.id AND c.user_id = $1) AS my_claims,
           (SELECT count(*) FROM branches b WHERE b.project_id = p.id AND NOT b.archived) AS branches,
           lc.number AS lc_number, lc.message AS lc_message, lc.created_at AS lc_created_at,
           lc.u_id AS lc_id, lc.u_name AS lc_name, lc.u_email AS lc_email
         FROM projects p JOIN orgs o ON o.id = p.org_id
         LEFT JOIN LATERAL (SELECT sum(t.size) AS size, count(*) AS files FROM tree_entries t JOIN branches b ON b.id = t.branch_id
              WHERE b.project_id = p.id AND b.parent_id IS NULL AND t.file_hash IS NOT NULL) st ON TRUE
         LEFT JOIN LATERAL (SELECT c.number, c.message, c.created_at, u.id AS u_id, u.name AS u_name, u.email AS u_email
              FROM changes c LEFT JOIN users u ON u.id = c.author_id WHERE c.project_id = p.id ORDER BY c.number DESC LIMIT 1) lc ON TRUE
         WHERE ($3::uuid[] IS NOT NULL) AND {where_sql}
         ORDER BY o.name, p.name"
    )
}

/// Projects at least this big keep checkpoints on the server unless set otherwise
/// (`MP_SERVER_CHECKPOINTS_FROM_GB`, default 20).
fn server_checkpoints_from() -> i64 {
    let gb: f64 = std::env::var("MP_SERVER_CHECKPOINTS_FROM_GB").ok().and_then(|v| v.parse().ok()).unwrap_or(20.0);
    (gb * 1024.0 * 1024.0 * 1024.0) as i64
}

pub fn summary_json(r: &PgRow) -> Value {
    let last = r.get::<Option<i32>, _>("lc_number").map(|n| {
        json!({ "number": n, "message": r.get::<String,_>("lc_message"), "created_at": r.get::<DateTime<Utc>,_>("lc_created_at"),
                "author": UserRef::from_row(r, "lc_") })
    });
    // Where checkpoints are kept: the project's setting, or by size when it is left automatic.
    let setting = r.get::<Option<bool>, _>("checkpoints_on_server");
    let on_server = setting.unwrap_or_else(|| r.get::<i64, _>("size") >= server_checkpoints_from());
    json!({
        "checkpoint_setting": match setting { None => "auto", Some(true) => "server", Some(false) => "local" },
        "checkpoint_storage": if on_server { "server" } else { "local" },
        "auto_checkpoints": r.get::<bool,_>("auto_checkpoints"),
        "storage": r.get::<String,_>("storage"),
        "storage_moving": r.get::<Option<String>,_>("storage_previous").is_some(),
        "id": r.get::<Uuid,_>("id"), "slug": r.get::<String,_>("slug"), "name": r.get::<String,_>("name"),
        "org": { "id": r.get::<Uuid,_>("o_id"), "slug": r.get::<String,_>("o_slug"), "name": r.get::<String,_>("o_name"), "created_at": r.get::<DateTime<Utc>,_>("o_created_at") },
        "archived": r.get::<bool,_>("archived"), "head_change": r.get::<i32,_>("head_change"),
        "size": r.get::<i64,_>("size"), "file_count": r.get::<i64,_>("file_count"),
        "claims": r.get::<i64,_>("claims"), "my_claims": r.get::<i64,_>("my_claims"),
        "branches": r.get::<i64,_>("branches"), "last_change": last,
        // The look for people who have not picked their own: the project's, else its organization's.
        "theme": {
            "light": r.get::<Option<String>,_>("theme_light").or(r.get("o_theme_light")),
            "dark": r.get::<Option<String>,_>("theme_dark").or(r.get("o_theme_dark")),
            "own_light": r.get::<Option<String>,_>("theme_light"), "own_dark": r.get::<Option<String>,_>("theme_dark"),
        },
    })
}

pub async fn list_projects(State(s): State<AppState>, user: CurrentUser) -> ApiResult<Json<Vec<Value>>> {
    let sql = summary_sql(&format!("p.archived_at IS NULL AND {VISIBLE_PROJECTS}"));
    let rows = sqlx::query(&sql).bind(user.id).bind(user.is_server_admin).bind(Vec::<Uuid>::new()).fetch_all(&s.db).await?;
    Ok(Json(rows.iter().map(summary_json).collect()))
}

pub async fn project_detail(s: &AppState, user: &CurrentUser, pid: Uuid) -> ApiResult<Value> {
    let acc = access::project(s, user, pid, Level::Read).await?;
    let sql = summary_sql("p.id = $4");
    let row = sqlx::query(&sql).bind(user.id).bind(user.is_server_admin).bind(Vec::<Uuid>::new()).bind(pid).fetch_one(&s.db).await?;
    let mut v = summary_json(&row);
    let sql = format!("{BRANCH_SELECT} WHERE b.project_id = $1 ORDER BY b.parent_id IS NOT NULL, b.archived, b.name");
    let branches: Vec<_> = sqlx::query(&sql).bind(pid).fetch_all(&s.db).await?.iter().map(branch_from_row).collect();
    v["branches_list"] = serde_json::to_value(branches).unwrap_or_default();
    v["my_role"] = json!(acc.role);
    v["can_write"] = json!(acc.level >= Level::Write);
    v["can_admin"] = json!(acc.level >= Level::Admin);
    Ok(v)
}

pub async fn get_project(State(s): State<AppState>, user: CurrentUser, Path(pid): Path<Uuid>) -> ApiResult<Json<Value>> {
    project_detail(&s, &user, pid).await.map(Json)
}
