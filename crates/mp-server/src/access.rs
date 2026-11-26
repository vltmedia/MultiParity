//! Who may do what. Roles are per organization: owners and admins administer it, members can
//! read and write every project in it, guests only the projects they were given. Server admins
//! can do everything everywhere.

use sqlx::Row;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::{ApiError, ApiResult};
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Read,
    Write,
    Admin,
}

#[derive(Debug, Clone)]
pub struct ProjectAccess {
    pub project_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub level: Level,
    pub role: String,
    pub archived: bool,
}

pub async fn org_role(state: &AppState, user: Uuid, org: Uuid) -> ApiResult<Option<String>> {
    Ok(sqlx::query_scalar("SELECT role FROM memberships WHERE org_id = $1 AND user_id = $2 AND (expires_at IS NULL OR expires_at > now())")
        .bind(org)
        .bind(user)
        .fetch_optional(&state.db)
        .await?)
}

pub async fn project(state: &AppState, user: &CurrentUser, project: Uuid, need: Level) -> ApiResult<ProjectAccess> {
    let row = sqlx::query("SELECT id, org_id, name, archived_at IS NOT NULL AS archived FROM projects WHERE id = $1")
        .bind(project)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError::not_found("That project"))?;
    let org_id: Uuid = row.get("org_id");
    let archived: bool = row.get("archived");
    let role = org_role(state, user.id, org_id).await?;
    let (level, role_name) = if user.is_server_admin {
        (Some(Level::Admin), role.clone().unwrap_or_else(|| "owner".into()))
    } else {
        match role.as_deref() {
            Some("owner") | Some("admin") => (Some(Level::Admin), role.clone().unwrap()),
            Some("member") => (Some(Level::Write), "member".into()),
            Some("guest") => {
                let w: Option<bool> = sqlx::query_scalar("SELECT can_write FROM project_access WHERE project_id = $1 AND user_id = $2")
                    .bind(project)
                    .bind(user.id)
                    .fetch_optional(&state.db)
                    .await?;
                (w.map(|w| if w { Level::Write } else { Level::Read }), "guest".into())
            }
            _ => (None, String::new()),
        }
    };
    let Some(level) = level else {
        return Err(ApiError::not_found("That project"));
    };
    if level < need {
        return Err(ApiError::forbidden(match need {
            Level::Admin => "Only admins of this organization can do that.",
            _ => "You can look at this project but not change it. Ask an admin for write access.",
        }));
    }
    if archived && need >= Level::Write {
        return Err(ApiError::conflict("This project is archived. Restore it in Admin before changing it."));
    }
    Ok(ProjectAccess { project_id: project, org_id, name: row.get("name"), level, role: role_name, archived })
}

/// Require owner or admin of `org` (or server admin).
pub async fn org_admin(state: &AppState, user: &CurrentUser, org: Uuid) -> ApiResult<String> {
    if user.is_server_admin {
        return Ok("owner".into());
    }
    match org_role(state, user.id, org).await?.as_deref() {
        Some(r @ ("owner" | "admin")) => Ok(r.to_string()),
        Some(_) => Err(ApiError::forbidden("Only owners and admins of this organization can do that.")),
        None => Err(ApiError::not_found("That organization")),
    }
}

pub fn server_admin(user: &CurrentUser) -> ApiResult<()> {
    if user.is_server_admin {
        Ok(())
    } else {
        Err(ApiError::forbidden("Only server admins can do that."))
    }
}

/// SQL condition (on alias `p`) for projects `$1` (user id) can see. `$2` is whether the user is
/// a server admin.
pub const VISIBLE_PROJECTS: &str = "($2 OR EXISTS (SELECT 1 FROM memberships m WHERE m.org_id = p.org_id AND m.user_id = $1
      AND (m.expires_at IS NULL OR m.expires_at > now())
      AND (m.role <> 'guest' OR EXISTS (SELECT 1 FROM project_access pa WHERE pa.project_id = p.id AND pa.user_id = $1))))";

/// The organizations an org-scoped admin may act on: all for server admins, otherwise the ones
/// where they are owner or admin.
pub async fn admin_orgs(state: &AppState, user: &CurrentUser) -> ApiResult<Option<Vec<Uuid>>> {
    if user.is_server_admin {
        return Ok(None);
    }
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT org_id FROM memberships WHERE user_id = $1 AND role IN ('owner','admin') AND (expires_at IS NULL OR expires_at > now())")
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    if ids.is_empty() {
        return Err(ApiError::forbidden("Only organization owners and admins can open Admin."));
    }
    Ok(Some(ids))
}
