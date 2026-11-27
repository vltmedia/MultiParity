//! MultiParity server library. `main.rs` reads configuration and calls [`serve`]; integration
//! tests call [`build_app`] directly.

pub mod access;
pub mod auth;
pub mod backup;
pub mod branches;
pub mod chunks;
pub mod email;
pub mod error;
pub mod events;
pub mod limits;
pub mod models;
pub mod routes;
pub mod store;
pub mod thumbs;
pub mod util;
pub mod webhooks;

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
use crate::store::{Locations, StorageConfig};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Every storage location; each project uses one (`chunks::store`).
    pub stores: Arc<Locations>,
    pub events: EventBus,
    pub config: Arc<Config>,
    pub sign_in: Arc<limits::SignInGuard>,
    pub backup_running: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub listen: String,
    /// Public URL used in invite links, e.g. `https://mp.studio.example`.
    pub public_url: String,
    pub web_dir: Option<String>,
    /// Named storage locations: `main` (from `MP_STORAGE`) first, then `MP_STORAGE_LOCATIONS`.
    pub storage: Vec<(String, StorageConfig)>,
    /// Where new projects keep their data unless told otherwise (`MP_DEFAULT_STORAGE`, `main`).
    pub default_storage: String,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub default_org: String,
    /// Header a trusted proxy puts the real client address in (`MP_CLIENT_IP_HEADER`), for
    /// sign-in protection behind Caddy or a Cloudflare tunnel.
    pub client_ip_header: Option<String>,
    /// Nightly backups (`MP_BACKUP_TARGET`); none when unset.
    pub backup: Option<backup::BackupConfig>,
    /// Sending email (SMTP or SendGrid); none when not set up.
    pub email: Option<email::Email>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = std::env::var("MP_LISTEN").unwrap_or_else(|_| "0.0.0.0:18500".into());
        Ok(Config {
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            public_url: std::env::var("MP_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:18500".into()),
            web_dir: std::env::var("MP_WEB_DIR").ok().filter(|d| std::path::Path::new(d).join("index.html").exists()),
            storage: store::locations_from_env(StorageConfig::from_env()?)?,
            default_storage: std::env::var("MP_DEFAULT_STORAGE").ok().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).unwrap_or_else(|| store::MAIN.to_string()),
            admin_email: std::env::var("MP_ADMIN_EMAIL").ok().filter(|s| !s.is_empty()),
            admin_password: std::env::var("MP_ADMIN_PASSWORD").ok().filter(|s| !s.is_empty()),
            default_org: std::env::var("MP_DEFAULT_ORG").unwrap_or_else(|_| "My Studio".into()),
            backup: backup::BackupConfig::from_env()?,
            email: email::Email::from_env()?,
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
    anyhow::ensure!(
        config.storage.iter().any(|(n, _)| *n == config.default_storage),
        "MP_DEFAULT_STORAGE is {}, but there is no storage location by that name (check MP_STORAGE_LOCATIONS)",
        config.default_storage
    );
    let stores = Arc::new(Locations::open(&config.storage).await?);
    let state = AppState { db, stores, events: EventBus::new(), config: Arc::new(config), sign_in: Arc::new(limits::SignInGuard::default()), backup_running: Arc::new(std::sync::atomic::AtomicBool::new(false)) };
    auth::bootstrap(&state).await?;
    routes::storage::resume_after_restart(&state).await?;

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
    let storage: Vec<String> = state.stores.iter().map(|(n, s)| format!("{n}: {}", s.description)).collect();
    tracing::info!(storage = %storage.join("; "), web = ?state.config.web_dir, "MultiParity server listening on {listen}");
    if let Some(b) = state.config.backup.clone() {
        tracing::info!(target = %b.target.describe(), at = %format!("{:02}:{:02} UTC", b.at.0, b.at.1), "nightly backups on");
        backup::spawn_scheduler(state.clone(), b);
    }
    thumbs::spawn_worker(state.clone());
    webhooks::spawn(state.clone());
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
