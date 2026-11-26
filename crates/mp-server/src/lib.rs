//! MultiParity server library. `main.rs` reads configuration and calls [`serve`]; integration
//! tests call [`build_app`] directly.

pub mod access;
pub mod auth;
pub mod branches;
pub mod error;
pub mod events;
pub mod limits;
pub mod models;
pub mod routes;
pub mod store;
pub mod util;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::DefaultBodyLimit;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::events::EventBus;
use crate::store::{BlobStore, StorageConfig};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub store: BlobStore,
    pub events: EventBus,
    pub config: Arc<Config>,
    pub sign_in: Arc<limits::SignInGuard>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub listen: String,
    /// Public URL used in invite links, e.g. `https://mp.studio.example`.
    pub public_url: String,
    pub web_dir: Option<String>,
    pub storage: StorageConfig,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub default_org: String,
    /// Header a trusted proxy puts the real client address in (`MP_CLIENT_IP_HEADER`), for
    /// sign-in protection behind Caddy or a Cloudflare tunnel.
    pub client_ip_header: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = std::env::var("MP_LISTEN").unwrap_or_else(|_| "0.0.0.0:18500".into());
        Ok(Config {
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            public_url: std::env::var("MP_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:18500".into()),
            web_dir: std::env::var("MP_WEB_DIR").ok().filter(|d| std::path::Path::new(d).join("index.html").exists()),
            storage: StorageConfig::from_env()?,
            admin_email: std::env::var("MP_ADMIN_EMAIL").ok().filter(|s| !s.is_empty()),
            admin_password: std::env::var("MP_ADMIN_PASSWORD").ok().filter(|s| !s.is_empty()),
            default_org: std::env::var("MP_DEFAULT_ORG").unwrap_or_else(|_| "My Studio".into()),
            client_ip_header: std::env::var("MP_CLIENT_IP_HEADER").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()),
            listen,
        })
    }
}

pub async fn connect(config: &Config) -> Result<PgPool> {
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&config.database_url)
        .await
        .context("connecting to Postgres")?;
    sqlx::migrate!("./migrations").run(&db).await.context("running database migrations")?;
    Ok(db)
}

pub async fn build_app(config: Config) -> Result<(Router, AppState)> {
    let db = connect(&config).await?;
    let store = BlobStore::open(&config.storage).await?;
    let state = AppState { db, store, events: EventBus::new(), config: Arc::new(config), sign_in: Arc::new(limits::SignInGuard::default()) };
    auth::bootstrap(&state).await?;

    let api = routes::router();
    let mut app = Router::new()
        .nest("/api", api)
        .layer(DefaultBodyLimit::max(64 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    if let Some(dir) = &state.config.web_dir {
        let index = std::path::Path::new(dir).join("index.html");
        app = app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)));
    }
    Ok((app, state))
}

pub async fn serve(config: Config) -> Result<()> {
    let listen = config.listen.clone();
    let (app, state) = build_app(config).await?;
    tracing::info!(storage = %state.store.description, web = ?state.config.web_dir, "MultiParity server listening on {listen}");
    let listener = tokio::net::TcpListener::bind(&listen).await.with_context(|| format!("binding {listen}"))?;
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
}
