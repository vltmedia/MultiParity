//! MultiParity over the Model Context Protocol, shared by `mp mcp` (stdio, on someone's computer)
//! and the server's `/api/mcp` (HTTP, with an access token). Tools are written against
//! [`Backend`]: "GET or POST this API path as the signed-in person", so they get exactly that
//! person's access, policies included, wherever they run.

use std::future::Future;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// The newest protocol version we speak; we answer with the client's when it asks for an older one.
pub const PROTOCOL: &str = "2025-06-18";
const KNOWN: [&str; 4] = ["2025-06-18", "2025-03-26", "2024-11-05", "2024-10-07"];

/// Calls to the MultiParity HTTP API as one person. Paths are relative to `/api`.
pub trait Backend: Sync {
    fn get(&self, path: &str) -> impl Future<Output = Result<Value>> + Send;
    fn post(&self, path: &str, body: &Value) -> impl Future<Output = Result<Value>> + Send;
}

pub fn urlenc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

pub fn instructions(server: &str) -> String {
    format!(
        "MultiParity version control on {server}. Projects hold big art and game files; people claim a file before editing it so two \
         people never change the same file. An organization files projects under clients and productions (both optional). Look up \
         projects with list_projects first; tools take a project's name. Admin tools (who_can_see, list_policies, \
         organization_structure, exports) work only for organization admins."
    )
}

/// The answer to `initialize`.
pub fn initialize(msg: &Value, server: &str, version: &str) -> Value {
    let asked = msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL);
    json!({
        "protocolVersion": if KNOWN.contains(&asked) { asked } else { PROTOCOL },
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "multiparity", "title": "MultiParity", "version": version },
        "instructions": instructions(server),
    })
}

/// A tool's outcome as a `tools/call` result (errors are results too, so the model sees them).
pub fn tool_result(r: Result<Value>) -> Value {
    match r {
        Ok(v) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() }],
            "structuredContent": if v.is_object() { v.clone() } else { json!({ "items": v }) },
            "isError": false,
        }),
        Err(e) => json!({ "content": [{ "type": "text", "text": format!("{e:#}") }], "isError": true }),
    }
}

