//! Branch update and merge.
//!
//! Each child branch row in `tree_entries` keeps `base_*`: the parent's version of that path as of
//! the last update from the parent. With that, for every path:
//! - the parent changed it since the last update when `parent != base`;
//! - this branch changed it when `child != base`.
//! Update from parent brings parent-only changes in, auto-merges text changed on both sides and
//! reports anything else as a conflict. Merge into parent requires the branch to be up to date,
//! so it can never conflict.

use std::collections::{BTreeSet, HashMap};

use bytes::Bytes;
use mp_core::chunk::{chunk_bytes, ChunkParams};
use mp_core::kind::{is_text_kind, kind_of, looks_like_text};
use mp_core::Manifest;
use serde::Serialize;
use sqlx::{PgConnection, Row};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

const MERGE_SIZE_LIMIT: i64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entry {
    pub file_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MergeReport {
    pub up_to_date: bool,
    pub only_here: usize,
    pub only_parent: usize,
    pub auto_merged: Vec<String>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Conflict {
    pub path: String,
    pub kind: &'static str,
}

#[derive(Debug, Clone)]
pub struct BranchRow {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub shares_claims: bool,
    pub protected: bool,
    pub archived: bool,
    pub head_change: i32,
    pub last_merged_change: i32,
}

pub async fn load_branch(db: &mut PgConnection, project: Uuid, branch: Uuid) -> ApiResult<BranchRow> {
    let r = sqlx::query("SELECT id, project_id, name, parent_id, shares_claims, protected, archived, head_change, last_merged_change FROM branches WHERE id = $1 AND project_id = $2")
        .bind(branch)
        .bind(project)
        .fetch_optional(&mut *db)
        .await?
        .ok_or_else(|| ApiError::not_found("That branch"))?;
    Ok(BranchRow {
        id: r.get("id"),
        project_id: r.get("project_id"),
        name: r.get("name"),
        parent_id: r.get("parent_id"),
        shares_claims: r.get("shares_claims"),
        protected: r.get("protected"),
        archived: r.get("archived"),
        head_change: r.get("head_change"),
        last_merged_change: r.get("last_merged_change"),
    })
}

/// The branch whose claims apply to `branch`: walk up while branches share claims.
pub async fn claim_scope(db: &mut PgConnection, branch: Uuid) -> ApiResult<Uuid> {
    let mut cur = branch;
    for _ in 0..64 {
        let r = sqlx::query("SELECT parent_id, shares_claims FROM branches WHERE id = $1").bind(cur).fetch_one(&mut *db).await?;
        let parent: Option<Uuid> = r.get("parent_id");
        let shares: bool = r.get("shares_claims");
        match parent {
            Some(p) if shares => cur = p,
            _ => return Ok(cur),
        }
    }
    Ok(cur)
}

/// Lock the project row so change numbers and tree updates are serialised; returns the next
/// change number without consuming it.
pub async fn lock_project(db: &mut PgConnection, project: Uuid) -> ApiResult<i32> {
    Ok(sqlx::query_scalar("SELECT next_change FROM projects WHERE id = $1 FOR UPDATE").bind(project).fetch_one(&mut *db).await?)
}

pub async fn take_change_number(db: &mut PgConnection, project: Uuid) -> ApiResult<i32> {
    Ok(sqlx::query_scalar("UPDATE projects SET next_change = next_change + 1 WHERE id = $1 RETURNING next_change - 1")
        .bind(project)
        .fetch_one(&mut *db)
        .await?)
}

type Tree = HashMap<String, (Entry, Entry)>;

async fn load_tree(db: &mut PgConnection, branch: Uuid) -> ApiResult<Tree> {
    let rows = sqlx::query("SELECT path, file_hash, manifest_hash, size, base_file_hash, base_manifest_hash, base_size FROM tree_entries WHERE branch_id = $1")
        .bind(branch)
        .fetch_all(&mut *db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.get::<String, _>("path"),
                (
                    Entry { file_hash: r.get("file_hash"), manifest_hash: r.get("manifest_hash"), size: r.get("size") },
                    Entry { file_hash: r.get("base_file_hash"), manifest_hash: r.get("base_manifest_hash"), size: r.get("base_size") },
                ),
            )
        })
        .collect())
}

