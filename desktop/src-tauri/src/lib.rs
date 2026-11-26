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

const DAEMON: &str = "127.0.0.1:18519";

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
        .invoke_handler(tauri::generate_handler![pick_folder, default_root, open_server_page])
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
            let quit = MenuItem::with_id(app, "quit", "Quit MultiParity", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &latest, &PredefinedMenuItem::separator(app)?, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main").menu(&menu).tooltip("MultiParity").show_menu_on_left_click(true);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "open" => show_main(app),
                "latest" => {
                    tauri::async_runtime::spawn(get_latest_everywhere());
                }
                "quit" => app.exit(0),
                _ => {}
            })
            .build(app)?;
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
