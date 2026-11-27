use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use futures::stream::Stream;
use futures::StreamExt;
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::access::{self, Level};
use crate::auth::CurrentUser;
use crate::error::ApiResult;
use crate::AppState;

#[derive(Deserialize)]
pub struct EventsQuery {
    project: Option<Uuid>,
}

/// Live events. With `project`, only that project's events; otherwise events from every
/// organization the caller belongs to.
pub async fn stream(State(s): State<AppState>, user: CurrentUser, Query(q): Query<EventsQuery>) -> ApiResult<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>> {
    let orgs: Option<Vec<Uuid>> = match q.project {
        Some(p) => {
            access::project(&s, &user, p, Level::Read).await?;
            None
        }
        None if user.is_server_admin => None,
        None => Some(sqlx::query_scalar("SELECT org_id FROM memberships WHERE user_id = $1").bind(user.id).fetch_all(&s.db).await?),
    };
    // A project's events only go to people who can see it (their policies, as they are when they
    // connect; the app reconnects often enough for changes to reach it).
    let projects: Option<std::collections::HashSet<Uuid>> = match (&orgs, q.project) {
        (Some(_), None) => Some(
            sqlx::query_scalar("SELECT p.id FROM projects p WHERE mp_access($1, p.id) > 0").bind(user.id).fetch_all(&s.db).await?.into_iter().collect(),
        ),
        _ => None,
    };
    let project = q.project;
    let me = user.id;
    let rx = BroadcastStream::new(s.events.subscribe());
    let stream = rx.filter_map(move |e| {
        let orgs = orgs.clone();
        let projects = projects.clone();
        async move {
            let e = e.ok()?;
            let visible = match (project, &orgs) {
                (Some(p), _) => e.project_id == Some(p),
                (None, Some(_)) if e.project_id.is_some() => projects.as_ref().map(|set| set.contains(&e.project_id.unwrap())).unwrap_or(false),
                (None, Some(list)) => e.org_id.map(|o| list.contains(&o)).unwrap_or(false) || e.data.get("user_id").and_then(|v| v.as_str()) == Some(&me.to_string()),
                (None, None) => true,
            };
            if !visible {
                return None;
            }
            let data = serde_json::to_string(&e).ok()?;
            Some(Ok(SseEvent::default().event(e.kind.clone()).id(e.id.to_string()).data(data)))
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20))))
}