/// Read a whole file's bytes from its manifest.
pub async fn read_blob(state: &AppState, db: &mut PgConnection, manifest_hash: &str) -> ApiResult<Vec<u8>> {
    let chunks: serde_json::Value = sqlx::query_scalar("SELECT chunks FROM manifests WHERE hash = $1").bind(manifest_hash).fetch_one(&mut *db).await?;
    let list: Vec<mp_core::ChunkRef> = serde_json::from_value(chunks).map_err(|e| anyhow::anyhow!("bad manifest {manifest_hash}: {e}"))?;
    let mut out = Vec::new();
    for c in list {
        let data = state.store.get_chunk(&c.hash).await?;
        out.extend(mp_core::decompress(&data, c.len as usize).map_err(ApiError::from)?);
    }
    Ok(out)
}

/// Store bytes as chunks and a manifest; returns the manifest.
pub async fn write_blob(state: &AppState, db: &mut PgConnection, data: &[u8]) -> ApiResult<Manifest> {
    let (manifest, chunks) = chunk_bytes(data, ChunkParams::default()).map_err(ApiError::from)?;
    for c in chunks {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM chunks WHERE hash = $1)").bind(&c.hash).fetch_one(&mut *db).await?;
        if !exists {
            let comp = mp_core::compress(&c.data).map_err(ApiError::from)?;
            let stored = comp.len() as i64;
            state.store.put_chunk(&c.hash, Bytes::from(comp)).await?;
            sqlx::query("INSERT INTO chunks (hash, size, stored_size) VALUES ($1,$2,$3) ON CONFLICT DO NOTHING")
                .bind(&c.hash)
                .bind(c.data.len() as i64)
                .bind(stored)
                .execute(&mut *db)
                .await?;
        }
    }
    save_manifest(db, &manifest).await?;
    Ok(manifest)
}

pub async fn save_manifest(db: &mut PgConnection, m: &Manifest) -> ApiResult<()> {
    sqlx::query("INSERT INTO manifests (hash, size, file_hash, chunks) VALUES ($1,$2,$3,$4) ON CONFLICT (hash) DO NOTHING")
        .bind(&m.hash)
        .bind(m.size as i64)
        .bind(&m.file_hash)
        .bind(serde_json::to_value(&m.chunks).unwrap_or_default())
        .execute(&mut *db)
        .await?;
    Ok(())
}

enum Act {
    /// Child takes the parent's version; base becomes the parent's version.
    TakeParent(String, Entry),
    /// Child keeps its version; base becomes the parent's version.
    SetBase(String, Entry),
    /// Child gets merged content; base becomes the parent's version.
    Write(String, Entry, Entry),
}

