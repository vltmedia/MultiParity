//! Automation from the command line:
//!
//! - `mp watch` prints events as they happen, or runs a command for each (event scripting).
//! - `mp mcp` serves MultiParity to AI assistants over the Model Context Protocol (stdio).

use std::time::Duration;

use anyhow::{Context, Result};
use mp_client::accounts::Accounts;
use mp_client::api::Api;
use mp_client::workspace::{Registry, Workspace};
use mp_core::mcp;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// `change.published` matches `change.published`, `change.*` and `*`; no patterns match all.
fn matches(patterns: &[String], kind: &str) -> bool {
    patterns.is_empty() || patterns.iter().any(|p| p == "*" || p == kind || p.strip_suffix(".*").map(|pre| kind.starts_with(&format!("{pre}."))).unwrap_or(false))
}

fn api_for_account(account: Option<&str>) -> Result<Api> {
    let accounts = Accounts::load()?;
    Ok(Api::for_account(accounts.pick(account)?))
}

/// A project by name (any case), slug or id.
async fn project_id(api: &Api, key: &str) -> Result<String> {
    let list = api.projects().await?;
    let k = key.trim().to_lowercase();
    list.iter()
        .find(|p| p.id == key || p.name.to_lowercase() == k || p.slug.to_lowercase() == k)
        .map(|p| p.id.clone())
        .with_context(|| format!("There is no project called {key}. `mp projects` lists them."))
}

// ---------------------------------------------------------------------------------------------
// mp watch

pub async fn watch(account: Option<String>, project: Option<String>, kinds: Vec<String>, exec: Option<String>) -> Result<()> {
    let api = api_for_account(account.as_deref())?;
    let pid = match &project {
        Some(p) => Some(project_id(&api, p).await?),
        None => None,
    };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    // One event at a time, in order, so scripts never overlap.
    let runner = tokio::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match &exec {
                Some(cmd) => run_hook(cmd, &ev).await,
                None => println!("{}", serde_json::to_string(&ev).unwrap_or_default()),
            }
        }
    });
    eprintln!("Watching {}{}. Stop with Ctrl+C.", api.server, project.as_deref().map(|p| format!(", project {p}")).unwrap_or_default());
    let mut pause = 2u64;
    loop {
        let started = std::time::Instant::now();
        let r = api
            .each_event(|ev| {
                let kind = ev["kind"].as_str().unwrap_or_default();
                let in_project = pid.as_deref().map(|p| ev["project_id"].as_str() == Some(p)).unwrap_or(true);
                if in_project && matches(&kinds, kind) {
                    let _ = tx.send(ev);
                }
            })
            .await;
        if runner.is_finished() {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(60) {
            pause = 2;
        }
        match r {
            Err(e) => eprintln!("{e:#}. Trying again in {pause} s."),
            Ok(()) => eprintln!("The server closed the event stream. Trying again in {pause} s."),
        }
        tokio::time::sleep(Duration::from_secs(pause)).await;
        pause = (pause * 2).min(60);
    }
}

/// Run `cmd` through the shell with the event as JSON on stdin and in MP_EVENT_* variables.
async fn run_hook(cmd: &str, ev: &Value) {
    let mut c = if cfg!(windows) {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(cmd);
        c
    } else {
        let mut c = tokio::process::Command::new("sh");
        c.arg("-c").arg(cmd);
        c
    };
    let json = serde_json::to_string(ev).unwrap_or_default();
    c.env("MP_EVENT_KIND", ev["kind"].as_str().unwrap_or_default())
        .env("MP_EVENT_ID", ev["id"].to_string())
        .env("MP_EVENT_PROJECT_ID", ev["project_id"].as_str().unwrap_or_default())
        .env("MP_EVENT_ORG_ID", ev["org_id"].as_str().unwrap_or_default())
        .env("MP_EVENT_ACTOR", ev["actor"]["name"].as_str().unwrap_or_default())
        .env("MP_EVENT_ACTOR_EMAIL", ev["actor"]["email"].as_str().unwrap_or_default())
        .env("MP_EVENT_JSON", &json)
        .stdin(std::process::Stdio::piped());
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            eprintln!("Could not run the command: {e}");
            return;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(json.as_bytes()).await;
    }
    match child.wait().await {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("The command for {} (event {}) ended with {s}.", ev["kind"].as_str().unwrap_or("an event"), ev["id"]),
        Err(e) => eprintln!("The command failed: {e}"),
    }
}

