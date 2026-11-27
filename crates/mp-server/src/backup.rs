//! Backups: the database and every storage location's file data, copied to a backup target
//! (a folder, a mounted drive or share, or S3-compatible storage) every night, kept for a week
//! of dailies and a month of weeklies.
//!
//! Layout on the target:
//! - `db/<snapshot>/<table>.csv.zst`: each table, from one consistent database snapshot.
//! - `db/<snapshot>/manifest.json`: written last; a snapshot without one is incomplete.
//! - `chunks/<xx>/<hash>`: file data, compressed as stored. Copied once and shared by every
//!   snapshot that needs it, so nightly backups only copy what is new.
//!
//! The database is written table by table with `COPY`, not `pg_dump`, so no Postgres client
//! tools are needed and a backup restores into a newer server (columns are matched by name).

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::Ordering;

use anyhow::{Context, Result};
use bytes::Bytes;
use chrono::{DateTime, Datelike, Utc};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, PgConnection, Row};

use crate::store::{parse_spec, BlobStore, StorageConfig};
use crate::AppState;

#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub target: StorageConfig,
    /// Daily start time, UTC.
    pub at: (u32, u32),
    pub keep_daily: usize,
    pub keep_weekly: usize,
}

impl BackupConfig {
    /// `MP_BACKUP_TARGET` (e.g. `mount:/data/backups`, `s3:bucket=...`), `MP_BACKUP_AT` (UTC,
    /// default 03:00), `MP_BACKUP_KEEP_DAILY` (7), `MP_BACKUP_KEEP_WEEKLY` (4). No target: no backups.
    pub fn from_env() -> Result<Option<Self>> {
        let Some(spec) = std::env::var("MP_BACKUP_TARGET").ok().filter(|s| !s.trim().is_empty()) else { return Ok(None) };
        let at = std::env::var("MP_BACKUP_AT").unwrap_or_else(|_| "03:00".into());
        let (h, m) = at.split_once(':').context("MP_BACKUP_AT should look like 03:00")?;
        let num = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        Ok(Some(BackupConfig {
            target: parse_spec("backup", spec.trim())?,
            at: (h.parse().context("MP_BACKUP_AT hour")?, m.parse().context("MP_BACKUP_AT minute")?),
            keep_daily: num("MP_BACKUP_KEEP_DAILY", 7).max(1),
            keep_weekly: num("MP_BACKUP_KEEP_WEEKLY", 4),
        }))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub server_version: String,
    pub migration: i64,
    pub tables: Vec<TableInfo>,
    pub chunks: u64,
    pub db_bytes: u64,
    pub new_chunks: u64,
    pub new_chunk_bytes: u64,
}

const PARALLEL: usize = 8;
/// Not backed up: migrations (the server version has them) and thumbnails (drawn again).
const SKIP_TABLES: &[&str] = &["_sqlx_migrations", "thumbnails"];

fn chunk_rel(hash: &str) -> String {
    format!("chunks/{}/{}", &hash[..2], hash)
}

fn zstd(data: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::encode_all(data, 3)?)
}

fn unzstd(data: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::decode_all(data)?)
}

async fn public_tables(conn: &mut PgConnection) -> Result<Vec<(String, Vec<String>)>> {
    let rows = sqlx::query(
        "SELECT c.table_name, array_agg(c.column_name::text ORDER BY c.ordinal_position) AS cols
         FROM information_schema.columns c JOIN information_schema.tables t ON t.table_name = c.table_name AND t.table_schema = c.table_schema
         WHERE c.table_schema = 'public' AND t.table_type = 'BASE TABLE' GROUP BY c.table_name ORDER BY c.table_name",
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows.iter().map(|r| (r.get::<String, _>("table_name"), r.get::<Vec<String>, _>("cols"))).collect())
}

