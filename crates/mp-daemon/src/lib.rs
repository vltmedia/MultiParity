//! The background daemon. It serves a small local API on the loopback interface for the desktop
//! app and the web UI (selective sync, status, publish), and every few seconds it scans each
//! connected folder to claim claimable files on first edit and take automatic checkpoints.
//!
//! Security: bound to 127.0.0.1 only, and any request that carries an `Origin` header must come
//! from a signed-in server's web UI, the desktop app, or localhost; otherwise a web page in the
//! person's browser could drive it.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use mp_client::accounts::Accounts;
use mp_client::checkpoints::{self, CheckpointOptions, CheckpointOutcome};
use mp_client::ops;
use mp_client::scan;
use mp_client::workspace::{Registry, Workspace};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, CorsLayer};

#[derive(Clone, Default)]
struct DState {
    locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    last_auto: Arc<Mutex<HashMap<String, (Instant, String)>>>,
    last_trim: Arc<Mutex<HashMap<String, Instant>>>,
    /// Why the last automatic checkpoint was skipped or what it left out, per folder.
    notices: Arc<Mutex<HashMap<String, String>>>,
}

impl DState {
    async fn lock_for(&self, id: &str) -> Arc<Mutex<()>> {
        self.locks.lock().await.entry(id.to_string()).or_default().clone()
    }
}

struct DErr(StatusCode, String);

impl IntoResponse for DErr {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": { "code": if self.0 == StatusCode::NOT_FOUND { "not_found" } else { "error" }, "message": self.1 } }))).into_response()
    }
}

impl From<anyhow::Error> for DErr {
    fn from(e: anyhow::Error) -> Self {
        DErr(StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}"))
    }
}

type DResult<T> = Result<T, DErr>;

fn find_ws(id: &str) -> DResult<Workspace> {
    for root in Registry::workspaces()? {
        if let Ok(ws) = Workspace::open(&root) {
            if ws.config.id == id || ws.config.project_id == id {
                return Ok(ws);
            }
        }
    }
    Err(DErr(StatusCode::NOT_FOUND, "That project is not connected on this computer.".into()))
}

fn allowed_origin(origin: &str, servers: &HashSet<String>) -> bool {
    let o = origin.trim_end_matches('/');
    servers.contains(o)
        || o == "tauri://localhost"
        || o == "http://tauri.localhost"
        || o == "https://tauri.localhost"
        || o.starts_with("http://localhost:")
        || o.starts_with("http://127.0.0.1:")
        || o == "http://localhost"
}

fn server_origins() -> HashSet<String> {
    Accounts::load()
        .map(|a| a.accounts.iter().map(|x| x.server.trim_end_matches('/').to_string()).collect())
        .unwrap_or_default()
}

async fn origin_guard(req: Request, next: Next) -> Response {
    if let Some(origin) = req.headers().get("origin").and_then(|v| v.to_str().ok()) {
        if !allowed_origin(origin, &server_origins()) {
            return DErr(StatusCode::FORBIDDEN, "This page is not allowed to use the MultiParity app on this computer.".into()).into_response();
        }
    }
    next.run(req).await
}

/// Start the daemon in the background of an app that embeds it (the desktop app). Returns
/// `Ok(false)` without starting when another daemon already answers on that address.
pub async fn spawn(listen: &str, interval: u64) -> Result<bool> {
    let addr: std::net::SocketAddr = listen.parse()?;
    if tokio::net::TcpStream::connect(addr).await.is_ok() {
        return Ok(false);
    }
    let listen = listen.to_string();
    tokio::spawn(async move {
        if let Err(e) = run(&listen, interval).await {
            tracing::error!(error = %format!("{e:#}"), "daemon stopped");
        }
    });
    Ok(true)
}

