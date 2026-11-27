//! Exporting a project: a branch's files at one change, as plain files or one .zip, written to an
//! export target. The target "server" is this server's exports folder (MP_EXPORT_DIR, default
//! /data/exports), whose .zip files can be downloaded from the web app; more targets (a folder,
//! a NAS, an S3 bucket) are named in MP_EXPORT_TARGETS the same way as storage locations:
//!
//!     MP_EXPORT_TARGETS="delivery=s3:bucket=client-delivery;endpoint=https://...;region=auto nas=mount:/data/nas-exports"
//!     MP_STORAGE_DELIVERY_ACCESS_KEY_ID=...  MP_STORAGE_DELIVERY_SECRET_ACCESS_KEY=...
//!
//! Files are streamed part by part, so a 40 GB file never sits in memory. A .zip is written on
//! this server first (zip64, so files over 4 GB are fine) and then sent to a remote target.

use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use object_store::path::Path as ObjPath;
use object_store::{ObjectStore, WriteMultipart};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::store::{parse_spec, BlobStore, StorageConfig};
use crate::AppState;

pub fn export_dir() -> String {
    std::env::var("MP_EXPORT_DIR").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/data/exports".into())
}

/// The targets an export can go to: "server" first, then MP_EXPORT_TARGETS.
pub fn targets() -> Vec<(String, StorageConfig)> {
    let mut out = vec![("server".to_string(), StorageConfig::Local { path: export_dir(), mounted: false })];
    for item in std::env::var("MP_EXPORT_TARGETS").unwrap_or_default().split_whitespace() {
        if let Some((name, spec)) = item.split_once('=') {
            match parse_spec(name, spec) {
                Ok(cfg) if name != "server" => out.push((name.to_string(), cfg)),
                Ok(_) => tracing::warn!("MP_EXPORT_TARGETS: the name `server` is taken by this server's own exports folder"),
                Err(e) => tracing::warn!(error = %e, "MP_EXPORT_TARGETS: {item}"),
            }
        }
    }
    out
}

pub fn targets_json() -> Value {
    json!(targets().iter().map(|(name, cfg)| json!({
        "name": name,
        "description": if name == "server" { "This server (download the .zip from here)".to_string() } else { cfg.describe() },
        "kind": match cfg { StorageConfig::S3 { .. } => "s3", StorageConfig::Local { mounted: true, .. } => "mount", _ => "local" },
        "downloadable": name == "server",
    })).collect::<Vec<_>>())
}

/// Mark exports a restart cut short, so nobody waits on them.
pub async fn fail_interrupted(db: &sqlx::PgPool) {
    let _ = sqlx::query("UPDATE exports SET status = 'failed', error = 'The server restarted while this was running. Start the export again.', finished_at = now() WHERE status IN ('queued', 'running')")
        .execute(db)
        .await;
}

/// Run one export in the background.
pub fn spawn(s: AppState, id: Uuid) {
    tokio::spawn(async move {
        let r = run(&s, id).await;
        let q = match &r {
            Ok(()) => sqlx::query("UPDATE exports SET status = 'done', error = NULL, finished_at = now(), files_done = files_total, bytes_done = bytes_total WHERE id = $1").bind(id),
            Err(e) => {
                tracing::warn!(export = %id, error = %format!("{e:#}"), "export failed");
                sqlx::query("UPDATE exports SET status = 'failed', error = $2, finished_at = now() WHERE id = $1").bind(id).bind(format!("{e:#}"))
            }
        };
        let _ = q.execute(&s.db).await;
        if r.is_ok() {
            after(&s, id).await;
        }
    });
}

/// After a good export: archive the project if asked, and tell webhooks.
async fn after(s: &AppState, id: Uuid) {
    let Ok(row) = sqlx::query("SELECT e.project_id, e.archive_after, e.created_by, e.name, e.target, p.org_id FROM exports e JOIN projects p ON p.id = e.project_id WHERE e.id = $1").bind(id).fetch_one(&s.db).await else { return };
    let project: Uuid = row.get("project_id");
    let org: Uuid = row.get("org_id");
    let by: Option<Uuid> = row.get("created_by");
    crate::events::emit(&s.db, &s.events, Some(org), Some(project), by, "project.exported", json!({ "export": id, "name": row.get::<String,_>("name"), "target": row.get::<String,_>("target") })).await;
    if row.get::<bool, _>("archive_after") {
        let _ = sqlx::query("UPDATE projects SET archived_at = now() WHERE id = $1 AND archived_at IS NULL").bind(project).execute(&s.db).await;
        crate::events::emit(&s.db, &s.events, Some(org), Some(project), by, "project.archived", json!({ "after_export": id })).await;
    }
}

struct Entry {
    path: String,
    size: i64,
    chunks: Vec<mp_core::ChunkRef>,
}

