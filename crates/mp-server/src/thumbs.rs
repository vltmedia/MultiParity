//! Thumbnails and previews of published files.
//!
//! Publishing queues every file that can be drawn; a background worker reads it from storage
//! and keeps two JPEGs in the database: `small` (256 px, for lists) and `large` (1280 px, for
//! the preview). Drawn: common image formats including TGA, TIFF, EXR and HDR (tone mapped),
//! and Blender files, from the preview Blender saves inside them.

use std::time::Duration;

use sqlx::Row;
use uuid::Uuid;

use crate::AppState;

pub use mp_core::thumbs::{draw, source_of, Drawn, Source, LARGE, SMALL};

/// Files bigger than this are not read for a thumbnail.
const MAX_BYTES: i64 = mp_core::thumbs::MAX_BYTES as i64;

/// Queue files of a project for drawing (those with a drawable extension and no row yet).
pub async fn enqueue(db: &sqlx::PgPool, project: Uuid, files: &[(String, String, String, i64)]) {
    for (path, file_hash, manifest_hash, size) in files {
        if source_of(path).is_none() || *size > MAX_BYTES {
            continue;
        }
        let _ = sqlx::query("INSERT INTO thumbnails (file_hash, project_id, manifest_hash, path) VALUES ($1,$2,$3,$4) ON CONFLICT (file_hash) DO NOTHING")
            .bind(file_hash)
            .bind(project)
            .bind(manifest_hash)
            .bind(path)
            .execute(db)
            .await;
    }
}

/// Queue drawable files on any branch that have no thumbnail yet (new publishes, merges, and
/// files published before thumbnails existed). Returns how many were queued.
async fn find_new(db: &sqlx::PgPool) -> crate::error::ApiResult<usize> {
    let rows = sqlx::query(
        "SELECT DISTINCT ON (t.file_hash) t.path, t.file_hash, t.manifest_hash, t.size, b.project_id
         FROM tree_entries t JOIN branches b ON b.id = t.branch_id
         LEFT JOIN thumbnails th ON th.file_hash = t.file_hash
         WHERE th.file_hash IS NULL AND t.file_hash IS NOT NULL AND t.manifest_hash IS NOT NULL
           AND lower(t.path) ~ '[.](png|jpe?g|gif|webp|bmp|tga|tiff?|exr|hdr|blend)$'
         LIMIT 500",
    )
    .fetch_all(db)
    .await?;
    let n = rows.len();
    for r in rows {
        let project: Uuid = r.get("project_id");
        enqueue(db, project, &[(r.get("path"), r.get("file_hash"), r.get("manifest_hash"), r.get("size"))]).await;
    }
    Ok(n)
}

/// Draw queued files, one at a time, for as long as the server runs.
pub fn spawn_worker(s: AppState) {
    tokio::spawn(async move {
        loop {
            match work_one(&s).await {
                Ok(true) => continue,
                Ok(false) => {
                    if !matches!(find_new(&s.db).await, Ok(n) if n > 0) {
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e.message, "thumbnail worker");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });
}

/// Draw the oldest queued file. `Ok(false)` when there was nothing to do.
async fn work_one(s: &AppState) -> crate::error::ApiResult<bool> {
    let Some(r) = sqlx::query(
        "UPDATE thumbnails SET status = 'pending', done_at = now() WHERE file_hash = (
           SELECT file_hash FROM thumbnails WHERE status = 'pending' AND (done_at IS NULL OR done_at < now() - interval '10 minutes')
           ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED)
         RETURNING file_hash, project_id, manifest_hash, path",
    )
    .fetch_optional(&s.db)
    .await?
    else {
        return Ok(false);
    };
    let file_hash: String = r.get("file_hash");
    let path: String = r.get("path");
    let manifest: String = r.get("manifest_hash");
    let project: Option<Uuid> = r.get("project_id");
    let result = async {
        let project = project.ok_or_else(|| "the project is gone".to_string())?;
        let mut conn = s.db.acquire().await.map_err(|e| e.to_string())?;
        let ps = crate::chunks::ProjectStore::load(&mut conn, project).await.map_err(|e| e.message)?;
        let bytes = crate::branches::read_blob(s, &mut conn, &ps, &manifest).await.map_err(|e| e.message)?;
        drop(conn);
        let source = source_of(&path).ok_or_else(|| "not a drawable file".to_string())?;
        tokio::task::spawn_blocking(move || draw(source, &bytes)).await.map_err(|e| e.to_string())?
    }
    .await;
    let q = match result {
        Ok(Some(d)) => sqlx::query("UPDATE thumbnails SET status = 'ready', width = $2, height = $3, small = $4, large = $5, error = NULL, done_at = now() WHERE file_hash = $1")
            .bind(&file_hash)
            .bind(d.width as i32)
            .bind(d.height as i32)
            .bind(d.small)
            .bind(d.large),
        Ok(None) => sqlx::query("UPDATE thumbnails SET status = 'none', done_at = now() WHERE file_hash = $1").bind(&file_hash),
        Err(e) => {
            tracing::info!(path = %path, error = %e, "no thumbnail");
            sqlx::query("UPDATE thumbnails SET status = 'failed', error = $2, done_at = now() WHERE file_hash = $1").bind(&file_hash).bind(e)
        }
    };
    q.execute(&s.db).await?;
    Ok(true)
}

