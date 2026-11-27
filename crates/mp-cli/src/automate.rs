//! Automation from the command line:
//!
//! - `mp watch` prints events as they happen, or runs a command for each (event scripting).
//! - `mp mcp` serves MultiParity to AI assistants over the Model Context Protocol (stdio).

use std::time::Duration;

use anyhow::{Context, Result};
use mp_client::accounts::Accounts;
use mp_client::api::Api;
use mp_client::workspace::{Registry, Workspace};
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
// mp mcp

const PROTOCOL: &str = "2025-06-18";

fn tools() -> Value {
    let project = json!({ "type": "string", "description": "Project name or id, as list_projects shows it." });
    let branch = json!({ "type": "string", "description": "Branch name; the main branch when left out." });
    let paths = json!({ "type": "array", "items": { "type": "string" }, "description": "File paths inside the project, like Art/Hero/hero.blend." });
    json!([
        { "name": "list_projects", "description": "The MultiParity projects this account can open, with their organization, size and claims.",
          "inputSchema": { "type": "object", "properties": {} }, "annotations": { "readOnlyHint": true } },
        { "name": "list_files", "description": "Files on a branch of a project, with who has each claimed. Filter by folder or by part of the name.",
          "inputSchema": { "type": "object", "properties": { "project": project, "branch": branch,
            "folder": { "type": "string", "description": "Only files in this folder, like Art/Hero." },
            "search": { "type": "string", "description": "Only files whose path contains this." } }, "required": ["project"] },
          "annotations": { "readOnlyHint": true } },
        { "name": "recent_changes", "description": "The latest published changes on a branch: number, message, who and when.",
          "inputSchema": { "type": "object", "properties": { "project": project, "branch": branch,
            "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "How many, newest first (default 20)." } }, "required": ["project"] },
          "annotations": { "readOnlyHint": true } },
        { "name": "change_details", "description": "One change: its message, author, and the files it added, edited or deleted.",
          "inputSchema": { "type": "object", "properties": { "project": project, "number": { "type": "integer" } }, "required": ["project", "number"] },
          "annotations": { "readOnlyHint": true } },
        { "name": "file_history", "description": "Every published version of one file, newest first, with who published it and why.",
          "inputSchema": { "type": "object", "properties": { "project": project, "path": { "type": "string" }, "branch": branch }, "required": ["project", "path"] },
          "annotations": { "readOnlyHint": true } },
        { "name": "claims", "description": "Who has which files claimed in a project, and since when.",
          "inputSchema": { "type": "object", "properties": { "project": project }, "required": ["project"] },
          "annotations": { "readOnlyHint": true } },
        { "name": "local_folders", "description": "Project folders synced on this computer, their branch, and their unpublished new, edited and deleted files.",
          "inputSchema": { "type": "object", "properties": {} }, "annotations": { "readOnlyHint": true } },
        { "name": "claim_files", "description": "Claim files for this account so nobody else can publish them. Files someone else holds are reported, not taken.",
          "inputSchema": { "type": "object", "properties": { "project": project, "paths": paths, "branch": branch }, "required": ["project", "paths"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true } },
        { "name": "release_files", "description": "Release this account's claims on files.",
          "inputSchema": { "type": "object", "properties": { "project": project, "paths": paths, "branch": branch }, "required": ["project", "paths"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true } },
        { "name": "ask_to_release", "description": "Tell whoever holds a file that this account is waiting for it (they get a notification, and an email if the server sends email).",
          "inputSchema": { "type": "object", "properties": { "project": project, "path": { "type": "string" }, "branch": branch }, "required": ["project", "path"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false } },
    ])
}

async fn branch_id(api: &Api, project: &str, name: Option<&str>) -> Result<String> {
    let list = api.branches(project).await?;
    let b = match name {
        Some(n) => list.iter().find(|b| b.name.eq_ignore_ascii_case(n) || b.id == n).with_context(|| format!("There is no branch called {n}."))?,
        None => list.iter().find(|b| b.parent_id.is_none()).context("This project has no main branch.")?,
    };
    Ok(b.id.clone())
}

async fn call_tool(api: &Api, name: &str, a: &Value) -> Result<Value> {
    let s = |k: &str| a[k].as_str().map(str::to_string);
    let project = match name {
        "list_projects" | "local_folders" => String::new(),
        _ => project_id(api, &s("project").context("Say which project.")?).await?,
    };
    let branch = || async { branch_id(api, &project, a["branch"].as_str()).await };
    let paths: Vec<String> = a["paths"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    Ok(match name {
        "list_projects" => {
            let list: Value = api.get("/projects").await?;
            json!(list.as_array().into_iter().flatten().map(|p| json!({ "id": p["id"], "name": p["name"], "organization": p["org"]["name"], "size": p["size"], "claims": p["claims"], "last_change": p["last_change"] })).collect::<Vec<_>>())
        }
        "list_files" => {
            let b = branch().await?;
            let mut q = vec![];
            if let Some(f) = s("folder") {
                q.push(format!("prefix={}/", urlenc(f.trim_matches('/'))));
            }
            if let Some(x) = s("search") {
                q.push(format!("q={}", urlenc(&x)));
            }
            let files: Value = api.get(&format!("/projects/{project}/branches/{b}/files?{}", q.join("&"))).await?;
            let all = files.as_array().cloned().unwrap_or_default();
            let shown: Vec<Value> = all.iter().take(300).map(|f| json!({ "path": f["path"], "size": f["size"], "change": f["change"], "kind": f["kind"],
                "claimed_by": f["claim"]["user"]["name"], "tags": f["tags"].as_array().map(|t| t.iter().map(|x| x["name"].clone()).collect::<Vec<_>>()) })).collect();
            json!({ "files": shown, "total": all.len(), "note": if all.len() > 300 { "Only the first 300 are listed; narrow with folder or search." } else { "" } })
        }
        "recent_changes" => {
            let b = branch().await?;
            let n = a["limit"].as_i64().unwrap_or(20).clamp(1, 100);
            api.get(&format!("/projects/{project}/changes?branch={b}&limit={n}")).await?
        }
        "change_details" => api.get(&format!("/projects/{project}/changes/{}", a["number"].as_i64().context("Say which change number.")?)).await?,
        "file_history" => {
            let b = branch().await?;
            let path = s("path").context("Say which file.")?;
            api.get(&format!("/projects/{project}/versions?branch={b}&path={}", urlenc(&path))).await?
        }
        "claims" => api.get(&format!("/projects/{project}/claims")).await?,
        "claim_files" => {
            let b = branch().await?;
            api.post(&format!("/projects/{project}/claims"), &json!({ "branch": b, "paths": paths, "device": "AI assistant" })).await?
        }
        "release_files" => {
            let b = branch().await?;
            api.post(&format!("/projects/{project}/claims/release"), &json!({ "branch": b, "paths": paths })).await?
        }
        "ask_to_release" => {
            let b = branch().await?;
            api.post(&format!("/projects/{project}/claims/nudge"), &json!({ "branch": b, "path": s("path").context("Say which file.")? })).await?
        }
        "local_folders" => {
            let mut out = vec![];
            for root in Registry::workspaces()? {
                let Ok(ws) = Workspace::open(&root) else { continue };
                let st = mp_client::scan::status(&ws, &[]).ok();
                let paths = |l: Option<&Vec<mp_client::scan::LocalFile>>| l.map(|v| v.iter().map(|f| f.path.clone()).collect::<Vec<_>>()).unwrap_or_default();
                out.push(json!({ "folder": ws.root.display().to_string(), "project": ws.config.project_name, "organization": ws.config.org_name, "branch": ws.config.branch_name,
                    "server": ws.config.server, "new": paths(st.as_ref().map(|s| &s.new)), "edited": paths(st.as_ref().map(|s| &s.edited)), "deleted": paths(st.as_ref().map(|s| &s.deleted)) }));
            }
            json!(out)
        }
        other => anyhow::bail!("There is no tool called {other}."),
    })
}

fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
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
            "initialize" => Ok(json!({
                "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "multiparity", "title": "MultiParity", "version": env!("CARGO_PKG_VERSION") },
                "instructions": format!("MultiParity version control on {}. Projects hold big art and game files; people claim a file before editing it so two people never change the same file. Look up projects with list_projects first; tools take the project's name.", api.server),
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or_default();
                let args = msg["params"]["arguments"].clone();
                Ok(match call_tool(&api, name, &args).await {
                    Ok(v) => json!({ "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }], "structuredContent": if v.is_object() { v.clone() } else { json!({ "items": v }) }, "isError": false }),
                    Err(e) => json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true }),
                })
            }
            other => Err((-32601, format!("Unknown method {other}"))),
        };
        let r = match result {
            Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
            Err((code, m)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": m } }),
        };
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
    fn tools_have_schemas() {
        for t in super::tools().as_array().unwrap() {
            assert!(t["name"].is_string() && t["description"].is_string() && t["inputSchema"]["type"] == "object", "{t}");
        }
    }
}
