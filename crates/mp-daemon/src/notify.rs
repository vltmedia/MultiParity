//! Notifications for the desktop app: the daemon listens to each signed-in account's live event
//! stream and keeps the few events that matter to the person at this computer:
//!
//! - someone else published or merged into a branch one of their folders is on ("get latest");
//! - someone asked them to release a file they have claimed.
//!
//! The app reads them from `GET /local/notifications?after=<id>` and shows them as system
//! notifications and in the tray menu. Which kinds show is a setting kept in
//! `<config dir>/notifications.json`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use mp_client::accounts::{config_dir, Account, Accounts};
use mp_client::workspace::{Registry, Workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

const KEEP: usize = 200;

#[derive(Debug, Clone, Serialize)]
pub struct Notice {
    pub id: u64,
    /// "newer" (others published to your branch) or "nudge" (someone wants your file)
    pub kind: String,
    pub title: String,
    pub body: String,
    pub workspace_id: Option<String>,
    pub project_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Tell me when others publish to a branch I have a folder on.
    pub newer: bool,
    /// Tell me when someone asks me to release a file.
    pub nudges: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { newer: true, nudges: true }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf {
        config_dir().join("notifications.json")
    }
    pub fn load() -> Settings {
        std::fs::read(Self::path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self) -> anyhow::Result<()> {
        mp_client::workspace::write_json_atomic(&Self::path(), self)
    }
}

#[derive(Clone, Default)]
pub struct Notices {
    inner: Arc<Mutex<(u64, VecDeque<Notice>)>>,
    watching: Arc<Mutex<HashSet<String>>>,
}

impl Notices {
    pub async fn push(&self, kind: &str, title: String, body: String, workspace_id: Option<String>, project_id: Option<String>) {
        let mut g = self.inner.lock().await;
        g.0 += 1;
        let n = Notice { id: g.0, kind: kind.into(), title, body, workspace_id, project_id, created_at: chrono_now() };
        g.1.push_back(n);
        while g.1.len() > KEEP {
            g.1.pop_front();
        }
    }

    /// Notices newer than `after`, and the newest id (so a fresh app can start from "now").
    pub async fn since(&self, after: u64) -> (Vec<Notice>, u64) {
        let g = self.inner.lock().await;
        (g.1.iter().filter(|n| n.id > after).cloned().collect(), g.0)
    }

    /// Keep one watcher per signed-in account; new accounts are picked up within a minute.
    pub fn spawn(self) {
        tokio::spawn(async move {
            loop {
                if let Ok(accounts) = Accounts::load() {
                    for a in accounts.accounts {
                        let key = format!("{}|{}", a.id, a.token);
                        if self.watching.lock().await.insert(key.clone()) {
                            let me = self.clone();
                            tokio::spawn(async move {
                                me.watch(a).await;
                                me.watching.lock().await.remove(&key);
                            });
                        }
                    }
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    /// Follow one account's stream, reconnecting with a growing pause. Returns when the account
    /// is signed out (or its token changed: the new one gets its own watcher).
    async fn watch(&self, a: Account) {
        let api = mp_client::Api::for_account(&a);
        let mut pause = 2u64;
        loop {
            let still = Accounts::load().ok().and_then(|x| x.accounts.into_iter().find(|y| y.id == a.id)).map(|y| y.token == a.token).unwrap_or(false);
            if !still {
                return;
            }
            let me = match api.me().await {
                Ok(m) => m,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(pause)).await;
                    pause = (pause * 2).min(300);
                    continue;
                }
            };
            match api.events().await {
                Ok(mut resp) => {
                    pause = 2;
                    let mut buf = String::new();
                    loop {
                        match tokio::time::timeout(Duration::from_secs(90), resp.chunk()).await {
                            Ok(Ok(Some(bytes))) => {
                                buf.push_str(&String::from_utf8_lossy(&bytes));
                                // Events end with a blank line.
                                while let Some(end) = buf.find("\n\n") {
                                    let block: String = buf.drain(..end + 2).collect();
                                    let data: String = block.lines().filter_map(|l| l.strip_prefix("data:")).map(str::trim_start).collect::<Vec<_>>().join("\n");
                                    if let Ok(ev) = serde_json::from_str::<Value>(&data) {
                                        self.consider(&a, &me.id, &me.email, &ev).await;
                                    }
                                }
                            }
                            // Closed, failed, or silent past the 20 s keep-alives: reconnect.
                            _ => break,
                        }
                    }
                }
                Err(e) => tracing::debug!(error = %format!("{e:#}"), server = %a.server, "event stream unavailable"),
            }
            tokio::time::sleep(Duration::from_secs(pause)).await;
            pause = (pause * 2).min(300);
        }
    }

    async fn consider(&self, a: &Account, my_id: &str, my_email: &str, ev: &Value) {
        let kind = ev["kind"].as_str().unwrap_or_default();
        let actor_email = ev["actor"]["email"].as_str().unwrap_or_default();
        let actor = ev["actor"]["name"].as_str().filter(|s| !s.is_empty()).unwrap_or("Someone");
        let project = ev["project_id"].as_str().unwrap_or_default().to_string();
        let data = &ev["data"];
        let settings = Settings::load();
        // This account's folders on this computer, by project.
        let folders: Vec<Workspace> = Registry::workspaces()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| Workspace::open(&r).ok())
            .filter(|w| w.config.account_id == a.id && w.config.project_id == project)
            .collect();
        match kind {
            "change.published" | "change.merged" if settings.newer && !actor_email.eq_ignore_ascii_case(my_email) => {
                let branch = data["branch_id"].as_str().unwrap_or_default();
                let Some(ws) = folders.iter().find(|w| w.config.branch_id == branch) else { return };
                let n = data["number"].as_i64().unwrap_or_default();
                let body = if kind == "change.merged" {
                    format!("{actor} merged {} into {} (change {n}).", data["from"].as_str().unwrap_or("a branch"), ws.config.branch_name)
                } else {
                    let msg = data["message"].as_str().unwrap_or_default();
                    format!("{actor} published change {n}: {msg}")
                };
                self.push("newer", format!("Newer files in {}", ws.config.project_name), body, Some(ws.config.id.clone()), Some(project)).await;
            }
            "claim.nudged" if settings.nudges && data["holder_id"].as_str() == Some(my_id) => {
                let path = data["path"].as_str().unwrap_or_default();
                let name = path.rsplit('/').next().unwrap_or(path);
                let from = data["from"].as_str().unwrap_or(actor);
                let pname = folders.first().map(|w| w.config.project_name.clone()).unwrap_or_else(|| "a project".into());
                self.push("nudge", format!("{from} is waiting for {name}"), format!("In {pname}. Publish or release it when you can."), folders.first().map(|w| w.config.id.clone()), Some(project)).await;
            }
            _ => {}
        }
    }
}

fn chrono_now() -> String {
    // RFC 3339 without pulling in chrono here.
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("{}", d.as_secs())
}

/// Count of folders whose branch moved on since they last got latest, per workspace id; for the
/// tray's "Get latest" line. Cheap: one tree request per folder.
pub async fn behind_counts() -> HashMap<String, usize> {
    let mut out = HashMap::new();
    for root in Registry::workspaces().unwrap_or_default() {
        let Ok(ws) = Workspace::open(&root) else { continue };
        let Ok(api) = mp_client::ops::api_for(&ws) else { continue };
        if let Ok(d) = api.tree(&ws.config.project_id, &ws.config.branch_id, ws.config.synced_change).await {
            let n = d.entries.iter().filter(|e| ws.in_scope(&e.path)).count();
            if n > 0 {
                out.insert(ws.config.id.clone(), n);
            }
        }
    }
    out
}