pub async fn run(listen: &str, interval: u64) -> Result<()> {
    let addr: std::net::SocketAddr = listen.parse()?;
    anyhow::ensure!(addr.ip().is_loopback(), "The daemon only listens on this computer (127.0.0.1 or ::1), not {listen}.");
    let state = DState::default();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|o: &HeaderValue, _| o.to_str().map(|s| allowed_origin(s, &server_origins())).unwrap_or(false)))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([axum::http::header::CONTENT_TYPE]);
    let app = Router::new()
        .route("/local/health", get(health))
        .route("/local/workspaces", get(workspaces))
        .route("/local/workspaces/:id/status", get(status))
        .route("/local/workspaces/:id/folders", get(folders))
        .route("/local/workspaces/:id/sync", put(set_sync))
        .route("/local/workspaces/:id/get-latest", post(get_latest))
        .route("/local/workspaces/:id/publish", post(publish))
        .route("/local/workspaces/:id/checkpoint", post(checkpoint))
        .route("/local/workspaces/:id/checkpoints", get(checkpoints))
        .route("/local/workspaces/:id/ignore/suggest", get(ignore_suggest))
        .route("/local/workspaces/:id/ignore", post(ignore_add))
        .route("/local/workspaces/:id/no-checkpoint", post(no_checkpoint_add))
        .route("/local/workspaces/:id/space", get(space))
        .route("/local/workspaces/:id/disconnect", post(disconnect))
        .route("/local/workspaces/:id/draft", post(save_draft))
        .route("/local/workspaces/:id/checkpoints/:cp", get(checkpoint_detail))
        .route("/local/workspaces/:id/rewind", post(rewind))
        .route("/local/workspaces/:id/discard", post(discard))
        .route("/local/accounts", get(accounts))
        .route("/local/accounts/login", post(account_login))
        .route("/local/accounts/:id", axum::routing::delete(account_logout))
        .route("/local/accounts/:id/session", get(account_session))
        .route("/local/projects", get(all_projects))
        .route("/local/connect", post(connect))
        .layer(middleware::from_fn(origin_guard))
        .layer(cors)
        .with_state(state.clone());

    let bg = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(interval.max(5))).await;
            if let Err(e) = background_pass(&bg).await {
                tracing::warn!(error = %format!("{e:#}"), "background scan failed");
            }
        }
    });

    eprintln!("MultiParity daemon listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// One pass over every connected folder: claim claimable files on first edit and, where
/// automatic checkpoints are turned on, take one when the set of edits changed and the last one
/// is over five minutes old.
async fn background_pass(state: &DState) -> Result<()> {
    for root in Registry::workspaces()? {
        let Ok(mut ws) = Workspace::open(&root) else { continue };
        let lock = state.lock_for(&ws.config.id).await;
        let Ok(_guard) = lock.try_lock() else { continue };
        // Keep checkpoint data within the folder's space limit: on start, then hourly.
        {
            let mut trims = state.last_trim.lock().await;
            if trims.get(&ws.config.id).map(|t| t.elapsed() >= Duration::from_secs(3600)).unwrap_or(true) {
                trims.insert(ws.config.id.clone(), Instant::now());
                let api = ops::api_for(&ws).ok();
                if let Some(api) = &api {
                    // Where the project keeps checkpoints may have changed.
                    let _ = checkpoints::refresh_project_settings(api, &mut ws).await;
                }
                match ops::trim_cache(api.as_ref(), &ws).await {
                    Ok(r) if r.freed_parts > 0 || !r.removed_checkpoints.is_empty() => {
                        tracing::info!(folder = %ws.root.display(), freed = r.freed_bytes, kept = r.kept_bytes, removed = ?r.removed_checkpoints, "trimmed checkpoint data")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(error = %e, "could not trim checkpoint data"),
                }
            }
        }
        let root2: PathBuf = ws.root.clone();
        let st = tokio::task::spawn_blocking(move || -> Result<scan::Status> {
            let ws = Workspace::open(&root2)?;
            scan::status(&ws, &[])
        })
        .await??;
        if st.is_clean() {
            continue;
        }
        if ws.config.auto_claim {
            let wanted: Vec<String> = st.edited.iter().chain(&st.new).map(|f| f.path.clone()).filter(|p| mp_core::kind::is_claimable(p)).collect();
            if !wanted.is_empty() {
                if let Ok(api) = ops::api_for(&ws) {
                    let mine: HashSet<String> = api
                        .claims(&ws.config.project_id)
                        .await
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|c| c.user.as_ref().map(|u| u.email == ws.config.account_email).unwrap_or(false))
                        .map(|c| c.path)
                        .collect();
                    let todo: Vec<String> = wanted.into_iter().filter(|p| !mine.contains(p)).collect();
                    if !todo.is_empty() {
                        match api.claim(&ws.config.project_id, &ws.config.branch_id, &todo, &ops::device_name()).await {
                            Ok((claimed, held)) => {
                                for p in claimed {
                                    tracing::info!(path = %p, "claimed on first edit");
                                }
                                for c in held {
                                    tracing::warn!(path = %c.path, holder = ?c.user.map(|u| u.name), "edited a file someone else has claimed");
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "auto-claim failed"),
                        }
                    }
                }
            }
        }
        if ws.config.auto_checkpoints_on() {
            let signature = {
                let mut v: Vec<String> = st.new.iter().chain(&st.edited).map(|f| format!("{}:{}:{}", f.path, f.size, f.mtime)).collect();
                v.extend(st.deleted.iter().map(|f| format!("-{}", f.path)));
                v.sort();
                v.join("|")
            };
            let mut last = state.last_auto.lock().await;
            let due = match last.get(&ws.config.id) {
                Some((t, sig)) => *sig != signature && t.elapsed() >= Duration::from_secs(300),
                None => true,
            };
            if due {
                let api = ops::api_for(&ws).ok();
                match checkpoints::checkpoint(api.as_ref(), &ws, CheckpointOptions::auto()).await {
                    Ok(out) => {
                        // Recorded whether or not one was made, so work that does not fit is
                        // not read again on every pass.
                        last.insert(ws.config.id.clone(), (Instant::now(), signature));
                        let mut notices = state.notices.lock().await;
                        match notice_for(&out) {
                            Some(n) => {
                                if notices.get(&ws.config.id) != Some(&n) {
                                    tracing::info!(folder = %ws.root.display(), "{n}");
                                }
                                notices.insert(ws.config.id.clone(), n);
                            }
                            None => {
                                notices.remove(&ws.config.id);
                            }
                        }
                        if out.checkpoint.is_some() && !out.on_server {
                            let _ = ops::trim_cache(api.as_ref(), &ws).await;
                        }
                    }
                    Err(e) => {
                        // Usually a file that was still being saved: try again in five minutes.
                        tracing::warn!(error = %format!("{e:#}"), "automatic checkpoint failed");
                        last.insert(ws.config.id.clone(), (Instant::now(), String::new()));
                    }
                }
            }
        }
    }
    Ok(())
}

fn human(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.0} KB", (b / 1e3).max(1.0))
    }
}