pub fn reply(id: Value, r: std::result::Result<Value, (i64, String)>) -> Value {
    match r {
        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
        Err((code, m)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": m } }),
    }
}

/// The tools every MultiParity MCP server offers.
pub fn tools() -> Vec<Value> {
    let project = json!({ "type": "string", "description": "Project name or id, as list_projects shows it." });
    let branch = json!({ "type": "string", "description": "Branch name; the main branch when left out." });
    let org = json!({ "type": "string", "description": "Organization name; can be left out when the account is in only one." });
    let paths = json!({ "type": "array", "items": { "type": "string" }, "description": "File paths inside the project, like Art/Hero/hero.blend." });
    let ro = json!({ "readOnlyHint": true });
    vec![
        json!({ "name": "list_projects", "description": "The projects this account can open: organization, client, production (and whether it is the production's main project), size, claims and last change. Filter by client or production.",
          "inputSchema": { "type": "object", "properties": {
            "client": { "type": "string", "description": "Only projects of this client." },
            "production": { "type": "string", "description": "Only projects of this production (e.g. a game or show)." } } },
          "annotations": ro }),
        json!({ "name": "list_files", "description": "Files on a branch of a project, with who has each claimed. Filter by folder or by part of the name.",
          "inputSchema": { "type": "object", "properties": { "project": project, "branch": branch,
            "folder": { "type": "string", "description": "Only files in this folder, like Art/Hero." },
            "search": { "type": "string", "description": "Only files whose path contains this." } }, "required": ["project"] },
          "annotations": ro }),
        json!({ "name": "recent_changes", "description": "The latest published changes on a branch: number, message, who and when.",
          "inputSchema": { "type": "object", "properties": { "project": project, "branch": branch,
            "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "How many, newest first (default 20)." } }, "required": ["project"] },
          "annotations": ro }),
        json!({ "name": "change_details", "description": "One change: its message, author, and the files it added, edited or deleted.",
          "inputSchema": { "type": "object", "properties": { "project": project, "number": { "type": "integer" } }, "required": ["project", "number"] },
          "annotations": ro }),
        json!({ "name": "file_history", "description": "Every published version of one file, newest first, with who published it and why.",
          "inputSchema": { "type": "object", "properties": { "project": project, "path": { "type": "string" }, "branch": branch }, "required": ["project", "path"] },
          "annotations": ro }),
        json!({ "name": "claims", "description": "Who has which files claimed in a project, and since when.",
          "inputSchema": { "type": "object", "properties": { "project": project }, "required": ["project"] }, "annotations": ro }),
        json!({ "name": "claim_files", "description": "Claim files for this account so nobody else can publish them. Files someone else holds are reported, not taken.",
          "inputSchema": { "type": "object", "properties": { "project": project, "paths": paths, "branch": branch }, "required": ["project", "paths"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true } }),
        json!({ "name": "release_files", "description": "Release this account's claims on files.",
          "inputSchema": { "type": "object", "properties": { "project": project, "paths": paths, "branch": branch }, "required": ["project", "paths"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true } }),
        json!({ "name": "ask_to_release", "description": "Tell whoever holds a file that this account is waiting for it (they get a notification, and an email if the server sends email).",
          "inputSchema": { "type": "object", "properties": { "project": project, "path": { "type": "string" }, "branch": branch }, "required": ["project", "path"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false } }),
        json!({ "name": "organization_structure", "description": "Admins: an organization's clients and productions, with how many projects each has and each production's main project.",
          "inputSchema": { "type": "object", "properties": { "organization": org } }, "annotations": ro }),
        json!({ "name": "who_can_see", "description": "Admins: everyone in the project's organization, whether they can change, look at or not see the project, and which role or policy decides it.",
          "inputSchema": { "type": "object", "properties": { "project": project }, "required": ["project"] }, "annotations": ro }),
        json!({ "name": "list_policies", "description": "Admins: an organization's access policies, their rules (no access, look or change, on the organization, a client, a production or a project) and who has each.",
          "inputSchema": { "type": "object", "properties": { "organization": org } }, "annotations": ro }),
        json!({ "name": "list_exports", "description": "Admins: a project's exports (to a .zip or plain files, on the server or a NAS or bucket) and how far each got.",
          "inputSchema": { "type": "object", "properties": { "project": project }, "required": ["project"] }, "annotations": ro }),
        json!({ "name": "start_export", "description": "Admins: export one branch of a project, as it is now, as one .zip or plain files, to the server or a place set up for exports. Runs in the background; check it with list_exports. Does not archive the project.",
          "inputSchema": { "type": "object", "properties": { "project": project, "branch": branch,
            "target": { "type": "string", "description": "Where to: 'server' (default) or a name the server's admin set up." },
            "format": { "type": "string", "enum": ["zip", "files"], "description": "One .zip (default) or plain files." } }, "required": ["project"] },
          "annotations": { "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false } }),
    ]
}

fn s<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a[k].as_str().map(str::trim).filter(|x| !x.is_empty())
}

/// A project by name (any case), slug or id.
pub async fn project_id<B: Backend>(b: &B, key: &str) -> Result<String> {
    let list = b.get("/projects").await?;
    let k = key.trim().to_lowercase();
    list.as_array()
        .into_iter()
        .flatten()
        .find(|p| p["id"] == key || p["name"].as_str().map(str::to_lowercase) == Some(k.clone()) || p["slug"].as_str().map(str::to_lowercase) == Some(k.clone()))
        .and_then(|p| p["id"].as_str().map(str::to_string))
        .with_context(|| format!("There is no project called {key} that this account can see. list_projects shows them."))
}

async fn branch_id<B: Backend>(b: &B, project: &str, name: Option<&str>) -> Result<String> {
    let list = b.get(&format!("/projects/{project}/branches")).await?;
    let all = list.as_array().cloned().unwrap_or_default();
    let found = match name {
        Some(n) => all.iter().find(|x| x["name"].as_str().map(|v| v.eq_ignore_ascii_case(n)).unwrap_or(false) || x["id"] == n),
        None => all.iter().find(|x| x["parent_id"].is_null()),
    };
    found.and_then(|x| x["id"].as_str().map(str::to_string)).with_context(|| match name {
        Some(n) => format!("There is no branch called {n}."),
        None => "This project has no main branch.".into(),
    })
}

async fn org_id<B: Backend>(b: &B, name: Option<&str>) -> Result<String> {
    let list = b.get("/orgs").await?;
    let all = list.as_array().cloned().unwrap_or_default();
    let pick = match name {
        Some(n) => all.iter().find(|o| o["id"] == n || o["name"].as_str().map(|v| v.eq_ignore_ascii_case(n)).unwrap_or(false) || o["slug"] == n),
        None if all.len() == 1 => all.first(),
        None => {
            let names: Vec<&str> = all.iter().filter_map(|o| o["name"].as_str()).collect();
            bail!("Say which organization: {}.", names.join(", "))
        }
    };
    pick.and_then(|o| o["id"].as_str().map(str::to_string)).with_context(|| format!("There is no organization called {}.", name.unwrap_or("")))
}

/// Run a tool. `Ok(None)` when there is no such tool here (a caller may offer more of its own).
pub async fn call_tool<B: Backend>(b: &B, name: &str, a: &Value) -> Result<Option<Value>> {
    let needs_project = !matches!(name, "list_projects" | "organization_structure" | "list_policies");
    if !tools().iter().any(|t| t["name"] == name) {
        return Ok(None);
    }
    let project = if needs_project { project_id(b, s(a, "project").context("Say which project.")?).await? } else { String::new() };
    let paths: Vec<String> = a["paths"].as_array().map(|x| x.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let br = || branch_id(b, &project, s(a, "branch"));
    Ok(Some(match name {
        "list_projects" => {
            let list = b.get("/projects").await?;
            let want = |field: &str, p: &Value| match s(a, field) {
                None => true,
                Some(w) => p[field]["name"].as_str().map(|n| n.eq_ignore_ascii_case(w)).unwrap_or(false),
            };
            json!(list.as_array().into_iter().flatten().filter(|p| want("client", p) && want("production", p)).map(|p| json!({
                "id": p["id"], "name": p["name"], "organization": p["org"]["name"],
                "client": p["client"]["name"], "production": p["production"]["name"], "main_project": p["is_main"],
                "size": p["size"], "files": p["file_count"], "claims": p["claims"], "last_change": p["last_change"],
            })).collect::<Vec<_>>())
        }
        "list_files" => {
            let br = br().await?;
            let mut q = vec![];
            if let Some(f) = s(a, "folder") {
                q.push(format!("prefix={}/", urlenc(f.trim_matches('/'))));
            }
            if let Some(x) = s(a, "search") {
                q.push(format!("q={}", urlenc(x)));
            }
            let files = b.get(&format!("/projects/{project}/branches/{br}/files?{}", q.join("&"))).await?;
            let all = files.as_array().cloned().unwrap_or_default();
            let shown: Vec<Value> = all.iter().take(300).map(|f| json!({ "path": f["path"], "size": f["size"], "change": f["change"], "kind": f["kind"],
                "claimed_by": f["claim"]["user"]["name"], "tags": f["tags"].as_array().map(|t| t.iter().map(|x| x["name"].clone()).collect::<Vec<_>>()) })).collect();
            json!({ "files": shown, "total": all.len(), "note": if all.len() > 300 { "Only the first 300 are listed; narrow with folder or search." } else { "" } })
        }
        "recent_changes" => {
            let br = br().await?;
            let n = a["limit"].as_i64().unwrap_or(20).clamp(1, 100);
            b.get(&format!("/projects/{project}/changes?branch={br}&limit={n}")).await?
        }
        "change_details" => b.get(&format!("/projects/{project}/changes/{}", a["number"].as_i64().context("Say which change number.")?)).await?,
        "file_history" => {
            let br = br().await?;
            b.get(&format!("/projects/{project}/versions?branch={br}&path={}", urlenc(s(a, "path").context("Say which file.")?))).await?
        }
        "claims" => b.get(&format!("/projects/{project}/claims")).await?,
        "claim_files" => {
            let br = br().await?;
            b.post(&format!("/projects/{project}/claims"), &json!({ "branch": br, "paths": paths, "device": "AI assistant" })).await?
        }
        "release_files" => {
            let br = br().await?;
            b.post(&format!("/projects/{project}/claims/release"), &json!({ "branch": br, "paths": paths })).await?
        }
        "ask_to_release" => {
            let br = br().await?;
            b.post(&format!("/projects/{project}/claims/nudge"), &json!({ "branch": br, "path": s(a, "path").context("Say which file.")? })).await?
        }
        "organization_structure" => {
            let org = org_id(b, s(a, "organization")).await?;
            let st = b.get(&format!("/admin/orgs/{org}/structure")).await?;
            let projects = b.get(&format!("/admin/projects?org={org}")).await?;
            let name_of = |id: &Value| projects.as_array().into_iter().flatten().find(|p| p["id"] == *id).map(|p| p["name"].clone()).unwrap_or(Value::Null);
            let client_of = |id: &Value| st["clients"].as_array().into_iter().flatten().find(|c| c["id"] == *id).map(|c| c["name"].clone()).unwrap_or(Value::Null);
            json!({
                "clients": st["clients"].as_array().into_iter().flatten().map(|c| json!({ "name": c["name"], "productions": c["productions"], "projects": c["projects"] })).collect::<Vec<_>>(),
                "productions": st["productions"].as_array().into_iter().flatten().map(|p| json!({
                    "name": p["name"], "client": client_of(&p["client_id"]), "projects": p["projects"], "main_project": name_of(&p["main_project_id"]),
                })).collect::<Vec<_>>(),
            })
        }
        "who_can_see" => b.get(&format!("/admin/projects/{project}/who")).await?,
        "list_policies" => {
            let org = org_id(b, s(a, "organization")).await?;
            let list = b.get(&format!("/admin/orgs/{org}/policies")).await?;
            json!(list.as_array().into_iter().flatten().map(|p| json!({
                "name": p["name"], "description": p["description"], "every_member": p["is_default"],
                "rules": p["rules"].as_array().into_iter().flatten().map(|r| json!({ "level": r["level"], "on": r["scope_kind"], "name": r["scope_name"] })).collect::<Vec<_>>(),
                "people": p["people"].as_array().into_iter().flatten().map(|u| u["name"].clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>())
        }
        "list_exports" => b.get(&format!("/admin/projects/{project}/exports")).await?,
        "start_export" => {
            let branch = match s(a, "branch") {
                Some(_) => Some(br().await?),
                None => None,
            };
            let format = s(a, "format").unwrap_or("zip");
            if format != "zip" && format != "files" {
                bail!("format is zip or files.");
            }
            b.post(&format!("/admin/projects/{project}/exports"), &json!({ "target": s(a, "target").unwrap_or("server"), "format": format, "branch": branch, "archive_after": false })).await?
        }
        _ => return Ok(None),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn tools_have_schemas() {
        let all = super::tools();
        for t in &all {
            assert!(t["name"].is_string() && t["description"].is_string() && t["inputSchema"]["type"] == "object", "{t}");
        }
        let mut names: Vec<_> = all.iter().map(|t| t["name"].as_str().unwrap()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len(), "tool names are unique");
    }

    #[test]
    fn protocol_versions() {
        let m = serde_json::json!({ "params": { "protocolVersion": "2025-03-26" } });
        assert_eq!(super::initialize(&m, "x", "1")["protocolVersion"], "2025-03-26");
        let m = serde_json::json!({ "params": { "protocolVersion": "1999-01-01" } });
        assert_eq!(super::initialize(&m, "x", "1")["protocolVersion"], super::PROTOCOL);
    }
}
