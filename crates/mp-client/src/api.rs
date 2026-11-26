use std::time::Duration;

use anyhow::{Context, Result};
use bytes::Bytes;
use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use crate::types::*;

/// An error returned by the server, with its code and the server's message.
#[derive(Debug)]
pub struct ServerError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
    pub details: Value,
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ServerError {}

#[derive(Clone)]
pub struct Api {
    pub server: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Api {
    pub fn new(server: &str, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .user_agent(concat!("multiparity/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Api { server: server.trim_end_matches('/').to_string(), token, http }
    }

    pub fn for_account(a: &crate::Account) -> Self {
        Self::new(&a.server, Some(a.token.clone()))
    }

    fn req(&self, method: Method, path: &str) -> RequestBuilder {
        let mut r = self.http.request(method, format!("{}/api{}", self.server, path));
        if let Some(t) = &self.token {
            r = r.bearer_auth(t);
        }
        r
    }

    async fn send<T: DeserializeOwned>(&self, r: RequestBuilder) -> Result<T> {
        let resp = r.send().await.with_context(|| format!("could not reach the server at {}", self.server))?;
        let status = resp.status();
        if status.is_success() {
            return resp.json::<T>().await.context("reading the server's response");
        }
        let body: Value = resp.json().await.unwrap_or_else(|_| json!({}));
        let err = &body["error"];
        Err(ServerError {
            status,
            code: err["code"].as_str().unwrap_or("error").to_string(),
            message: err["message"].as_str().map(str::to_string).unwrap_or_else(|| format!("The server answered {status}.")),
            details: err["details"].clone(),
        }
        .into())
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send(self.req(Method::GET, path)).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        self.send(self.req(Method::POST, path).json(body)).await
    }

    pub async fn patch<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        self.send(self.req(Method::PATCH, path).json(body)).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send(self.req(Method::DELETE, path)).await
    }

    pub async fn delete_with<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        self.send(self.req(Method::DELETE, path).json(body)).await
    }

    // Accounts

    pub async fn login(&self, email: &str, password: &str, device: &str) -> Result<(String, Me)> {
        let v: Value = self.post("/auth/login", &json!({ "email": email, "password": password, "device": device })).await?;
        let token = v["token"].as_str().context("no token in login response")?.to_string();
        Ok((token, serde_json::from_value(v["user"].clone())?))
    }

    pub async fn me(&self) -> Result<Me> {
        self.get("/me").await
    }

    // Projects and branches

    pub async fn projects(&self) -> Result<Vec<ProjectSummary>> {
        self.get("/projects").await
    }

    pub async fn project(&self, id: &str) -> Result<ProjectSummary> {
        self.get(&format!("/projects/{id}")).await
    }

    pub async fn branches(&self, project: &str) -> Result<Vec<Branch>> {
        self.get(&format!("/projects/{project}/branches")).await
    }

    pub async fn tree(&self, project: &str, branch: &str, since: i64) -> Result<TreeDelta> {
        self.get(&format!("/projects/{project}/branches/{branch}/tree?since={since}")).await
    }

    // Sync protocol

    pub async fn missing_chunks(&self, project: &str, hashes: &[String]) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for batch in hashes.chunks(20_000) {
            let v: Value = self.post(&format!("/projects/{project}/chunks/missing"), &json!({ "hashes": batch })).await?;
            out.extend(serde_json::from_value::<Vec<String>>(v["missing"].clone())?);
        }
        Ok(out)
    }

    pub async fn put_chunk(&self, project: &str, hash: &str, compressed: Vec<u8>) -> Result<()> {
        let mut tries = 0;
        loop {
            tries += 1;
            let r = self.req(Method::PUT, &format!("/projects/{project}/chunks/{hash}")).header("content-type", "application/zstd").body(compressed.clone());
            match self.send::<Value>(r).await {
                Ok(_) => return Ok(()),
                Err(e) if tries < 4 && e.downcast_ref::<ServerError>().map(|s| s.status.is_server_error()).unwrap_or(true) => {
                    tokio::time::sleep(Duration::from_millis(400 * tries)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn get_chunk(&self, project: &str, hash: &str) -> Result<Bytes> {
        let mut tries = 0;
        loop {
            tries += 1;
            let resp = self.req(Method::GET, &format!("/projects/{project}/chunks/{hash}")).send().await;
            match resp {
                Ok(r) if r.status().is_success() => return Ok(r.bytes().await?),
                Ok(r) if tries >= 4 || r.status().is_client_error() => anyhow::bail!("could not download part of a file ({}): the server answered {}", &hash[..12], r.status()),
                Err(e) if tries >= 4 => return Err(e).context("downloading part of a file"),
                _ => tokio::time::sleep(Duration::from_millis(400 * tries)).await,
            }
        }
    }

    pub async fn manifest(&self, project: &str, hash: &str) -> Result<mp_core::Manifest> {
        self.get(&format!("/projects/{project}/manifests/{hash}")).await
    }

    pub async fn publish(&self, project: &str, branch: &str, message: &str, entries: &[PublishEntry]) -> Result<Change> {
        let v: Value = self.post(&format!("/projects/{project}/branches/{branch}/publish"), &json!({ "message": message, "entries": entries })).await?;
        Ok(serde_json::from_value(v["change"].clone())?)
    }

    // Older versions

    pub async fn versions(&self, project: &str, branch: &str, path: &str) -> Result<Vec<FileVersion>> {
        let r = self.req(Method::GET, &format!("/projects/{project}/versions")).query(&[("path", path), ("branch", branch)]);
        let v: Value = self.send(r).await?;
        Ok(serde_json::from_value(v["versions"].clone())?)
    }

    /// Make an older version the latest on the server, as a new change.
    pub async fn restore_version(&self, project: &str, branch: &str, path: &str, change: i64, message: Option<&str>) -> Result<Change> {
        let v: Value = self.post(&format!("/projects/{project}/branches/{branch}/restore-version"), &json!({ "path": path, "change": change, "message": message })).await?;
        Ok(serde_json::from_value(v["change"].clone())?)
    }

    // Claims

    pub async fn claim(&self, project: &str, branch: &str, paths: &[String], device: &str) -> Result<(Vec<String>, Vec<Claim>)> {
        let v: Value = self.post(&format!("/projects/{project}/claims"), &json!({ "branch": branch, "paths": paths, "device": device })).await?;
        Ok((serde_json::from_value(v["claimed"].clone())?, serde_json::from_value(v["held"].clone())?))
    }

    pub async fn release(&self, project: &str, branch: &str, paths: &[String]) -> Result<Vec<String>> {
        let v: Value = self.post(&format!("/projects/{project}/claims/release"), &json!({ "branch": branch, "paths": paths })).await?;
        Ok(serde_json::from_value(v["released"].clone())?)
    }

    pub async fn claims(&self, project: &str) -> Result<Vec<Claim>> {
        self.get(&format!("/projects/{project}/claims")).await
    }
}

/// True when an error is the server saying a publish conflicts (stale or claimed files).
pub fn conflict_details(e: &anyhow::Error) -> Option<&Value> {
    e.downcast_ref::<ServerError>().filter(|s| s.status == StatusCode::CONFLICT).map(|s| &s.details)
}
