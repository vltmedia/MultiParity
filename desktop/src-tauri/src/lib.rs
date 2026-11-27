//! MultiParity desktop app.
//!
//! - Embeds the background daemon (local API on 127.0.0.1:18519, auto-claim, auto-checkpoints),
//!   unless one is already running, so there is one thing to install.
//! - Its own window is the cross-account Projects page (`ui/`): accounts, every project from every
//!   organization, selective sync, connecting folders.
//! - "Open" shows a project in a window on that server's web UI, already signed in.
//! - Lives in the menu bar / system tray, so closing the window keeps syncing.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_updater::UpdaterExt;

const DAEMON: &str = "127.0.0.1:18519";
/// The latest public release, for when no server can be asked about updates.
const GITHUB_LATEST: &str = "https://github.com/vltmedia/MultiParity/releases/latest/download/latest.json";

/// Folder picker for "Sync to this computer".
#[tauri::command]
async fn pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    let mut d = app.dialog().file().set_title("Choose where this project lives on this computer");
    if let Some(s) = start {
        d = d.set_directory(s);
    }
    d.blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| p.display().to_string())
}

/// Suggested parent folder for new projects: ~/MultiParity.
#[tauri::command]
fn default_root() -> String {
    dirs::home_dir().unwrap_or_default().join("MultiParity").display().to_string()
}

/// Open a project (or any page) of a server's web UI in its own window, signed in with the
/// account's token. The token is placed only on that server's origin.
#[tauri::command]
async fn open_server_page(app: AppHandle, account_id: String, path: String) -> Result<(), String> {
    let accounts = mp_client::Accounts::load().map_err(|e| e.to_string())?;
    let a = accounts.by_id(&account_id).map_err(|e| e.to_string())?;
    let origin = a.server.trim_end_matches('/').to_string();
    let url = format!("{origin}{path}").parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let label = format!("server-{}", account_id.chars().filter(|c| c.is_ascii_alphanumeric()).take(12).collect::<String>());
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.navigate(url);
        let _ = w.set_focus();
        return Ok(());
    }
    let script = format!(
        "if (location.origin === {origin:?}) {{ try {{ localStorage.setItem('mp.token', {token:?}); }} catch (e) {{}} }}",
        origin = origin,
        token = a.token
    );
    WebviewWindowBuilder::new(&app, label, WebviewUrl::External(url))
        .title(format!("MultiParity · {}", mp_client::accounts::host_of(&a.server)))
        .inner_size(1360.0, 860.0)
        .min_inner_size(1000.0, 640.0)
        .initialization_script(&script)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// A path inside one of the folders synced on this computer, or an error. Keeps the page from
/// opening anything else.
fn inside_synced_folder(path: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::PathBuf::from(path);
    let p = std::fs::canonicalize(&p).map_err(|_| format!("{path} is not on this computer (it may not be synced here)."))?;
    let roots = mp_client::workspace::Registry::workspaces().map_err(|e| e.to_string())?;
    if roots.iter().filter_map(|r| std::fs::canonicalize(r).ok()).any(|r| p.starts_with(&r)) {
        Ok(p)
    } else {
        Err("That file is not in a synced folder.".into())
    }
}

/// Open a synced file in the app the system uses for it.
#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let p = inside_synced_folder(&path)?;
    tauri_plugin_opener::open_path(p, None::<&str>).map_err(|e| e.to_string())
}

/// Show a synced file in Explorer or Finder.
#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let p = inside_synced_folder(&path)?;
    tauri_plugin_opener::reveal_item_in_dir(p).map_err(|e| e.to_string())
}