// ---------------------------------------------------------------------------------------------
// mp mcp (the tools themselves are in mp_core::mcp, shared with the server's /api/mcp)

/// What `mp mcp` adds to the shared tools: this computer's synced folders.
fn local_tools() -> Vec<Value> {
    vec![json!({ "name": "local_folders", "description": "Project folders synced on this computer, their branch, and their unpublished new, edited and deleted files.",
      "inputSchema": { "type": "object", "properties": {} }, "annotations": { "readOnlyHint": true } })]
}

fn local_folders() -> Result<Value> {
    let mut out = vec![];
    for root in Registry::workspaces()? {
        let Ok(ws) = Workspace::open(&root) else { continue };
        let st = mp_client::scan::status(&ws, &[]).ok();
        let paths = |l: Option<&Vec<mp_client::scan::LocalFile>>| l.map(|v| v.iter().map(|f| f.path.clone()).collect::<Vec<_>>()).unwrap_or_default();
        out.push(json!({ "folder": ws.root.display().to_string().trim_start_matches(r"\?\").to_string(), "project": ws.config.project_name, "organization": ws.config.org_name, "branch": ws.config.branch_name,
            "server": ws.config.server, "new": paths(st.as_ref().map(|s| &s.new)), "edited": paths(st.as_ref().map(|s| &s.edited)), "deleted": paths(st.as_ref().map(|s| &s.deleted)) }));
    }
    Ok(json!(out))
}

/// The shared MCP tools call the server through this account's API client.
struct ApiBackend<'a>(&'a Api);

impl mcp::Backend for ApiBackend<'_> {
    async fn get(&self, path: &str) -> Result<Value> {
        self.0.get(path).await
    }
    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        self.0.post(path, body).await
    }
}

/// Serve the Model Context Protocol on stdin/stdout, one JSON-RPC message per line.
pub async fn mcp(account: Option<String>) -> Result<()> {
    let api = api_for_account(account.as_deref())?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut out = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let r = json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": format!("Not JSON: {e}") } });
                out.write_all(format!("{r}\n").as_bytes()).await?;
                out.flush().await?;
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // a notification: nothing to answer
        let method = msg["method"].as_str().unwrap_or_default();
        let result: std::result::Result<Value, (i64, String)> = match method {
            "initialize" => Ok(mcp::initialize(&msg, &api.server, env!("CARGO_PKG_VERSION"))),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": mcp::tools().into_iter().chain(local_tools()).collect::<Vec<_>>() })),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or_default();
                let args = msg["params"]["arguments"].clone();
                let r = if name == "local_folders" {
                    local_folders()
                } else {
                    match mcp::call_tool(&ApiBackend(&api), name, &args).await {
                        Ok(Some(v)) => Ok(v),
                        Ok(None) => Err(anyhow::anyhow!("There is no tool called {name}.")),
                        Err(e) => Err(e),
                    }
                };
                Ok(mcp::tool_result(r))
            }
            other => Err((-32601, format!("Unknown method {other}"))),
        };
        let r = mcp::reply(id, result);
        out.write_all(format!("{r}\n").as_bytes()).await?;
        out.flush().await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn patterns() {
        let p = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(super::matches(&[], "x.y"));
        assert!(super::matches(&p(&["change.*"]), "change.published"));
        assert!(!super::matches(&p(&["change.*"]), "claim.added"));
    }

    #[test]
    fn local_tools_have_schemas() {
        for t in super::local_tools() {
            assert!(t["name"].is_string() && t["description"].is_string() && t["inputSchema"]["type"] == "object", "{t}");
        }
    }
}