/// Work out (and with `apply`, perform) an update of `child` from its parent. Must be called
/// inside a transaction holding the project lock.
pub async fn update_from_parent(
    state: &AppState,
    db: &mut PgConnection,
    child: &BranchRow,
    resolutions: &HashMap<String, String>,
    apply: bool,
    author: Uuid,
) -> ApiResult<(MergeReport, Option<i32>)> {
    let parent_id = child.parent_id.ok_or_else(|| ApiError::invalid("Main has no parent branch to update from."))?;
    let parent = load_tree(db, parent_id).await?;
    let mine = load_tree(db, child.id).await?;
    let paths: BTreeSet<&String> = parent.keys().chain(mine.keys()).collect();
    let mut report = MergeReport::default();
    let mut acts = Vec::new();
    for path in paths {
        let p = parent.get(path).map(|(c, _)| c.clone()).unwrap_or_default();
        let (c, b) = mine.get(path).cloned().unwrap_or_default();
        let parent_changed = p.file_hash != b.file_hash;
        let mine_changed = c.file_hash != b.file_hash;
        if !parent_changed {
            if mine_changed {
                report.only_here += 1;
            }
            continue;
        }
        report.only_parent += 1;
        if !mine_changed {
            acts.push(Act::TakeParent(path.clone(), p));
            continue;
        }
        if c.file_hash == p.file_hash {
            acts.push(Act::SetBase(path.clone(), p));
            continue;
        }
        match resolutions.get(path.as_str()).map(String::as_str) {
            Some("parent") => {
                acts.push(Act::TakeParent(path.clone(), p));
                continue;
            }
            Some("mine") => {
                acts.push(Act::SetBase(path.clone(), p));
                continue;
            }
            _ => {}
        }
        let text = is_text_kind(kind_of(path));
        let mergeable = text
            && c.manifest_hash.is_some()
            && p.manifest_hash.is_some()
            && c.size <= MERGE_SIZE_LIMIT
            && p.size <= MERGE_SIZE_LIMIT
            && b.size <= MERGE_SIZE_LIMIT;
        if mergeable {
            let mine_bytes = read_blob(state, db, c.manifest_hash.as_deref().unwrap()).await?;
            let their_bytes = read_blob(state, db, p.manifest_hash.as_deref().unwrap()).await?;
            let base_bytes = match &b.manifest_hash {
                Some(h) => Some(read_blob(state, db, h).await?),
                None => None,
            };
            if looks_like_text(&mine_bytes) && looks_like_text(&their_bytes) {
                let base_s = base_bytes.as_deref().map(|x| String::from_utf8_lossy(x).into_owned());
                let merged = mp_core::merge::merge_text(
                    base_s.as_deref(),
                    &String::from_utf8_lossy(&mine_bytes),
                    &String::from_utf8_lossy(&their_bytes),
                );
                if let Ok(text) = merged {
                    report.auto_merged.push(path.clone());
                    if apply {
                        let m = write_blob(state, db, text.as_bytes()).await?;
                        acts.push(Act::Write(path.clone(), Entry { file_hash: Some(m.file_hash.clone()), manifest_hash: Some(m.hash.clone()), size: m.size as i64 }, p));
                    }
                    continue;
                }
            }
        }
        report.conflicts.push(Conflict { path: path.clone(), kind: if text { "text" } else { "binary" } });
    }
    report.up_to_date = report.only_parent == 0;
    if !apply || !report.conflicts.is_empty() || acts.is_empty() {
        return Ok((report, None));
    }

    let content_changes = acts.iter().filter(|a| !matches!(a, Act::SetBase(..))).count();
    let number = if content_changes > 0 { Some(take_change_number(db, child.project_id).await?) } else { None };
    let mut total = 0i64;
    for act in &acts {
        match act {
            Act::SetBase(path, base) => {
                sqlx::query("UPDATE tree_entries SET base_file_hash = $3, base_manifest_hash = $4, base_size = $5 WHERE branch_id = $1 AND path = $2")
                    .bind(child.id)
                    .bind(path)
                    .bind(&base.file_hash)
                    .bind(&base.manifest_hash)
                    .bind(base.size)
                    .execute(&mut *db)
                    .await?;
            }
            Act::TakeParent(path, new) | Act::Write(path, new, _) => {
                let base = match act {
                    Act::Write(_, _, b) => b,
                    _ => new,
                };
                let n = number.unwrap();
                let old = mine.get(path).map(|(c, _)| c.clone()).unwrap_or_default();
                upsert_entry(db, child.id, path, new, n, Some(base)).await?;
                let op = op_for(&old, new);
                record_change_file(db, child.project_id, n, path, op, new).await?;
                total += new.size;
            }
        }
    }
    if let Some(n) = number {
        let parent_name: String = sqlx::query_scalar("SELECT name FROM branches WHERE id = $1").bind(parent_id).fetch_one(&mut *db).await?;
        sqlx::query("INSERT INTO changes (project_id, number, branch_id, author_id, message, kind, file_count, size) VALUES ($1,$2,$3,$4,$5,'update',$6,$7)")
            .bind(child.project_id)
            .bind(n)
            .bind(child.id)
            .bind(author)
            .bind(format!("Updated from {parent_name}"))
            .bind(content_changes as i32)
            .bind(total)
            .execute(&mut *db)
            .await?;
        sqlx::query("UPDATE branches SET head_change = $2 WHERE id = $1").bind(child.id).bind(n).execute(&mut *db).await?;
    }
    Ok((report, number))
}

