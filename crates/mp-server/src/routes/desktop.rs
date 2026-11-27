//! The desktop app's updates and downloads, served by the studio's own server, so a studio
//! decides which version its people run.
//!
//! `MP_DESKTOP_DIR` (default `/data/desktop`) holds `latest.json` in the Tauri updater format
//! (`{version, notes, pub_date, platforms: {"windows-x86_64": {url, signature}, ...}}`, plus an
//! optional `downloads: {"windows-x86_64": "MultiParity_0.2.0_x64-setup.exe", ...}` for first
//! installs) and the files it names. `scripts/release/desktop-latest.mjs` writes it from a build.
//! A `url` that is just a file name is served from that folder.

use std::path::PathBuf;

use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

pub fn desktop_dir() -> PathBuf {
    std::env::var("MP_DESKTOP_DIR").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/data/desktop"))
}

fn latest() -> Option<Value> {
    let bytes = std::fs::read(desktop_dir().join("latest.json")).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// `1.2.3` (a leading `v` and anything after `-` or `+` ignored) as comparable numbers.
fn version_key(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches('v').split(['-', '+']).next().unwrap_or("").split('.').map(|x| x.parse().unwrap_or(0)).collect()
}

fn newer(candidate: &str, current: &str) -> bool {
    let (mut a, mut b) = (version_key(candidate), version_key(current));
    let n = a.len().max(b.len()).max(3);
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

fn file_url(s: &AppState, url: &str) -> String {
    if url.contains("://") {
        url.to_string()
    } else {
        format!("{}/api/desktop/files/{}", s.config.public_url.trim_end_matches('/'), url)
    }
}

/// What is published: the version and a download per platform. Public, like the sign-in page.
pub async fn info(State(s): State<AppState>) -> Json<Value> {
    let Some(l) = latest() else { return Json(json!({ "version": null, "downloads": [] })) };
    let mut downloads = Vec::new();
    if let Some(map) = l["downloads"].as_object() {
        for (platform, name) in map {
            if let Some(name) = name.as_str() {
                let size = std::fs::metadata(desktop_dir().join(name)).map(|m| m.len()).ok();
                downloads.push(json!({ "platform": platform, "name": name, "url": file_url(&s, name), "size": size }));
            }
        }
    }
    Json(json!({ "version": l["version"], "notes": l["notes"], "pub_date": l["pub_date"], "downloads": downloads,
                 "platforms": l["platforms"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() }))
}

/// The Tauri updater's question: is there something newer than `current` for this platform?
/// 204 when not.
pub async fn update(State(s): State<AppState>, Path((target, arch, current)): Path<(String, String, String)>) -> Response {
    let Some(l) = latest() else { return StatusCode::NO_CONTENT.into_response() };
    let version = l["version"].as_str().unwrap_or_default();
    if version.is_empty() || !newer(version, &current) {
        return StatusCode::NO_CONTENT.into_response();
    }
    let p = &l["platforms"][format!("{target}-{arch}")];
    let (Some(url), Some(signature)) = (p["url"].as_str(), p["signature"].as_str()) else {
        return StatusCode::NO_CONTENT.into_response();
    };
    Json(json!({ "version": version, "notes": l["notes"], "pub_date": l["pub_date"], "url": file_url(&s, url), "signature": signature })).into_response()
}

/// One file from the desktop folder (installers and update bundles).
pub async fn file(Path(name): Path<String>, req: Request) -> ApiResult<Response> {
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) || name == "latest.json" {
        return Err(ApiError::not_found("That file"));
    }
    let path = desktop_dir().join(&name);
    if !path.is_file() {
        return Err(ApiError::not_found("That file"));
    }
    let mut resp = match ServeFile::new(path).oneshot(req).await {
        Ok(r) => r.into_response(),
        Err(never) => match never {},
    };
    if let Ok(v) = format!("attachment; filename=\"{}\"", name.replace('"', "")).parse() {
        resp.headers_mut().insert(axum::http::header::CONTENT_DISPOSITION, v);
    }
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.9"));
        assert!(newer("1.0.0", "0.99.99"));
        assert!(newer("v0.1.1", "0.1.0"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0-beta", "0.1.0"));
        assert!(!newer("0.1", "0.1.0"));
        assert!(newer("0.1.0.1", "0.1.0"));
    }
}
