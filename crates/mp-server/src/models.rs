//! JSON shapes returned by the API (see docs/API.md) and the queries that load them.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRef {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl UserRef {
    /// From a row with `<prefix>id`, `<prefix>name`, `<prefix>email` columns; `None` when the id
    /// is NULL (deleted user).
    pub fn from_row(row: &PgRow, prefix: &str) -> Option<UserRef> {
        let id: Option<Uuid> = row.get(format!("{prefix}id").as_str());
        id.map(|id| UserRef {
            id,
            name: row.get::<Option<String>, _>(format!("{prefix}name").as_str()).unwrap_or_default(),
            email: row.get::<Option<String>, _>(format!("{prefix}email").as_str()).unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Org {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub member_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_count: Option<i64>,
}

impl Org {
    pub fn from_row(row: &PgRow) -> Org {
        Org {
            id: row.get("id"),
            slug: row.get("slug"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            member_count: row.try_get("member_count").ok(),
            project_count: row.try_get("project_count").ok(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OrgRole {
    pub org: Org,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Me {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub is_server_admin: bool,
    pub status: String,
    pub orgs: Vec<OrgRole>,
}

pub async fn org_roles(db: &PgPool, user: Uuid) -> sqlx::Result<Vec<OrgRole>> {
    let rows = sqlx::query(
        "SELECT o.id, o.slug, o.name, o.created_at, m.role FROM memberships m JOIN orgs o ON o.id = m.org_id
         WHERE m.user_id = $1 AND (m.expires_at IS NULL OR m.expires_at > now()) ORDER BY o.name",
    )
    .bind(user)
    .fetch_all(db)
    .await?;
    Ok(rows.iter().map(|r| OrgRole { org: Org::from_row(r), role: r.get("role") }).collect())
}

pub async fn load_me(db: &PgPool, user: Uuid) -> sqlx::Result<Me> {
    let r = sqlx::query("SELECT id, email, name, is_server_admin, status FROM users WHERE id = $1")
        .bind(user)
        .fetch_one(db)
        .await?;
    Ok(Me {
        id: r.get("id"),
        email: r.get("email"),
        name: r.get("name"),
        is_server_admin: r.get("is_server_admin"),
        status: r.get("status"),
        orgs: org_roles(db, user).await?,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: i64,
    pub kind: String,
    pub org_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub actor: Option<UserRef>,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

pub const EVENT_SELECT: &str = "SELECT e.id, e.kind, e.org_id, e.project_id, e.data, e.created_at,
    u.id AS a_id, u.name AS a_name, u.email AS a_email
    FROM events e LEFT JOIN users u ON u.id = e.actor_id ";

pub fn event_from_row(r: &PgRow) -> Event {
    Event {
        id: r.get("id"),
        kind: r.get("kind"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
        actor: UserRef::from_row(r, "a_"),
        data: r.get("data"),
        created_at: r.get("created_at"),
    }
}

pub async fn load_events(db: &PgPool, where_clause: &str, id: i64) -> sqlx::Result<Vec<Event>> {
    let sql = format!("{EVENT_SELECT} {where_clause}");
    let rows = sqlx::query(&sql).bind(id).fetch_all(db).await?;
    Ok(rows.iter().map(event_from_row).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct Branch {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub shares_claims: bool,
    pub protected: bool,
    pub archived: bool,
    pub head_change: i32,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<UserRef>,
    pub ahead: i64,
    pub behind: i64,
}

pub const BRANCH_SELECT: &str = "SELECT b.id, b.name, b.parent_id, b.shares_claims, b.protected, b.archived, b.head_change, b.created_at,
    u.id AS c_id, u.name AS c_name, u.email AS c_email,
    CASE WHEN b.parent_id IS NULL THEN 0 ELSE
      (SELECT count(*) FROM tree_entries t WHERE t.branch_id = b.id AND t.file_hash IS DISTINCT FROM t.base_file_hash) END AS ahead,
    CASE WHEN b.parent_id IS NULL THEN 0 ELSE
      (SELECT count(*) FROM tree_entries p LEFT JOIN tree_entries c ON c.branch_id = b.id AND c.path = p.path
       WHERE p.branch_id = b.parent_id AND p.file_hash IS DISTINCT FROM c.base_file_hash) END AS behind
    FROM branches b LEFT JOIN users u ON u.id = b.created_by ";

pub fn branch_from_row(r: &PgRow) -> Branch {
    Branch {
        id: r.get("id"),
        name: r.get("name"),
        parent_id: r.get("parent_id"),
        shares_claims: r.get("shares_claims"),
        protected: r.get("protected"),
        archived: r.get("archived"),
        head_change: r.get("head_change"),
        created_at: r.get("created_at"),
        created_by: UserRef::from_row(r, "c_"),
        ahead: r.get("ahead"),
        behind: r.get("behind"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub number: i32,
    pub branch_id: Uuid,
    pub message: String,
    pub author: Option<UserRef>,
    pub created_at: DateTime<Utc>,
    pub kind: String,
    pub file_count: i32,
    pub size: i64,
    pub merged_from: Option<Vec<i32>>,
}

pub const CHANGE_SELECT: &str = "SELECT c.number, c.branch_id, c.message, c.created_at, c.kind, c.file_count, c.size, c.merged_from,
    u.id AS u_id, u.name AS u_name, u.email AS u_email
    FROM changes c LEFT JOIN users u ON u.id = c.author_id ";

pub fn change_from_row(r: &PgRow) -> Change {
    Change {
        number: r.get("number"),
        branch_id: r.get("branch_id"),
        message: r.get("message"),
        author: UserRef::from_row(r, "u_"),
        created_at: r.get("created_at"),
        kind: r.get("kind"),
        file_count: r.get("file_count"),
        size: r.get("size"),
        merged_from: r.get("merged_from"),
    }
}

pub async fn load_change(db: &PgPool, project: Uuid, number: i32) -> sqlx::Result<Change> {
    let sql = format!("{CHANGE_SELECT} WHERE c.project_id = $1 AND c.number = $2");
    let r = sqlx::query(&sql).bind(project).bind(number).fetch_one(db).await?;
    Ok(change_from_row(&r))
}

#[derive(Debug, Clone, Serialize)]
pub struct Claim {
    pub path: String,
    pub branch_id: Uuid,
    pub user: Option<UserRef>,
    pub device: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectRef {
    pub id: Uuid,
    pub name: String,
}

pub const CLAIM_SELECT: &str = "SELECT cl.path, cl.branch_id, cl.device, cl.created_at, cl.project_id, p.name AS project_name,
    u.id AS u_id, u.name AS u_name, u.email AS u_email
    FROM claims cl JOIN users u ON u.id = cl.user_id JOIN projects p ON p.id = cl.project_id ";

pub fn claim_from_row(r: &PgRow, with_project: bool) -> Claim {
    Claim {
        path: r.get("path"),
        branch_id: r.get("branch_id"),
        user: UserRef::from_row(r, "u_"),
        device: r.get("device"),
        created_at: r.get("created_at"),
        project: with_project.then(|| ProjectRef { id: r.get("project_id"), name: r.get("project_name") }),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TagRef {
    pub id: Uuid,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub group: Option<String>,
    pub org_id: Uuid,
    pub project_id: Option<Uuid>,
}

pub fn tag_from_row(r: &PgRow) -> Tag {
    Tag {
        id: r.get("id"),
        name: r.get("name"),
        color: r.get("color"),
        group: r.get("group_name"),
        org_id: r.get("org_id"),
        project_id: r.get("project_id"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishEntry {
    pub path: String,
    pub op: String,
    pub prev_file_hash: Option<String>,
    pub manifest: Option<mp_core::Manifest>,
}