async fn run(s: &AppState, id: Uuid) -> Result<()> {
    let row = sqlx::query("SELECT project_id, branch_id, target, format, name FROM exports WHERE id = $1").bind(id).fetch_one(&s.db).await?;
    let project: Uuid = row.get("project_id");
    let branch: Uuid = row.get("branch_id");
    let target: String = row.get("target");
    let format: String = row.get("format");
    let name: String = row.get("name");
    let cfg = targets().into_iter().find(|(n, _)| *n == target).map(|(_, c)| c).with_context(|| format!("the export target `{target}` is no longer set up on this server"))?;
    let store = BlobStore::open(&cfg).await.with_context(|| format!("opening the export target `{target}`"))?;

    // What to export: the branch as it is now (the change was fixed when the export was made).
    let rows = sqlx::query(
        "SELECT t.path, t.size, m.chunks FROM tree_entries t JOIN manifests m ON m.hash = t.manifest_hash
         WHERE t.branch_id = $1 AND t.file_hash IS NOT NULL ORDER BY t.path",
    )
    .bind(branch)
    .fetch_all(&s.db)
    .await?;
    let mut entries = Vec::with_capacity(rows.len());
    for r in rows {
        entries.push(Entry { path: r.get("path"), size: r.get("size"), chunks: serde_json::from_value(r.get("chunks"))? });
    }
    let bytes_total: i64 = entries.iter().map(|e| e.size).sum();
    sqlx::query("UPDATE exports SET status = 'running', files_total = $2, bytes_total = $3 WHERE id = $1").bind(id).bind(entries.len() as i32).bind(bytes_total).execute(&s.db).await?;
    let mut conn = s.db.acquire().await?;
    let ps = crate::chunks::ProjectStore::load(&mut conn, project).await.map_err(|e| anyhow::anyhow!(e.message))?;
    drop(conn);

    let mut progress = Progress { id, files: 0, bytes: 0, last: Instant::now() };
    if format == "files" {
        for e in &entries {
            let path = ObjPath::from(format!("{}/{}", store_prefixed(&store, &name), e.path));
            let mut w = WriteMultipart::new(store.object_store().put_multipart(&path).await.with_context(|| format!("starting {}", e.path))?);
            for c in &e.chunks {
                let raw = read_part(s, &ps, c).await?;
                w.wait_for_capacity(4).await?;
                progress.bytes += raw.len() as i64;
                w.write(&raw);
            }
            w.finish().await.with_context(|| format!("finishing {}", e.path))?;
            progress.files += 1;
            progress.save(&s.db, false).await;
        }
    } else {
        // Write the .zip here, from a thread fed part by part.
        let dir = std::path::PathBuf::from(export_dir());
        let local = target == "server";
        let zip_path = if local { dir.join(format!("{name}.zip")) } else { dir.join(".partial").join(format!("{id}.zip")) };
        std::fs::create_dir_all(zip_path.parent().unwrap()).with_context(|| format!("creating {}", zip_path.parent().unwrap().display()))?;
        let (tx, rx) = std::sync::mpsc::sync_channel::<Piece>(32);
        let zp = zip_path.clone();
        let writer = tokio::task::spawn_blocking(move || write_zip(&zp, rx));
        let sent = async {
            for e in &entries {
                tx.send(Piece::Start(e.path.clone(), e.size as u64)).map_err(|_| anyhow::anyhow!("the .zip writer stopped"))?;
                for c in &e.chunks {
                    let raw = read_part(s, &ps, c).await?;
                    progress.bytes += raw.len() as i64;
                    tx.send(Piece::Data(raw)).map_err(|_| anyhow::anyhow!("the .zip writer stopped"))?;
                }
                progress.files += 1;
                progress.save(&s.db, false).await;
            }
            drop(tx);
            Ok::<_, anyhow::Error>(())
        }
        .await;
        // If writing failed, that is the reason to report (sending then fails only because of it).
        writer.await.context("the .zip writer")??;
        sent?;
        if !local {
            // Send the finished .zip to the target, then drop the copy here.
            let path = ObjPath::from(format!("{}.zip", store_prefixed(&store, &name)));
            let mut w = WriteMultipart::new(store.object_store().put_multipart(&path).await?);
            let mut f = tokio::fs::File::open(&zip_path).await?;
            let mut buf = vec![0u8; 8 << 20];
            loop {
                let n = tokio::io::AsyncReadExt::read(&mut f, &mut buf).await?;
                if n == 0 {
                    break;
                }
                w.wait_for_capacity(4).await?;
                w.write(&buf[..n]);
            }
            w.finish().await.context("sending the .zip")?;
            let _ = tokio::fs::remove_file(&zip_path).await;
        }
    }
    progress.save(&s.db, true).await;
    Ok(())
}

fn store_prefixed(store: &BlobStore, name: &str) -> String {
    store.prefixed(name)
}

async fn read_part(s: &AppState, ps: &crate::chunks::ProjectStore, c: &mp_core::ChunkRef) -> Result<Vec<u8>> {
    let data = crate::chunks::get(s, ps, &c.hash).await.map_err(|e| anyhow::anyhow!(e.message))?;
    mp_core::decompress(&data, c.len as usize)
}

enum Piece {
    Start(String, u64),
    Data(Vec<u8>),
}

fn write_zip(path: &std::path::Path, rx: std::sync::mpsc::Receiver<Piece>) -> Result<()> {
    let f = std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut z = zip::ZipWriter::new(std::io::BufWriter::with_capacity(1 << 20, f));
    let mut started = false;
    for piece in rx {
        match piece {
            Piece::Start(name, size) => {
                // Stored, not deflated: game and art files are mostly compressed already, and
                // this keeps exports fast. zip64 for anything near 4 GB.
                let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(size >= 0xFFFF_0000);
                z.start_file(name, opts)?;
                started = true;
            }
            Piece::Data(d) => {
                anyhow::ensure!(started, "file data before a file");
                z.write_all(&d)?;
            }
        }
    }
    z.finish()?.flush()?;
    Ok(())
}

struct Progress {
    id: Uuid,
    files: i32,
    bytes: i64,
    last: Instant,
}

impl Progress {
    async fn save(&mut self, db: &sqlx::PgPool, force: bool) {
        if !force && self.last.elapsed() < Duration::from_millis(800) {
            return;
        }
        self.last = Instant::now();
        let _ = sqlx::query("UPDATE exports SET files_done = $2, bytes_done = $3 WHERE id = $1").bind(self.id).bind(self.files).bind(self.bytes).execute(db).await;
    }
}

/// A one-time download link's token is checked here; the file is streamed from disk.
pub fn zip_on_disk(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(export_dir()).join(format!("{name}.zip"))
}
