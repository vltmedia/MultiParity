//! `/api/mcp`: MultiParity for AI assistants over the Model Context Protocol (Streamable HTTP,
//! answering with plain JSON; no server-sent stream). Sign in with a personal access token in
//! `Authorization: Bearer …`. The tools (mp_core::mcp, shared with `mp mcp`) call this server's
//! own routes in-process with that token, so an assistant gets exactly its person's access.

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use mp_core::mcp;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::auth::CurrentUser;
use crate::AppState;

struct InProcess {
    state: AppState,
    auth: HeaderValue,
}

impl InProcess {
    async fn call(&self, method: Method, path: &str, body: Option<&Value>) -> anyhow::Result<Value> {
        let req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, self.auth.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))?;
        let resp = super::router().with_state(self.state.clone()).oneshot(req).await?;
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 32 * 1024 * 1024).await?;
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if !status.is_success() {
            anyhow::bail!("{}", v["error"]["message"].as_str().unwrap_or(status.canonical_reason().unwrap_or("The server refused that.")));
        }
        Ok(v)
    }
}

impl mcp::Backend for InProcess {
    async fn get(&self, path: &str) -> anyhow::Result<Value> {
        self.call(Method::GET, path, None).await
    }
    async fn post(&self, path: &str, body: &Value) -> anyhow::Result<Value> {
        self.call(Method::POST, path, Some(body)).await
    }
}

async fn answer(b: &InProcess, server: &str, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned()?; // a notification: nothing to answer
    let method = msg["method"].as_str().unwrap_or_default();
    let result = match method {
        "initialize" => Ok(mcp::initialize(msg, server, env!("CARGO_PKG_VERSION"))),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": mcp::tools() })),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or_default();
            let r = match mcp::call_tool(b, name, &msg["params"]["arguments"]).await {
                Ok(Some(v)) => Ok(v),
                Ok(None) => Err(anyhow::anyhow!("There is no tool called {name}.")),
                Err(e) => Err(e),
            };
            Ok(mcp::tool_result(r))
        }
        other => Err((-32601, format!("Unknown method {other}"))),
    };
    Some(mcp::reply(id, result))
}

/// One JSON-RPC message, or a batch, per POST.
pub async fn post(State(s): State<AppState>, _user: CurrentUser, headers: HeaderMap, body: Bytes) -> Response {
    // Signed in by the extractor; the same token is used for the in-process calls.
    let Some(auth) = headers.get(header::AUTHORIZATION).cloned() else {
        return (StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, "Bearer realm=\"MultiParity\"")], Json(json!({
            "error": { "code": "unauthorized", "message": "Send a personal access token in an Authorization: Bearer header." }
        }))).into_response();
    };
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(mcp::reply(Value::Null, Err((-32700, format!("Not JSON: {e}")))))).into_response(),
    };
    let b = InProcess { state: s.clone(), auth };
    let server = s.config.public_url.clone();
    let out = match &msg {
        Value::Array(list) => {
            let mut all = Vec::new();
            for m in list {
                if let Some(r) = answer(&b, &server, m).await {
                    all.push(r);
                }
            }
            (!all.is_empty()).then(|| Value::Array(all))
        }
        m => answer(&b, &server, m).await,
    };
    match out {
        Some(v) => Json(v).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// No server-sent stream: every answer comes back on its POST.
pub async fn get() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response()
}

/// Nothing to end: the endpoint keeps no sessions.
pub async fn delete() -> Response {
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}
