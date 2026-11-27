use axum::routing::{delete, get, patch, post, put};
use axum::Router;

use crate::AppState;

mod account;
mod admin;
mod backups;
mod branches;
mod checkpoints;
mod desktop;
mod claims;
mod drafts;
mod events;
mod files;
mod projects;
pub mod storage;
mod sync;
mod tags;
mod thumbs;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { axum::Json(serde_json::json!({ "ok": true, "version": env!("CARGO_PKG_VERSION") })) }))
        // The desktop app's updates and downloads (public)
        .route("/desktop", get(desktop::info))
        .route("/desktop/update/:target/:arch/:current", get(desktop::update))
        .route("/desktop/files/:name", get(desktop::file))
        // Accounts
        .route("/auth/login", post(account::login))
        .route("/auth/logout", post(account::logout))
        .route("/auth/forgot", post(account::forgot))
        .route("/auth/reset", post(account::reset))
        .route("/server", get(account::server_info))
        .route("/me", get(account::me))
        .route("/me/tokens", get(account::list_tokens).post(account::create_token))
        .route("/me/tokens/:id", delete(account::revoke_token))
        .route("/invites/:token", get(account::invite_info))
        .route("/invites/:token/accept", post(account::accept_invite))
        // Organizations and projects
        .route("/orgs", get(projects::list_orgs).post(projects::create_org))
        .route("/orgs/:org/projects", post(projects::create_project))
        .route("/projects", get(projects::list_projects))
        .route("/projects/:p", get(projects::get_project))
        // Branches
        .route("/projects/:p/branches", get(branches::list).post(branches::create))
        .route("/projects/:p/branches/:b/update", post(branches::update))
        .route("/projects/:p/branches/:b/merge-preview", get(branches::merge_preview))
        .route("/projects/:p/branches/:b/merge", post(branches::merge))
        .route("/projects/:p/branches/:b/archive", post(branches::archive))
        // Files and history
        .route("/projects/:p/branches/:b/files", get(files::list_files))
        .route("/projects/:p/branches/:b/tree", get(files::tree))
        .route("/projects/:p/changes", get(files::list_changes))
        .route("/projects/:p/changes/:n", get(files::get_change))
        .route("/projects/:p/history", get(files::history))
        .route("/projects/:p/download", get(files::download))
        .route("/projects/:p/versions", get(files::versions))
        .route("/projects/:p/thumbs/:hash", get(thumbs::get))
        .route("/projects/:p/branches/:b/restore-version", post(files::restore_version))
        // Claims
        .route("/projects/:p/claims", get(claims::list).post(claims::claim))
        .route("/projects/:p/claims/release", post(claims::release))
        .route("/projects/:p/claims/nudge", post(claims::nudge))
        // Tags
        .route("/projects/:p/tags", get(tags::list).post(tags::create))
        .route("/projects/:p/tags/apply", post(tags::apply))
        .route("/projects/:p/tags/remove", post(tags::remove))
        // Drafts
        .route("/projects/:p/drafts", get(drafts::list).post(drafts::create))
        .route("/projects/:p/drafts/:id", get(drafts::get).delete(drafts::remove))
        // Checkpoints kept on the server (private to their author)
        .route("/projects/:p/checkpoints", get(checkpoints::list).post(checkpoints::create))
        .route("/projects/:p/checkpoints/:id", get(checkpoints::get).delete(checkpoints::remove))
        // Sync protocol
        .route("/projects/:p/chunks/missing", post(sync::missing_chunks))
        .route("/projects/:p/chunks/:hash", put(sync::put_chunk).get(sync::get_chunk))
        .route("/projects/:p/manifests/:hash", get(sync::get_manifest))
        .route("/projects/:p/branches/:b/publish", post(sync::publish))
        // Events
        .route("/events", get(events::stream))
        // Admin
        .route("/admin/overview", get(admin::overview))
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route("/admin/users/:id", get(admin::get_user).delete(admin::delete_user))
        .route("/admin/users/:id/suspend", post(admin::suspend_user))
        .route("/admin/users/:id/reactivate", post(admin::reactivate_user))
        .route("/admin/users/:id/sign-out", post(admin::sign_out_user))
        .route("/admin/users/:id/password", post(admin::set_password))
        .route("/admin/orgs/:org/members", get(admin::list_members))
        .route("/admin/orgs/:org/members/:user", patch(admin::update_member).delete(admin::remove_member))
        .route("/admin/orgs/:org/invites", get(admin::list_invites).post(admin::create_invites))
        .route("/admin/orgs/:org", patch(admin::update_org).delete(admin::delete_org))
        .route("/admin/invites/:id", delete(admin::revoke_invite))
        .route("/admin/projects", get(admin::list_projects))
        .route("/admin/projects/:p", patch(admin::update_project))
        .route("/admin/projects/:p/archive", post(admin::archive_project))
        .route("/admin/projects/:p/restore", post(admin::restore_project))
        .route("/admin/projects/:p/access", get(admin::list_project_access).post(admin::set_project_access))
        .route("/admin/claims", get(admin::list_claims))
        .route("/admin/claims/release", post(admin::release_claims))
        .route("/admin/checkpoints", get(checkpoints::admin_usage))
        .route("/admin/checkpoints/clear", post(checkpoints::admin_clear))
        .route("/admin/orgs/:org/webhooks", get(admin::list_webhooks).post(admin::create_webhook))
        .route("/admin/webhooks/:id", patch(admin::update_webhook).delete(admin::delete_webhook))
        .route("/admin/webhooks/:id/test", post(admin::test_webhook))
        .route("/admin/email", get(admin::email_status))
        .route("/admin/email/test", post(admin::email_test))
        .route("/admin/backups", get(backups::status))
        .route("/admin/backups/run", post(backups::run_now))
        .route("/admin/storage", get(storage::locations))
        .route("/admin/projects/:p/storage", get(storage::project_storage).post(storage::start_move))
        .route("/admin/storage/check", post(admin::storage_check))
        .route("/admin/storage/gc", get(admin::gc_preview).post(admin::gc_run))
        .route("/admin/audit", get(admin::audit))
        .fallback(|| async { crate::error::ApiError::not_found("That API endpoint") })
}