/// What to tell the person about a checkpoint that was skipped or left files out.
fn notice_for(out: &CheckpointOutcome) -> Option<String> {
    if let Some(t) = &out.too_big {
        return Some(format!(
            "Automatic checkpoint skipped: this work needs {} and {} is available for checkpoints on this computer. Save a draft to keep it on the server, or raise the limit.",
            human(t.needed_bytes),
            human(t.available_bytes)
        ));
    }
    if !out.left_out.is_empty() {
        return Some(format!("{} file(s) ({}) are left out of checkpoints: render output and caches can be made again.", out.left_out.len(), human(out.left_out_bytes)));
    }
    None
}

async fn health() -> Json<Value> {
    let n = Registry::workspaces().map(|v| v.len()).unwrap_or(0);
    Json(json!({ "version": env!("CARGO_PKG_VERSION"), "workspaces": n }))
}

/// A folder path as people write it: without the `\\?\` prefix Windows adds to full paths.
fn display_path(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

fn ws_json(ws: &Workspace) -> Value {
    let total: u64 = ws.remote.files.values().map(|r| r.size).sum();
    let local: u64 = ws.index.files.values().map(|i| i.size).sum();
    json!({
        "id": ws.config.id, "root": display_path(&ws.root), "server": ws.config.server, "account_email": ws.config.account_email,
        "project_id": ws.config.project_id, "project_name": ws.config.project_name, "org_name": ws.config.org_name,
        "branch_id": ws.config.branch_id, "branch_name": ws.config.branch_name, "synced_change": ws.config.synced_change,
        "include": ws.config.include, "exclude": ws.config.exclude, "local_bytes": local, "total_bytes": total,
        "checkpoint_storage": if ws.config.checkpoints_on_server() { "server" } else { "local" },
    })
}

async fn workspaces() -> DResult<Json<Vec<Value>>> {
    let mut out = Vec::new();
    for root in Registry::workspaces()? {
        if let Ok(ws) = Workspace::open(&root) {
            out.push(ws_json(&ws));
        }
    }
    Ok(Json(out))
}

async fn status(State(s): State<DState>, Path(id): Path<String>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let root = ws.root.clone();
    let (st, cps) = tokio::task::spawn_blocking(move || -> Result<_> {
        let ws = Workspace::open(&root)?;
        Ok((scan::status(&ws, &[])?, ops::list_checkpoints(&ws)?))
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))??;
    let claimed: HashSet<String> = match ops::api_for(&ws) {
        Ok(api) => api.claims(&ws.config.project_id).await.unwrap_or_default().into_iter().filter(|c| c.user.as_ref().map(|u| u.email == ws.config.account_email).unwrap_or(false)).map(|c| c.path).collect(),
        Err(_) => HashSet::new(),
    };
    let f = |list: &Vec<scan::LocalFile>| -> Vec<Value> { list.iter().map(|x| json!({ "path": x.path, "size": x.size, "claimed": claimed.contains(&x.path) })).collect() };
    let behind = match ops::api_for(&ws) {
        Ok(api) => api.tree(&ws.config.project_id, &ws.config.branch_id, ws.config.synced_change).await.map(|d| d.entries.iter().filter(|e| ws.in_scope(&e.path)).count()).unwrap_or(0),
        Err(_) => 0,
    };
    let notice = s.notices.lock().await.get(&ws.config.id).cloned();
    Ok(Json(json!({
        "new": f(&st.new), "edited": f(&st.edited), "deleted": f(&st.deleted), "behind": behind, "ignored": st.ignored.len(),
        "checkpoints": cps.iter().take(20).map(checkpoint_json).collect::<Vec<_>>(),
        "checkpoint_notice": notice,
        "auto_checkpoints": ws.config.auto_checkpoints_on(),
        "checkpoint_storage": if ws.config.checkpoints_on_server() { "server" } else { "local" },
    })))
}

async fn folders(Path(id): Path<String>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    Ok(Json(serde_json::to_value(ops::folder_tree(&ws)).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SyncBody {
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

async fn set_sync(State(s): State<DState>, Path(id): Path<String>, Json(b): Json<SyncBody>) -> DResult<Json<Value>> {
    let mut ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws)?;
    let r = ops::set_scope(&api, &mut ws, b.include, b.exclude).await?;
    let mut v = ws_json(&ws);
    v["report"] = serde_json::to_value(r).unwrap_or_default();
    Ok(Json(v))
}

async fn get_latest(State(s): State<DState>, Path(id): Path<String>) -> DResult<Json<Value>> {
    let mut ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws)?;
    let r = ops::get_latest(&api, &mut ws, |_| {}).await?;
    Ok(Json(json!({ "downloaded": r.downloaded.len(), "bytes": r.bytes, "removed": r.removed.len(), "kept_yours": r.kept_yours, "head": r.head })))
}

#[derive(Deserialize)]
struct PublishBody {
    message: String,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    keep_mine: Vec<String>,
}

async fn publish(State(s): State<DState>, Path(id): Path<String>, Json(b): Json<PublishBody>) -> Response {
    let run = async {
        let mut ws = find_ws(&id)?;
        let lock = s.lock_for(&ws.config.id).await;
        let _g = lock.lock().await;
        let api = ops::api_for(&ws)?;
        Ok::<_, DErr>(ops::publish(&api, &mut ws, &b.message, &b.paths, &b.keep_mine, |_| {}).await)
    };
    match run.await {
        Err(e) => e.into_response(),
        Ok(Ok(r)) => Json(json!({ "change": r.change, "files": r.files })).into_response(),
        Ok(Err(e)) => match mp_client::api::conflict_details(&e) {
            Some(d) => (StatusCode::CONFLICT, Json(json!({ "error": { "code": "conflict", "message": e.to_string(), "details": d } }))).into_response(),
            None => DErr::from(e).into_response(),
        },
    }
}

fn checkpoint_json(c: &checkpoints::Checkpoint) -> Value {
    json!({ "id": c.id, "note": c.note, "auto": c.auto, "system": c.system, "on_server": c.on_server, "created_at": c.created_at, "files": c.files.len() })
}

#[derive(Deserialize, Default)]
struct CheckpointBody {
    note: Option<String>,
    /// Include render output and caches too.
    #[serde(default)]
    all: bool,
}

async fn checkpoint(State(s): State<DState>, Path(id): Path<String>, body: Option<Json<CheckpointBody>>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let api = ops::api_for(&ws).ok();
    let mut opts = CheckpointOptions::manual(body.note.as_deref());
    opts.include_all = body.all;
    let out = checkpoints::checkpoint(api.as_ref(), &ws, opts).await?;
    if let Some(t) = &out.too_big {
        return Err(DErr(
            StatusCode::CONFLICT,
            format!("This checkpoint needs {} and this computer has {} to spare. Save a draft to keep the work on the server instead.", human(t.needed_bytes), human(t.available_bytes)),
        ));
    }
    match &out.checkpoint {
        Some(c) => {
            let mut v = checkpoint_json(c);
            v["left_out"] = json!(out.left_out.len());
            v["left_out_bytes"] = json!(out.left_out_bytes);
            Ok(Json(v))
        }
        None if !out.left_out.is_empty() => Err(DErr(
            StatusCode::CONFLICT,
            format!("Only render output and caches have changed ({} files), and checkpoints leave those out.", out.left_out.len()),
        )),
        None => Err(DErr(StatusCode::CONFLICT, "Nothing to checkpoint: no unpublished changes.".into())),
    }
}

async fn checkpoints(Path(id): Path<String>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let list = ops::list_checkpoints(&ws)?;
    Ok(Json(json!(list.iter().map(checkpoint_json).collect::<Vec<_>>())))
}

/// Disk space MultiParity uses in this folder beyond the files themselves; trims what is not
/// needed.
async fn space(State(s): State<DState>, Path(id): Path<String>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws).ok();
    let r = ops::trim_cache(api.as_ref(), &ws).await?;
    Ok(Json(serde_json::to_value(r).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SuggestQuery {
    path: String,
}

async fn ignore_suggest(Path(id): Path<String>, Query(q): Query<SuggestQuery>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let path = mp_core::paths::normalize(&q.path).ok_or_else(|| DErr(StatusCode::UNPROCESSABLE_ENTITY, "That is not a path inside the project.".into()))?;
    let choices = tokio::task::spawn_blocking(move || scan::ignore_choices(&ws, &path)).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(Json(serde_json::to_value(choices).unwrap_or_default()))
}

#[derive(Deserialize)]
struct IgnoreBody {
    pattern: String,
    #[serde(default)]
    personal: bool,
}

// ---------------------------------------------------------------------------------------------
// Accounts and projects across every signed-in account (the desktop Projects page).

fn account_json(a: &mp_client::Account) -> Value {
    json!({ "id": a.id, "server": a.server, "email": a.email, "name": a.name, "label": a.label })
}

async fn accounts() -> DResult<Json<Vec<Value>>> {
    Ok(Json(Accounts::load()?.accounts.iter().map(account_json).collect()))
}

#[derive(Deserialize)]
struct LoginBody {
    server: String,
    email: String,
    password: String,
    label: Option<String>,
}

async fn account_login(Json(b): Json<LoginBody>) -> DResult<Json<Value>> {
    let server = if b.server.starts_with("http://") || b.server.starts_with("https://") { b.server.clone() } else { format!("https://{}", b.server) };
    let api = mp_client::Api::new(&server, None);
    let (token, me) = api.login(&b.email, &b.password, &ops::device_name()).await.map_err(|e| DErr(StatusCode::UNAUTHORIZED, e.to_string()))?;
    let mut accounts = Accounts::load()?;
    let id = accounts.upsert(mp_client::Account { id: uuid::Uuid::new_v4().to_string(), server: api.server.clone(), email: me.email, name: me.name, token, label: b.label.filter(|l| !l.trim().is_empty()) });
    accounts.save()?;
    Ok(Json(account_json(accounts.by_id(&id)?)))
}

async fn account_logout(Path(id): Path<String>) -> DResult<Json<Value>> {
    let mut accounts = Accounts::load()?;
    let a = accounts.by_id(&id).map_err(|e| DErr(StatusCode::NOT_FOUND, e.to_string()))?.clone();
    let _ = mp_client::Api::for_account(&a).post::<_, Value>("/auth/logout", &json!({})).await;
    accounts.accounts.retain(|x| x.id != id);
    accounts.save()?;
    Ok(Json(json!({})))
}

/// The account's token, for the desktop app to open that server's web UI signed in. Never
/// handed to web pages: only requests without an Origin (the app itself) or from the app's own
/// webview origin get it.
async fn account_session(headers: axum::http::HeaderMap, Path(id): Path<String>) -> DResult<Json<Value>> {
    if let Some(o) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        let app = o == "tauri://localhost" || o == "http://tauri.localhost" || o == "https://tauri.localhost";
        if !app {
            return Err(DErr(StatusCode::FORBIDDEN, "Only the MultiParity app can read sign-in tokens.".into()));
        }
    }
    let accounts = Accounts::load()?;
    let a = accounts.by_id(&id).map_err(|e| DErr(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(Json(json!({ "server": a.server, "token": a.token })))
}

/// Every project from every signed-in account, with the folders connected on this computer.
async fn all_projects() -> DResult<Json<Value>> {
    let accounts = Accounts::load()?;
    let local: Vec<Workspace> = Registry::workspaces()?.into_iter().filter_map(|r| Workspace::open(&r).ok()).collect();
    let mut groups = Vec::new();
    for a in &accounts.accounts {
        let api = mp_client::Api::for_account(a);
        match api.projects().await {
            Ok(list) => {
                let projects: Vec<Value> = list
                    .into_iter()
                    .map(|p| {
                        let ws: Vec<Value> = local.iter().filter(|w| w.config.project_id == p.id && w.config.account_id == a.id).map(ws_json).collect();
                        json!({ "project": p, "workspaces": ws })
                    })
                    .collect();
                groups.push(json!({ "account": account_json(a), "projects": projects, "error": null }));
            }
            Err(e) => groups.push(json!({ "account": account_json(a), "projects": [], "error": e.to_string() })),
        }
    }
    Ok(Json(json!({ "accounts": groups })))
}

#[derive(Deserialize)]
struct ConnectBody {
    account_id: String,
    project_id: String,
    branch: Option<String>,
    folder: String,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    dry_run: bool,
    #[serde(default = "yes")]
    download: bool,
}

fn yes() -> bool {
    true
}

/// Connect a project to a folder. With `dry_run`, report what is already in the folder (matched,
/// different, only here, to download) and change nothing.
async fn connect(Json(b): Json<ConnectBody>) -> DResult<Json<Value>> {
    let accounts = Accounts::load()?;
    let a = accounts.by_id(&b.account_id).map_err(|e| DErr(StatusCode::NOT_FOUND, e.to_string()))?;
    let api = mp_client::Api::for_account(a);
    let project = api.project(&b.project_id).await?;
    let branch = match &b.branch {
        Some(k) => project.branches_list.iter().find(|x| &x.id == k || x.name.eq_ignore_ascii_case(k)).cloned(),
        None => project.branches_list.iter().find(|x| x.parent_id.is_none()).cloned(),
    }
    .ok_or_else(|| DErr(StatusCode::NOT_FOUND, "That branch was not found.".into()))?;
    let folder = PathBuf::from(&b.folder);
    if !folder.is_absolute() {
        return Err(DErr(StatusCode::UNPROCESSABLE_ENTITY, "Choose a full folder path.".into()));
    }
    let (ws, report) = ops::connect(
        &api,
        &folder,
        ops::ConnectOptions { account_id: &a.id, project: &project, branch: &branch, include: b.include, exclude: b.exclude, download: b.download, dry_run: b.dry_run },
    )
    .await?;
    Ok(Json(json!({ "report": report, "workspace": ws.as_ref().map(ws_json) })))
}

#[derive(Deserialize)]
struct DraftBody {
    title: String,
}

/// Save this folder's unpublished work as a draft on the server.
async fn save_draft(State(s): State<DState>, Path(id): Path<String>, Json(b): Json<DraftBody>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws)?;
    Ok(Json(ops::save_draft(&api, &ws, &b.title).await?))
}

/// One checkpoint with its files, for the rewind screen. Works for checkpoints kept on the
/// server too, from any computer.
async fn checkpoint_detail(Path((id, cp)): Path<(String, String)>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let api = ops::api_for(&ws)?;
    let c = checkpoints::load_checkpoint(&api, &ws, &cp).await.map_err(|e| DErr(StatusCode::NOT_FOUND, e.to_string()))?;
    let mut v = checkpoint_json(&c);
    v["files"] = json!(c
        .files
        .iter()
        .map(|f| json!({ "path": f.path, "deleted": f.manifest.is_none(), "size": f.manifest.as_ref().map(|m| m.size).unwrap_or(0) }))
        .collect::<Vec<_>>());
    Ok(Json(v))
}

#[derive(Deserialize)]
struct RewindBody {
    checkpoint: String,
    /// Only these files or folders; all of the checkpoint's files when empty.
    #[serde(default)]
    paths: Vec<String>,
}

/// Put files back as they were at a checkpoint. The current state is checkpointed first, so the
/// answer names the checkpoint that undoes it.
async fn rewind(State(s): State<DState>, Path(id): Path<String>, Json(b): Json<RewindBody>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws)?;
    let (undo, n) = checkpoints::rewind(&api, &ws, &b.checkpoint, &b.paths).await?;
    Ok(Json(json!({ "files": n, "undo": undo.map(|c| c.id) })))
}

#[derive(Deserialize)]
struct DiscardBody {
    paths: Vec<String>,
}

/// Throw away edits to these files: they go back to the server's version. The current state is
/// checkpointed first (or moved aside, for files checkpoints leave out).
async fn discard(State(s): State<DState>, Path(id): Path<String>, Json(b): Json<DiscardBody>) -> DResult<Json<Value>> {
    if b.paths.is_empty() {
        return Err(DErr(StatusCode::UNPROCESSABLE_ENTITY, "Choose the files to discard.".into()));
    }
    let mut ws = find_ws(&id)?;
    let lock = s.lock_for(&ws.config.id).await;
    let _g = lock.lock().await;
    let api = ops::api_for(&ws)?;
    let (undo, n) = ops::discard(&api, &mut ws, &b.paths).await?;
    Ok(Json(json!({ "files": n, "undo": undo.map(|c| c.id) })))
}

/// Stop syncing a folder on this computer. Files stay where they are.
async fn disconnect(Path(id): Path<String>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    Registry::remove(&ws.root)?;
    Ok(Json(json!({ "root": display_path(&ws.root) })))
}

async fn ignore_add(Path(id): Path<String>, Json(b): Json<IgnoreBody>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    if b.pattern.trim().is_empty() || b.pattern.contains('\n') {
        return Err(DErr(StatusCode::UNPROCESSABLE_ENTITY, "Enter one pattern.".into()));
    }
    let file = ops::add_ignore(&ws, &b.pattern, b.personal)?;
    Ok(Json(json!({ "file": file.display().to_string() })))
}

/// "Don't checkpoint files like this": add a rule to the shared or personal list.
async fn no_checkpoint_add(Path(id): Path<String>, Json(b): Json<IgnoreBody>) -> DResult<Json<Value>> {
    let ws = find_ws(&id)?;
    if b.pattern.trim().is_empty() || b.pattern.contains('\n') {
        return Err(DErr(StatusCode::UNPROCESSABLE_ENTITY, "Enter one pattern.".into()));
    }
    let file = checkpoints::add_no_checkpoint(&ws, &b.pattern, b.personal)?;
    Ok(Json(json!({ "file": file.display().to_string() })))
}
