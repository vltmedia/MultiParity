//! Chunk storage per project. Each project keeps its data in one storage location. While a
//! project is being moved, it also has a previous location: reads fall back to it, and anything
//! found only there is copied across when needed, so people can keep working during a move.

use std::collections::HashSet;

use bytes::Bytes;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::store::BlobStore;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct ProjectStore {
    pub current: String,
    pub previous: Option<String>,
}

impl ProjectStore {
    pub async fn load(db: &mut PgConnection, project: Uuid) -> ApiResult<Self> {
        let r = sqlx::query("SELECT storage, storage_previous FROM projects WHERE id = $1").bind(project).fetch_one(&mut *db).await?;
        Ok(ProjectStore { current: r.get("storage"), previous: r.get("storage_previous") })
    }

    fn all(&self) -> Vec<String> {
        std::iter::once(self.current.clone()).chain(self.previous.clone()).collect()
    }
}

/// A configured location by name. A project pointing at a location the server no longer has
/// (removed from `MP_STORAGE_LOCATIONS`) gets a clear error instead of silently losing data.
pub fn store<'a>(s: &'a AppState, name: &str) -> ApiResult<&'a BlobStore> {
    s.stores.get(name).ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "storage_unavailable",
            format!("This project's files are in the storage location \u{201c}{name}\u{201d}, which this server is not configured with. Ask the server admin to add it back to MP_STORAGE_LOCATIONS."),
        )
    })
}

/// Which of these chunks the project already has (in its location or, during a move, the
/// previous one).
pub async fn present(db: &mut PgConnection, ps: &ProjectStore, hashes: &[String]) -> ApiResult<HashSet<String>> {
    let rows: Vec<String> = sqlx::query_scalar("SELECT DISTINCT hash FROM chunks WHERE location = ANY($1) AND hash = ANY($2)")
        .bind(ps.all())
        .bind(hashes)
        .fetch_all(&mut *db)
        .await?;
    Ok(rows.into_iter().collect())
}

/// Store an uploaded (compressed) chunk in the project's location.
pub async fn put(s: &AppState, db: &mut PgConnection, ps: &ProjectStore, hash: &str, compressed: Bytes, raw_len: usize) -> ApiResult<()> {
    let stored = compressed.len() as i64;
    store(s, &ps.current)?.put_chunk(hash, compressed).await?;
    sqlx::query("INSERT INTO chunks (location, hash, size, stored_size) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(&ps.current)
        .bind(hash)
        .bind(raw_len as i64)
        .bind(stored)
        .execute(&mut *db)
        .await?;
    Ok(())
}

/// Read a chunk (compressed), from the project's location or, during a move, the previous one.
pub async fn get(s: &AppState, ps: &ProjectStore, hash: &str) -> ApiResult<Bytes> {
    match store(s, &ps.current)?.get_chunk(hash).await {
        Ok(b) => Ok(b),
        Err(object_store::Error::NotFound { .. }) if ps.previous.is_some() => Ok(store(s, ps.previous.as_ref().unwrap())?.get_chunk(hash).await?),
        Err(e) => Err(e.into()),
    }
}

/// Copy one chunk between locations (no-op if the target already has it). Returns the bytes
/// copied.
pub async fn copy(s: &AppState, db: &mut PgConnection, from: &str, to: &str, hash: &str) -> ApiResult<u64> {
    let have: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM chunks WHERE location = $1 AND hash = $2)").bind(to).bind(hash).fetch_one(&mut *db).await?;
    if have {
        return Ok(0);
    }
    let row = sqlx::query("SELECT size, stored_size FROM chunks WHERE location = $1 AND hash = $2").bind(from).bind(hash).fetch_optional(&mut *db).await?;
    let Some(row) = row else { return Ok(0) };
    let data = store(s, from)?.get_chunk(hash).await?;
    let n = data.len() as u64;
    store(s, to)?.put_chunk(hash, data).await?;
    sqlx::query("INSERT INTO chunks (location, hash, size, stored_size) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
        .bind(to)
        .bind(hash)
        .bind(row.get::<i64, _>("size"))
        .bind(row.get::<i64, _>("stored_size"))
        .execute(&mut *db)
        .await?;
    Ok(n)
}

/// Make sure every one of these chunks is in the project's current location, copying any that
/// are only in the previous one. Returns the hashes that are in neither.
pub async fn ensure_current(s: &AppState, db: &mut PgConnection, ps: &ProjectStore, hashes: &[String]) -> ApiResult<Vec<String>> {
    let here: HashSet<String> = sqlx::query_scalar::<_, String>("SELECT hash FROM chunks WHERE location = $1 AND hash = ANY($2)")
        .bind(&ps.current)
        .bind(hashes)
        .fetch_all(&mut *db)
        .await?
        .into_iter()
        .collect();
    let mut missing = Vec::new();
    for h in hashes.iter().filter(|h| !here.contains(*h)) {
        match &ps.previous {
            Some(prev) if copy(s, db, prev, &ps.current, h).await? > 0 => {}
            _ => missing.push(h.clone()),
        }
    }
    Ok(missing)
}
