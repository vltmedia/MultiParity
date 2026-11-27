//! Webhooks: every event an organization's hooks ask for is posted to their URL as JSON:
//!
//!   POST <url>
//!   Content-Type: application/json
//!   X-MultiParity-Event: change.published
//!   X-MultiParity-Delivery: <event id>
//!   X-MultiParity-Signature: sha256=<hex HMAC-SHA256 of the body with the hook's secret>
//!
//!   {"id": 123, "kind": "change.published", "org_id": ..., "project_id": ..., "actor": {...},
//!    "data": {...}, "created_at": "...", "server": "https://mp.studio.example"}
//!
//! A failed delivery is tried again after 10 s and 60 s. After 20 failures in a row a hook is
//! paused (`active = false`) until an admin turns it back on.

use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::Row;
use uuid::Uuid;

use crate::models::Event;
use crate::AppState;

const PAUSE_AFTER: i32 = 20;

/// `change.published` matches `change.published`, `change.*` and `*`; no patterns match all.
pub fn matches(patterns: &[String], kind: &str) -> bool {
    patterns.is_empty()
        || patterns.iter().any(|p| p == "*" || p == kind || p.strip_suffix(".*").map(|pre| kind.starts_with(&format!("{pre}."))).unwrap_or(false))
}

pub fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("any key length");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Deliver events to webhooks for as long as the server runs.
pub fn spawn(s: AppState) {
    let mut rx = s.events.subscribe();
    let http = reqwest::Client::builder().timeout(Duration::from_secs(15)).user_agent(concat!("MultiParity-Webhooks/", env!("CARGO_PKG_VERSION"))).build().expect("http client");
    tokio::spawn(async move {
        loop {
            let e = match rx.recv().await {
                Ok(e) => e,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "webhooks fell behind and skipped events");
                    continue;
                }
                Err(_) => return,
            };
            let Some(org) = e.org_id else { continue };
            let hooks = match sqlx::query("SELECT id, url, events, project_id, secret FROM webhooks WHERE org_id = $1 AND active").bind(org).fetch_all(&s.db).await {
                Ok(h) => h,
                Err(err) => {
                    tracing::warn!(error = %err, "loading webhooks");
                    continue;
                }
            };
            for h in hooks {
                let events: Vec<String> = h.get("events");
                let project: Option<Uuid> = h.get("project_id");
                if !matches(&events, &e.kind) || (project.is_some() && project != e.project_id) {
                    continue;
                }
                tokio::spawn(deliver(s.clone(), http.clone(), h.get("id"), h.get("url"), h.get("secret"), e.clone()));
            }
        }
    });
}

async fn deliver(s: AppState, http: reqwest::Client, id: Uuid, url: String, secret: String, e: Event) {
    let mut v = serde_json::to_value(&e).unwrap_or_default();
    v["server"] = serde_json::json!(s.config.public_url);
    let body = serde_json::to_vec(&v).unwrap_or_default();
    let sig = sign(&secret, &body);
    let mut last: (Option<u16>, Option<String>) = (None, None);
    for (attempt, wait) in [0u64, 10, 60].into_iter().enumerate() {
        if wait > 0 {
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
        let r = http
            .post(&url)
            .header("content-type", "application/json")
            .header("x-multiparity-event", &e.kind)
            .header("x-multiparity-delivery", e.id.to_string())
            .header("x-multiparity-signature", &sig)
            .body(body.clone())
            .send()
            .await;
        last = match r {
            Ok(resp) if resp.status().is_success() => (Some(resp.status().as_u16()), None),
            Ok(resp) => (Some(resp.status().as_u16()), Some(format!("answered {}", resp.status()))),
            Err(err) => (None, Some(format!("{err}"))),
        };
        if last.1.is_none() {
            break;
        }
        tracing::info!(hook = %id, attempt = attempt + 1, error = ?last.1, "webhook delivery failed");
    }
    let ok = last.1.is_none();
    let _ = sqlx::query(
        "UPDATE webhooks SET last_at = now(), last_status = $2, last_error = $3,
           failures = CASE WHEN $4 THEN 0 ELSE failures + 1 END,
           active = CASE WHEN $4 THEN active ELSE failures + 1 < $5 END
         WHERE id = $1",
    )
    .bind(id)
    .bind(last.0.map(|x| x as i32))
    .bind(&last.1)
    .bind(ok)
    .bind(PAUSE_AFTER)
    .execute(&s.db)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        let p = |x: &[&str]| x.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(matches(&[], "change.published"));
        assert!(matches(&p(&["change.*"]), "change.merged"));
        assert!(!matches(&p(&["change.*"]), "claim.added"));
        assert!(!matches(&p(&["change.*"]), "changes.x"));
        assert!(matches(&p(&["claim.nudged", "draft.saved"]), "draft.saved"));
        assert!(matches(&p(&["*"]), "anything"));
    }

    #[test]
    fn signature_is_hmac_sha256() {
        // RFC 4231 test case 2.
        assert_eq!(sign("Jefe", b"what do ya want for nothing?"), "sha256=5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }
}