/// Merge `child` into its parent as one change. Requires the child to be up to date.
pub async fn merge_into_parent(db: &mut PgConnection, child: &BranchRow, message: &str, author: Uuid) -> ApiResult<i32> {
    let parent_id = child.parent_id.ok_or_else(|| ApiError::invalid("Main has no parent branch to merge into."))?;
    let parent = load_tree(db, parent_id).await?;
    let mine = load_tree(db, child.id).await?;
    let mut to_merge = Vec::new();
    for (path, (p, _)) in &parent {
        let b = mine.get(path).map(|(_, b)| b.clone()).unwrap_or_default();
        if p.file_hash != b.file_hash {
            return Err(ApiError::new(axum::http::StatusCode::CONFLICT, "needs_update", "The parent branch has new changes. Update from it first, then merge."));
        }
    }
    for (path, (c, b)) in &mine {
        if c.file_hash != b.file_hash {
            to_merge.push((path.clone(), c.clone()));
        }
    }
    if to_merge.is_empty() {
        return Err(ApiError::conflict("There is nothing to merge: this branch has no changes that its parent does not already have."));
    }
    to_merge.sort_by(|a, b| a.0.cmp(&b.0));
    let n = take_change_number(db, child.project_id).await?;
    let mut total = 0i64;
    for (path, c) in &to_merge {
        let old = parent.get(path).map(|(p, _)| p.clone()).unwrap_or_default();
        upsert_entry(db, parent_id, path, c, n, None).await?;
        sqlx::query("UPDATE tree_entries SET base_file_hash = file_hash, base_manifest_hash = manifest_hash, base_size = size WHERE branch_id = $1 AND path = $2")
            .bind(child.id)
            .bind(path)
            .execute(&mut *db)
            .await?;
        record_change_file(db, child.project_id, n, path, op_for(&old, c), c).await?;
        total += c.size;
    }
    let merged_from: Vec<i32> = sqlx::query_scalar("SELECT number FROM changes WHERE branch_id = $1 AND number > $2 ORDER BY number")
        .bind(child.id)
        .bind(child.last_merged_change)
        .fetch_all(&mut *db)
        .await?;
    sqlx::query("INSERT INTO changes (project_id, number, branch_id, author_id, message, kind, merged_from, file_count, size) VALUES ($1,$2,$3,$4,$5,'merge',$6,$7,$8)")
        .bind(child.project_id)
        .bind(n)
        .bind(parent_id)
        .bind(author)
        .bind(message)
        .bind(&merged_from)
        .bind(to_merge.len() as i32)
        .bind(total)
        .execute(&mut *db)
        .await?;
    sqlx::query("UPDATE branches SET head_change = $2 WHERE id = $1").bind(parent_id).bind(n).execute(&mut *db).await?;
    sqlx::query("UPDATE branches SET last_merged_change = $2 WHERE id = $1").bind(child.id).bind(n).execute(&mut *db).await?;
    Ok(n)
}

fn op_for(old: &Entry, new: &Entry) -> &'static str {
    match (&old.file_hash, &new.file_hash) {
        (_, None) => "delete",
        (None, Some(_)) => "add",
        _ => "edit",
    }
}

/// Write a branch's current version of `path`. `base` (child branches only) sets the base too;
/// `None` leaves an existing row's base untouched and a new row's base empty.
pub async fn upsert_entry(db: &mut PgConnection, branch: Uuid, path: &str, e: &Entry, change: i32, base: Option<&Entry>) -> ApiResult<()> {
    match base {
        Some(b) => {
            sqlx::query(
                "INSERT INTO tree_entries (branch_id, path, file_hash, manifest_hash, size, change_number, base_file_hash, base_manifest_hash, base_size)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                 ON CONFLICT (branch_id, path) DO UPDATE SET file_hash = EXCLUDED.file_hash, manifest_hash = EXCLUDED.manifest_hash,
                   size = EXCLUDED.size, change_number = EXCLUDED.change_number, base_file_hash = EXCLUDED.base_file_hash,
                   base_manifest_hash = EXCLUDED.base_manifest_hash, base_size = EXCLUDED.base_size",
            )
            .bind(branch)
            .bind(path)
            .bind(&e.file_hash)
            .bind(&e.manifest_hash)
            .bind(e.size)
            .bind(change)
            .bind(&b.file_hash)
            .bind(&b.manifest_hash)
            .bind(b.size)
            .execute(&mut *db)
            .await?;
        }
        None => {
            sqlx::query(
                "INSERT INTO tree_entries (branch_id, path, file_hash, manifest_hash, size, change_number) VALUES ($1,$2,$3,$4,$5,$6)
                 ON CONFLICT (branch_id, path) DO UPDATE SET file_hash = EXCLUDED.file_hash, manifest_hash = EXCLUDED.manifest_hash,
                   size = EXCLUDED.size, change_number = EXCLUDED.change_number",
            )
            .bind(branch)
            .bind(path)
            .bind(&e.file_hash)
            .bind(&e.manifest_hash)
            .bind(e.size)
            .bind(change)
            .execute(&mut *db)
            .await?;
        }
    }
    Ok(())
}

pub async fn record_change_file(db: &mut PgConnection, project: Uuid, change: i32, path: &str, op: &str, e: &Entry) -> ApiResult<()> {
    sqlx::query("INSERT INTO change_files (project_id, change_number, path, op, size, file_hash, manifest_hash) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(project)
        .bind(change)
        .bind(path)
        .bind(op)
        .bind(e.size)
        .bind(&e.file_hash)
        .bind(&e.manifest_hash)
        .execute(&mut *db)
        .await?;
    Ok(())
}