fn quote(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Take one backup now. Returns its manifest.
pub async fn run(s: &AppState, cfg: &BackupConfig) -> Result<Manifest> {
    let target = BlobStore::open(&cfg.target).await.context("opening the backup target")?;
    let started = Utc::now();
    let id = started.format("%Y%m%d-%H%M%S").to_string();

    // 1. The database, from one consistent snapshot.
    let mut conn = s.db.acquire().await?;
    let mut tx = conn.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await?;
    let migration: i64 = sqlx::query_scalar("SELECT COALESCE(max(version), 0) FROM _sqlx_migrations").fetch_one(&mut *tx).await?;
    let mut tables = Vec::new();
    let mut db_bytes = 0u64;
    let mut chunk_rows: Vec<(String, String)> = Vec::new();
    for (name, cols) in public_tables(&mut tx).await? {
        if SKIP_TABLES.contains(&name.as_str()) {
            continue;
        }
        let col_list = cols.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        let rows: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {}", quote(&name))).fetch_one(&mut *tx).await?;
        let mut csv = Vec::new();
        {
            let mut stream = tx.copy_out_raw(&format!("COPY {} ({col_list}) TO STDOUT WITH (FORMAT csv)", quote(&name))).await?;
            while let Some(part) = stream.try_next().await? {
                csv.extend_from_slice(&part);
            }
        }
        let packed = zstd(&csv)?;
        db_bytes += packed.len() as u64;
        target.put_raw(&format!("db/{id}/{name}.csv.zst"), Bytes::from(packed)).await?;
        tables.push(TableInfo { name, columns: cols, rows });
    }
    for r in sqlx::query("SELECT location, hash FROM chunks").fetch_all(&mut *tx).await? {
        chunk_rows.push((r.get("location"), r.get("hash")));
    }
    tx.commit().await?;
    drop(conn);

    // 2. File data: copy each part the target does not have yet (shared across snapshots).
    let have: HashSet<String> = target.list_raw("chunks").await?.into_iter().filter_map(|(p, _)| p.rsplit('/').next().map(str::to_string)).collect();
    let mut seen = HashSet::new();
    let todo: Vec<(String, String)> = chunk_rows.iter().filter(|(_, h)| !have.contains(h) && seen.insert(h.clone())).cloned().collect();
    let total_chunks = chunk_rows.iter().map(|(_, h)| h).collect::<HashSet<_>>().len() as u64;
    let results: Vec<Result<u64>> = futures::stream::iter(todo)
        .map(|(loc, hash)| {
            let (s, target) = (s.clone(), target.clone());
            async move {
                let store = s.stores.get(&loc).with_context(|| format!("storage location {loc} is not configured"))?;
                let data = store.get_chunk(&hash).await.with_context(|| format!("reading {hash} from {loc}"))?;
                let n = data.len() as u64;
                target.put_raw(&chunk_rel(&hash), data).await?;
                Ok(n)
            }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    let mut new_chunks = 0u64;
    let mut new_bytes = 0u64;
    for r in results {
        new_bytes += r?;
        new_chunks += 1;
    }

    // 3. The manifest marks the snapshot complete.
    let m = Manifest { id: id.clone(), created_at: started, server_version: env!("CARGO_PKG_VERSION").into(), migration, tables, chunks: total_chunks, db_bytes, new_chunks, new_chunk_bytes: new_bytes };
    target.put_raw(&format!("db/{id}/manifest.json"), Bytes::from(serde_json::to_vec_pretty(&m)?)).await?;

    // 4. Retention, then remove file data no kept snapshot needs.
    prune(&target, cfg).await?;
    Ok(m)
}

/// Complete snapshots on the target, newest first.
pub async fn snapshots(target: &BlobStore) -> Result<Vec<Manifest>> {
    let mut out = Vec::new();
    for (path, _) in target.list_raw("db").await? {
        if path.ends_with("/manifest.json") {
            if let Ok(b) = target.get_raw(&path).await {
                if let Ok(m) = serde_json::from_slice::<Manifest>(&b) {
                    out.push(m);
                }
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

/// Which snapshots to keep: the newest per day for `keep_daily` days, and the newest per ISO
/// week for `keep_weekly` weeks. The newest snapshot is always kept.
pub fn keep_set(snaps: &[Manifest], cfg: &BackupConfig) -> HashSet<String> {
    let mut keep = HashSet::new();
    if let Some(first) = snaps.first() {
        keep.insert(first.id.clone());
    }
    let mut days = BTreeMap::new();
    let mut weeks = BTreeMap::new();
    for m in snaps {
        days.entry(m.created_at.date_naive()).or_insert_with(|| m.id.clone());
        let w = m.created_at.iso_week();
        weeks.entry((w.year(), w.week())).or_insert_with(|| m.id.clone());
    }
    keep.extend(days.into_iter().rev().take(cfg.keep_daily).map(|(_, id)| id));
    keep.extend(weeks.into_iter().rev().take(cfg.keep_weekly).map(|(_, id)| id));
    keep
}

async fn snapshot_chunks(target: &BlobStore, m: &Manifest) -> Result<Vec<(String, String)>> {
    let Some(t) = m.tables.iter().find(|t| t.name == "chunks") else { return Ok(Vec::new()) };
    let li = t.columns.iter().position(|c| c == "location");
    let hi = t.columns.iter().position(|c| c == "hash").context("the chunks table has no hash column")?;
    let csv = unzstd(&target.get_raw(&format!("db/{}/chunks.csv.zst", m.id)).await?)?;
    let text = String::from_utf8(csv)?;
    Ok(text
        .lines()
        .filter(|l| !l.is_empty())
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            Some((li.and_then(|i| f.get(i)).map(|s| s.to_string()).unwrap_or_else(|| crate::store::MAIN.into()), f.get(hi)?.to_string()))
        })
        .collect())
}

async fn prune(target: &BlobStore, cfg: &BackupConfig) -> Result<()> {
    let snaps = snapshots(target).await?;
    let keep = keep_set(&snaps, cfg);
    let mut removed = false;
    for m in snaps.iter().filter(|m| !keep.contains(&m.id)) {
        for (path, _) in target.list_raw(&format!("db/{}", m.id)).await? {
            target.delete_raw(&path).await?;
        }
        removed = true;
        tracing::info!(snapshot = %m.id, "removed an old backup");
    }
    if !removed {
        return Ok(());
    }
    let mut needed = HashSet::new();
    for m in snaps.iter().filter(|m| keep.contains(&m.id)) {
        needed.extend(snapshot_chunks(target, m).await?.into_iter().map(|(_, h)| h));
    }
    for (path, _) in target.list_raw("chunks").await? {
        if let Some(h) = path.rsplit('/').next() {
            if !needed.contains(h) {
                target.delete_raw(&path).await?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Scheduler

/// Run one backup and record it in `backup_runs`. Only one runs at a time.
pub async fn run_recorded(s: &AppState, cfg: &BackupConfig, by: Option<uuid::Uuid>) -> Result<()> {
    if s.backup_running.swap(true, Ordering::SeqCst) {
        anyhow::bail!("A backup is already running.");
    }
    let id: uuid::Uuid = match sqlx::query_scalar("INSERT INTO backup_runs (started_by) VALUES ($1) RETURNING id").bind(by).fetch_one(&s.db).await {
        Ok(id) => id,
        Err(e) => {
            s.backup_running.store(false, Ordering::SeqCst);
            return Err(e.into());
        }
    };
    let result = run(s, cfg).await;
    let (status, err, snap) = match &result {
        Ok(m) => ("done", None, Some(m)),
        Err(e) => ("failed", Some(format!("{e:#}")), None),
    };
    let _ = sqlx::query(
        "UPDATE backup_runs SET finished_at = now(), status = $2, error = $3, snapshot = $4, db_bytes = $5, new_chunks = $6, new_chunk_bytes = $7, total_chunks = $8 WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(&err)
    .bind(snap.map(|m| m.id.clone()))
    .bind(snap.map(|m| m.db_bytes as i64))
    .bind(snap.map(|m| m.new_chunks as i64))
    .bind(snap.map(|m| m.new_chunk_bytes as i64))
    .bind(snap.map(|m| m.chunks as i64))
    .execute(&s.db)
    .await;
    s.backup_running.store(false, Ordering::SeqCst);
    match &result {
        Ok(m) => tracing::info!(snapshot = %m.id, new_chunks = m.new_chunks, "backup finished"),
        Err(e) => tracing::error!(error = %format!("{e:#}"), "backup failed"),
    }
    result.map(|_| ())
}

/// When the next scheduled backup is due, after `now`.
pub fn next_run(cfg: &BackupConfig, now: DateTime<Utc>) -> DateTime<Utc> {
    let today = now.date_naive().and_hms_opt(cfg.at.0, cfg.at.1, 0).unwrap().and_utc();
    if today > now { today } else { today + chrono::Duration::days(1) }
}

/// Check every minute; run when the daily time has passed and no backup started since.
pub fn spawn_scheduler(s: AppState, cfg: BackupConfig) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let now = Utc::now();
            let due = next_run(&cfg, now) - chrono::Duration::days(1);
            let last: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT max(started_at) FROM backup_runs").fetch_one(&s.db).await.ok().flatten();
            // Also catches up a backup missed while the server was down.
            if now >= due && last.map(|l| l < due).unwrap_or(true) {
                let _ = run_recorded(&s, &cfg, None).await;
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------
// Restore

pub struct RestoreOptions {
    pub from: StorageConfig,
    pub snapshot: Option<String>,
    pub db_only: bool,
    pub force: bool,
}

/// Restore a snapshot into this server's (empty) database and storage locations.
pub async fn restore(config: &crate::Config, opts: RestoreOptions) -> Result<()> {
    let target = BlobStore::open(&opts.from).await.context("opening the backup")?;
    let snaps = snapshots(&target).await?;
    anyhow::ensure!(!snaps.is_empty(), "There are no complete backups in {}.", target.description);
    let m = match opts.snapshot.as_deref() {
        None | Some("latest") => snaps[0].clone(),
        Some(id) => snaps.iter().find(|m| m.id == id).cloned().with_context(|| format!("There is no backup {id}. Available: {}", snaps.iter().map(|m| m.id.as_str()).collect::<Vec<_>>().join(", ")))?,
    };
    println!("Restoring backup {} ({}), {} tables, {} file parts.", m.id, m.created_at, m.tables.len(), m.chunks);

    let db = crate::connect(config).await?; // runs migrations, so the schema is current
    let mut conn = db.acquire().await?;
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users").fetch_one(&mut *conn).await?;
    anyhow::ensure!(
        users == 0 || opts.force,
        "This database already has {users} people in it. Restore into an empty database, or pass --force to replace everything in it."
    );
    let current: BTreeMap<String, Vec<String>> = public_tables(&mut conn).await?.into_iter().collect();
    let mut tx = conn.begin().await?;
    // Load in any order: foreign keys are not checked while restoring.
    sqlx::query("SET LOCAL session_replication_role = replica").execute(&mut *tx).await?;
    if opts.force {
        let names: Vec<String> = current.keys().filter(|n| !SKIP_TABLES.contains(&n.as_str())).map(|n| quote(n)).collect();
        if !names.is_empty() {
            sqlx::query(&format!("TRUNCATE {} CASCADE", names.join(", "))).execute(&mut *tx).await?;
        }
    }
    for t in &m.tables {
        let Some(now_cols) = current.get(&t.name) else {
            println!("  skipped {}: this server has no such table", t.name);
            continue;
        };
        let missing: Vec<&String> = t.columns.iter().filter(|c| !now_cols.contains(c)).collect();
        anyhow::ensure!(missing.is_empty(), "Table {} has columns this server no longer has ({:?}); restore with the server version that made the backup ({}).", t.name, missing, m.server_version);
        let csv = unzstd(&target.get_raw(&format!("db/{}/{}.csv.zst", m.id, t.name)).await?)?;
        let col_list = t.columns.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ");
        let mut copy = tx.copy_in_raw(&format!("COPY {} ({col_list}) FROM STDIN WITH (FORMAT csv)", quote(&t.name))).await?;
        copy.send(csv).await?;
        let n = copy.finish().await?;
        anyhow::ensure!(n as i64 == t.rows, "{}: restored {n} rows, expected {}", t.name, t.rows);
    }
    // Serial columns continue after the restored rows.
    for r in sqlx::query("SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = 'public' AND column_default LIKE 'nextval%'").fetch_all(&mut *tx).await? {
        let (t, c): (String, String) = (r.get("table_name"), r.get("column_name"));
        sqlx::query(&format!("SELECT setval(pg_get_serial_sequence('{t}', '{c}'), GREATEST(COALESCE((SELECT max({}) FROM {}), 0), 1))", quote(&c), quote(&t)))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    println!("Database restored.");
    if opts.db_only {
        return Ok(());
    }

    let stores = crate::store::Locations::open(&config.storage).await?;
    let rows = snapshot_chunks(&target, &m).await?;
    let mut copied = 0u64;
    let mut skipped_locations: BTreeMap<String, u64> = BTreeMap::new();
    let results: Vec<Result<(String, bool)>> = futures::stream::iter(rows)
        .map(|(loc, hash)| {
            let target = target.clone();
            let store = stores.get(&loc).cloned();
            async move {
                let Some(store) = store else { return Ok((loc, false)) };
                if !store.has_chunk(&hash).await? {
                    let data = target.get_raw(&chunk_rel(&hash)).await.with_context(|| format!("the backup is missing file data {hash}"))?;
                    store.put_chunk(&hash, data).await?;
                }
                Ok((loc, true))
            }
        })
        .buffer_unordered(PARALLEL)
        .collect()
        .await;
    for r in results {
        let (loc, ok) = r?;
        if ok {
            copied += 1;
        } else {
            *skipped_locations.entry(loc).or_default() += 1;
        }
    }
    println!("File data restored: {copied} parts.");
    for (loc, n) in skipped_locations {
        println!("  {n} parts belong to storage location {loc}, which this server is not configured with; add it to MP_STORAGE_LOCATIONS and run the restore again with --force.");
    }
    Ok(())
}
