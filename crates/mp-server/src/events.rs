//! The event log. Every meaningful action is written to the `events` table (the audit log) and
//! broadcast to live subscribers (the SSE stream, later webhooks and scripts).

use serde_json::Value;
use sqlx::{PgExecutor, PgPool};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::models::Event;

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        EventBus { tx }
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
    pub fn send(&self, e: Event) {
        let _ = self.tx.send(e);
    }
}

/// Write an event inside an existing transaction or connection; returns its id. Call
/// [`publish_saved`] after the transaction commits to broadcast it.
pub async fn record<'e>(
    db: impl PgExecutor<'e>,
    org: Option<Uuid>,
    project: Option<Uuid>,
    actor: Option<Uuid>,
    kind: &str,
    data: Value,
) -> sqlx::Result<i64> {
    sqlx::query_scalar("INSERT INTO events (org_id, project_id, actor_id, kind, data) VALUES ($1,$2,$3,$4,$5) RETURNING id")
        .bind(org)
        .bind(project)
        .bind(actor)
        .bind(kind)
        .bind(data)
        .fetch_one(db)
        .await
}

/// Load a saved event with its actor and broadcast it.
pub async fn publish_saved(db: &PgPool, bus: &EventBus, id: i64) {
    match crate::models::load_events(db, "WHERE e.id = $1", id).await {
        Ok(mut list) if !list.is_empty() => bus.send(list.remove(0)),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "could not load event for broadcast"),
    }
}

/// Record and broadcast in one go, outside any transaction.
pub async fn emit(db: &PgPool, bus: &EventBus, org: Option<Uuid>, project: Option<Uuid>, actor: Option<Uuid>, kind: &str, data: Value) {
    match record(db, org, project, actor, kind, data).await {
        Ok(id) => publish_saved(db, bus, id).await,
        Err(e) => tracing::warn!(error = %e, kind, "could not record event"),
    }
}