/// Follow the daemon's notifications: show each as a system notification, and keep the tray's
/// tooltip and "Get latest" line saying how many projects have newer files.
async fn follow_notifications(app: AppHandle, latest_item: MenuItem<tauri::Wry>) {
    let http = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().expect("http client");
    let base = format!("http://{DAEMON}/local");
    let mut last: Option<u64> = None;
    let mut tick = 0u64;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
        let url = match last {
            Some(n) => format!("{base}/notifications?after={n}"),
            None => format!("{base}/notifications"),
        };
        if let Ok(r) = http.get(&url).send().await {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                if last.is_some() {
                    for n in v["notifications"].as_array().into_iter().flatten() {
                        let _ = app.notification().builder().title(n["title"].as_str().unwrap_or("MultiParity")).body(n["body"].as_str().unwrap_or_default()).show();
                    }
                }
                last = v["last"].as_u64().or(last);
            }
        }
        // The "newer files" count every minute (one small request per synced folder).
        if tick % 15 == 0 {
            if let Ok(r) = http.get(format!("{base}/behind")).send().await {
                if let Ok(v) = r.json::<serde_json::Map<String, serde_json::Value>>().await {
                    let projects = v.len();
                    let (label, tip) = if projects == 0 {
                        ("Get latest for all projects".to_string(), "MultiParity: up to date".to_string())
                    } else {
                        let p = if projects == 1 { "1 project has".to_string() } else { format!("{projects} projects have") };
                        (format!("Get latest for all ({p} newer files)"), format!("MultiParity: {p} newer files"))
                    };
                    let _ = latest_item.set_text(label);
                    if let Some(t) = app.tray_by_id("main") {
                        let _ = t.set_tooltip(Some(tip));
                    }
                }
            }
        }
        tick += 1;
    }
}

// ------------------------------------------------------------------ updates
//
// Each studio server hands out the desktop app (`/api/desktop/update/...`), so a studio decides
// which version its people run. The app asks every server it is signed in to, downloads a newer
// version quietly, and installs it when the person chooses "Restart to update" (tray or app).

#[derive(Default)]
struct Updates {
    /// Version downloaded and ready to install, with its bytes.
    ready: tokio::sync::Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
    checking: std::sync::atomic::AtomicBool,
    last_error: std::sync::Mutex<Option<String>>,
}

#[derive(serde::Serialize)]
struct UpdateStatus {
    current: String,
    ready: Option<String>,
    checking: bool,
    error: Option<String>,
}

async fn status_of(app: &AppHandle) -> UpdateStatus {
    let u = app.state::<Updates>();
    let ready = u.ready.lock().await.as_ref().map(|(x, _)| x.version.clone());
    UpdateStatus {
        current: app.package_info().version.to_string(),
        ready,
        checking: u.checking.load(std::sync::atomic::Ordering::SeqCst),
        error: u.last_error.lock().ok().and_then(|e| e.clone()),
    }
}

/// Ask each signed-in server; download the first newer version found. Returns its version.
async fn check_for_update(app: &AppHandle) -> Result<Option<String>, String> {
    let u = app.state::<Updates>();
    if let Some((x, _)) = u.ready.lock().await.as_ref() {
        return Ok(Some(x.version.clone()));
    }
    if u.checking.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Ok(None);
    }
    let result = async {
        let accounts = mp_client::Accounts::load().map_err(|e| e.to_string())?;
        let mut servers: Vec<String> = accounts.accounts.iter().map(|a| a.server.trim_end_matches('/').to_string()).collect();
        servers.dedup();
        let mut last_err = None;
        // Each server answers from its own build, or the latest GitHub release when it has none.
        // GitHub directly only when no server could be asked.
        let mut endpoints: Vec<String> = servers.iter().map(|s| format!("{s}/api/desktop/update/{{{{target}}}}/{{{{arch}}}}/{{{{current_version}}}}")).collect();
        endpoints.push(GITHUB_LATEST.to_string());
        let mut asked_a_server = false;
        for (i, url) in endpoints.into_iter().enumerate() {
            let github = i == servers.len();
            if github && asked_a_server {
                break;
            }
            let server = if github { "GitHub".to_string() } else { servers[i].clone() };
            let Ok(url) = url.parse::<tauri::Url>() else { continue };
            let updater = match app.updater_builder().endpoints(vec![url]).and_then(|b| b.build()) {
                Ok(x) => x,
                Err(e) => { last_err = Some(e.to_string()); continue }
            };
            match updater.check().await {
                Ok(Some(update)) => {
                    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| format!("downloading {}: {e}", update.version))?;
                    let v = update.version.clone();
                    *u.ready.lock().await = Some((update, bytes));
                    return Ok(Some(v));
                }
                Ok(None) => asked_a_server |= !github,
                Err(e) => last_err = Some(format!("{server}: {e}")),
            }
        }
        match last_err {
            Some(e) if !asked_a_server => Err(e),
            _ => Ok(None),
        }
    }
    .await;
    u.checking.store(false, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut e) = u.last_error.lock() {
        *e = result.as_ref().err().cloned();
    }
    result
}

/// Check now and then: shortly after start, then every six hours.
async fn follow_updates(app: AppHandle, item: MenuItem<tauri::Wry>) {
    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    loop {
        if let Ok(Some(v)) = check_for_update(&app).await {
            let _ = item.set_text(format!("Restart to update to {v}"));
            let _ = item.set_enabled(true);
            let _ = app.notification().builder().title("MultiParity update ready").body(format!("Version {v} is downloaded. Restart MultiParity when it suits you.")).show();
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
    }
}

async fn install_ready(app: &AppHandle) -> Result<(), String> {
    let u = app.state::<Updates>();
    let Some((update, bytes)) = u.ready.lock().await.take() else { return Err("No update is ready yet.".into()) };
    // On Windows the installer closes the app and starts it again; elsewhere restart here.
    update.install(bytes).map_err(|e| e.to_string())?;
    app.restart();
}

#[tauri::command]
async fn update_status(app: AppHandle) -> UpdateStatus {
    status_of(&app).await
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<UpdateStatus, String> {
    check_for_update(&app).await?;
    Ok(status_of(&app).await)
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    install_ready(&app).await
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

async fn get_latest_everywhere() {
    let Ok(roots) = mp_client::workspace::Registry::workspaces() else { return };
    for root in roots {
        let Ok(mut ws) = mp_client::Workspace::open(&root) else { continue };
        let Ok(api) = mp_client::ops::api_for(&ws) else { continue };
        if let Err(e) = mp_client::ops::get_latest(&api, &mut ws, |_| {}).await {
            tracing::warn!(error = %e, root = %root.display(), "get latest failed");
        }
    }
}

pub fn run() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_env("MP_LOG").unwrap_or_else(|_| "info".into())).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Updates::default())
        .invoke_handler(tauri::generate_handler![pick_folder, default_root, open_server_page, open_path, reveal_path, update_status, check_update, install_update])
        .setup(|app| {
            // The daemon runs inside the app unless the command-line one is already running.
            tauri::async_runtime::spawn(async {
                match mp_daemon::spawn(DAEMON, 20).await {
                    Ok(true) => tracing::info!("started the background daemon"),
                    Ok(false) => tracing::info!("using the daemon that is already running"),
                    Err(e) => tracing::error!(error = %e, "could not start the daemon"),
                }
            });

            let open = MenuItem::with_id(app, "open", "Open MultiParity", true, None::<&str>)?;
            let latest = MenuItem::with_id(app, "latest", "Get latest for all projects", true, None::<&str>)?;
            let update = MenuItem::with_id(app, "update", format!("MultiParity {}", app.package_info().version), false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit MultiParity", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &latest, &PredefinedMenuItem::separator(app)?, &update, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main").menu(&menu).tooltip("MultiParity").show_menu_on_left_click(true);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "open" => show_main(app),
                "latest" => {
                    tauri::async_runtime::spawn(get_latest_everywhere());
                }
                "update" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = install_ready(&app).await {
                            tracing::warn!(error = %e, "update failed");
                        }
                    });
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .build(app)?;
            tauri::async_runtime::spawn(follow_notifications(app.handle().clone(), latest.clone()));
            tauri::async_runtime::spawn(follow_updates(app.handle().clone(), update.clone()));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps the app (and syncing) running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running MultiParity");
}
