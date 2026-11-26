//! Checkpoints: private save points of unpublished work.
//!
//! Where the data lives depends on the project (or this folder's own setting):
//! - **On this computer**: `.mp/cache` holds only the parts that exist nowhere else, within a
//!   space limit (a tenth of the free disk unless set). A checkpoint that would not fit is not
//!   written at all.
//! - **On the server**: parts are uploaded and only a small list stays here, so nothing heavy
//!   sits on a laptop and checkpoints survive a dead disk.
//!
//! Either way, files on the no-checkpoint list (render output, simulation caches; see
//! [`scan::DEFAULT_NO_CHECKPOINT`]) are left out unless asked for.

use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use mp_core::Manifest;
use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::ops::{device_name, manifest_of, read_part, stamp, upload_missing, write_from_manifest, PartSource};
use crate::scan;
use crate::types::PublishEntry;
use crate::workspace::Workspace;

const MIB: u64 = 1024 * 1024;
/// Checkpoints never take the disk below this much free space.
const RESERVE: u64 = 1024 * MIB;
/// Below this much free space, people are warned.
const LOW_DISK: u64 = 5 * 1024 * MIB;
/// Files moved aside instead of being overwritten are kept this long.
const SAFETY_DAYS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub note: Option<String>,
    pub auto: bool,
    /// Made by MultiParity before rewinding, discarding or picking up a draft, as an undo.
    #[serde(default)]
    pub system: bool,
    /// Its file data is kept on the server, not in this folder.
    #[serde(default)]
    pub on_server: bool,
    pub created_at: String,
    pub branch_id: String,
    pub files: Vec<CheckpointFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFile {
    pub path: String,
    /// `None` records that the file was deleted at this point.
    pub manifest: Option<Manifest>,
    /// The file's size and modification time when checkpointed, so the next checkpoint does not
    /// read an unchanged file again.
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKind {
    /// Someone chose "Save checkpoint". Never removed automatically.
    Manual,
    /// Taken by the daemon when files are saved.
    Auto,
    /// The undo point before an operation that replaces files.
    Undo,
}

pub struct CheckpointOptions<'a> {
    pub note: Option<&'a str>,
    pub kind: CheckpointKind,
    /// Include files on the no-checkpoint list too.
    pub include_all: bool,
}

impl<'a> CheckpointOptions<'a> {
    pub fn manual(note: Option<&'a str>) -> Self {
        CheckpointOptions { note, kind: CheckpointKind::Manual, include_all: false }
    }
    pub fn auto() -> Self {
        CheckpointOptions { note: None, kind: CheckpointKind::Auto, include_all: false }
    }
    pub fn undo(note: &'a str) -> Self {
        CheckpointOptions { note: Some(note), kind: CheckpointKind::Undo, include_all: false }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TooBig {
    pub needed_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CheckpointOutcome {
    pub checkpoint: Option<Checkpoint>,
    /// Left out by the no-checkpoint list (render output, caches).
    pub left_out: Vec<String>,
    pub left_out_bytes: u64,
    /// Nothing was saved because it would not fit on this computer.
    pub too_big: Option<TooBig>,
    /// Nothing was saved because nothing changed since the last checkpoint.
    pub unchanged: bool,
    pub on_server: bool,
}

impl CheckpointOutcome {
    /// Changed files this checkpoint does not cover. Before replacing one of these, move it to
    /// `.mp/safety/` instead.
    pub fn unprotected(&self, all_changed: &[String]) -> HashSet<String> {
        if self.checkpoint.is_none() {
            all_changed.iter().cloned().collect()
        } else {
            self.left_out.iter().cloned().collect()
        }
    }
}

fn checkpoints_dir(ws: &Workspace) -> std::path::PathBuf {
    Workspace::mp_dir(&ws.root).join("checkpoints")
}

fn disk_free(ws: &Workspace) -> u64 {
    fs2::available_space(&ws.root).unwrap_or(u64::MAX)
}

/// Bytes in `.mp/cache`.
fn cache_used(ws: &Workspace) -> u64 {
    walkdir::WalkDir::new(ws.cache_dir()).into_iter().flatten().filter(|e| e.file_type().is_file()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()
}

/// The most space checkpoints may use in this folder: the folder's own limit, or a tenth of
/// what is free on the disk (counting what checkpoints already use as free).
pub fn limit_bytes(ws: &Workspace, used: u64) -> u64 {
    match ws.config.checkpoint_limit_mb {
        Some(mb) => mb.saturating_mul(MIB),
        None => disk_free(ws).saturating_add(used) / 10,
    }
}

/// Save a private save point of new, edited and deleted files.
///
/// With `api`, parts the server already has are not stored again, and projects that keep
/// checkpoints on the server send them there. Without it (offline) everything stays local.
pub async fn checkpoint(api: Option<&Api>, ws: &Workspace, opts: CheckpointOptions<'_>) -> Result<CheckpointOutcome> {
    let mut out = CheckpointOutcome::default();
    let st = scan::status(ws, &[])?;
    if st.is_clean() {
        return Ok(out);
    }
    let rules = if opts.include_all { None } else { Some(scan::Ignores::load_no_checkpoint(&ws.root)?) };
    let mut todo = Vec::new();
    for f in st.new.iter().chain(&st.edited) {
        if rules.as_ref().and_then(|r| r.check(&f.path, false)).is_some() {
            out.left_out.push(f.path.clone());
            out.left_out_bytes += f.size;
        } else {
            todo.push(f);
        }
    }
    if todo.is_empty() && st.deleted.is_empty() {
        return Ok(out);
    }

    // Manifests, reusing the last checkpoint's for files that have not been touched since.
    let previous = list_checkpoints(ws)?;
    let last = previous.iter().find(|c| c.branch_id == ws.config.branch_id);
    let reuse: HashMap<&str, &CheckpointFile> = last.map(|c| c.files.iter().map(|f| (f.path.as_str(), f)).collect()).unwrap_or_default();
    let mut files = Vec::new();
    let mut manifests: BTreeMap<String, Manifest> = BTreeMap::new();
    for f in &todo {
        let m = match reuse.get(f.path.as_str()) {
            Some(cf) if cf.mtime != 0 && cf.mtime == f.mtime && cf.size == f.size && cf.manifest.is_some() => cf.manifest.clone().unwrap(),
            _ => manifest_of(ws, &f.path)?,
        };
        files.push(CheckpointFile { path: f.path.clone(), manifest: Some(m.clone()), size: f.size, mtime: f.mtime });
        manifests.insert(f.path.clone(), m);
    }
    for f in &st.deleted {
        files.push(CheckpointFile { path: f.path.clone(), manifest: None, size: 0, mtime: 0 });
    }
    if opts.kind == CheckpointKind::Auto {
        if let Some(last) = last {
            let key = |fs: &[CheckpointFile]| -> BTreeMap<String, Option<String>> { fs.iter().map(|f| (f.path.clone(), f.manifest.as_ref().map(|m| m.hash.clone()))).collect() };
            if key(&last.files) == key(&files) {
                out.unchanged = true;
                return Ok(out);
            }
        }
    }

    let now = chrono::Utc::now();
    let mut cp = Checkpoint {
        id: format!("{}-{}", now.format("%Y%m%d%H%M%S"), &uuid::Uuid::new_v4().to_string()[..6]),
        note: opts.note.map(str::to_string),
        auto: opts.kind == CheckpointKind::Auto,
        system: opts.kind == CheckpointKind::Undo,
        on_server: false,
        created_at: now.to_rfc3339(),
        branch_id: ws.config.branch_id.clone(),
        files,
    };

    // On the server, when the project (or this folder) says so and the server can be reached.
    if ws.config.checkpoints_on_server() {
        if let Some(api) = api {
            match save_on_server(api, ws, &cp, &manifests).await {
                Ok(()) => {
                    cp.on_server = true;
                    crate::workspace::write_json_atomic(&checkpoints_dir(ws).join(format!("{}.json", cp.id)), &cp)?;
                    out.on_server = true;
                    out.checkpoint = Some(cp);
                    return Ok(out);
                }
                Err(e) => tracing::warn!(error = %e, "could not keep the checkpoint on the server; keeping it on this computer"),
            }
        }
    }

    // On this computer: only the parts that exist nowhere else, and only if they fit.
    let known = match api {
        Some(api) => published_parts(api, ws, manifests.keys()).await,
        None => HashSet::new(),
    };
    let mut sources: HashMap<String, PartSource> = HashMap::new();
    let mut needed = 0u64;
    for (path, m) in &manifests {
        let mut offset = 0u64;
        for c in &m.chunks {
            if !known.contains(&c.hash) && !sources.contains_key(&c.hash) && !ws.cached_chunk(&c.hash).exists() {
                sources.insert(c.hash.clone(), PartSource { path: path.clone(), offset, len: c.len });
                needed += c.len;
            }
            offset += c.len;
        }
    }
    let used = cache_used(ws);
    let by_disk = disk_free(ws).saturating_sub(RESERVE);
    let available = match opts.kind {
        // The limit governs what MultiParity takes on its own; a person's own checkpoint may
        // use what the disk has.
        CheckpointKind::Manual => by_disk,
        _ => limit_bytes(ws, used).saturating_sub(used).min(by_disk),
    };
    if needed > available {
        out.too_big = Some(TooBig { needed_bytes: needed, available_bytes: available });
        return Ok(out);
    }
    for (hash, src) in &sources {
        let comp = read_part(ws, hash, src)?;
        ws.put_cached_chunk(hash, &comp)?;
    }
    crate::workspace::write_json_atomic(&checkpoints_dir(ws).join(format!("{}.json", cp.id)), &cp)?;
    if cp.auto {
        prune_auto_checkpoints(ws)?;
    }
    out.checkpoint = Some(cp);
    Ok(out)
}

async fn save_on_server(api: &Api, ws: &Workspace, cp: &Checkpoint, manifests: &BTreeMap<String, Manifest>) -> Result<()> {
    let (_, uploaded) = upload_missing(api, ws, manifests, |_, _| {}).await?;
    let entries: Vec<PublishEntry> = cp
        .files
        .iter()
        .map(|f| PublishEntry { path: f.path.clone(), op: if f.manifest.is_some() { "put" } else { "delete" }.into(), prev_file_hash: None, manifest: f.manifest.clone() })
        .collect();
    let _: serde_json::Value = api
        .post(
            &format!("/projects/{}/checkpoints", ws.config.project_id),
            &serde_json::json!({ "id": cp.id, "branch": cp.branch_id, "device": device_name(), "note": cp.note, "auto": cp.auto, "system": cp.system, "stored": uploaded, "entries": entries }),
        )
        .await?;
    Ok(())
}

/// Parts of the published versions these files were edited from. The server keeps published
/// history for good, so a checkpoint need not store them. Best effort: what cannot be fetched
/// is simply stored locally.
async fn published_parts(api: &Api, ws: &Workspace, paths: impl Iterator<Item = &String>) -> HashSet<String> {
    let mut parts = HashSet::new();
    let mut seen = HashSet::new();
    for p in paths {
        let Some(base) = ws.index.files.get(p).map(|i| i.manifest_hash.clone()).or_else(|| ws.remote.files.get(p).map(|r| r.manifest_hash.clone())) else { continue };
        if seen.insert(base.clone()) {
            if let Ok(m) = api.manifest(&ws.config.project_id, &base).await {
                parts.extend(m.chunks.into_iter().map(|c| c.hash));
            }
        }
    }
    parts
}

pub fn list_checkpoints(ws: &Workspace) -> Result<Vec<Checkpoint>> {
    let mut out = Vec::new();
    let dir = checkpoints_dir(ws);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if e.path().extension().map(|x| x == "json").unwrap_or(false) {
                if let Some(cp) = crate::workspace::read_json::<Checkpoint>(&e.path())? {
                    out.push(cp);
                }
            }
        }
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

/// This person's checkpoints for the project that the server keeps, from every computer.
pub async fn list_on_server(api: &Api, ws: &Workspace) -> Result<Vec<serde_json::Value>> {
    api.get(&format!("/projects/{}/checkpoints", ws.config.project_id)).await
}

/// Keep the newest 50 automatic checkpoints and none older than 30 days. (The server thins its
/// own copies by the same rule.)
fn prune_auto_checkpoints(ws: &Workspace) -> Result<()> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(30);
    let autos: Vec<Checkpoint> = list_checkpoints(ws)?.into_iter().filter(|c| c.auto).collect();
    for (i, cp) in autos.iter().enumerate() {
        let old = chrono::DateTime::parse_from_rfc3339(&cp.created_at).map(|t| t < cutoff).unwrap_or(false);
        if i >= 50 || old {
            let _ = std::fs::remove_file(checkpoints_dir(ws).join(format!("{}.json", cp.id)));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Space on this computer

#[derive(Debug, Clone, Serialize, Default)]
pub struct CacheReport {
    /// Space checkpoints use on this computer after trimming.
    pub kept_bytes: u64,
    pub kept_parts: usize,
    pub freed_bytes: u64,
    pub freed_parts: usize,
    /// Old automatic or undo checkpoints removed to stay under the limit.
    pub removed_checkpoints: Vec<String>,
    pub limit_bytes: u64,
    /// True when the limit is a tenth of the free disk rather than a number someone set.
    pub limit_is_automatic: bool,
    /// False when the server could not be asked; then only parts no checkpoint uses were removed.
    pub checked_server: bool,
    /// Where this folder keeps checkpoints: "server" or "local".
    pub storage: String,
    /// Whether checkpoints are taken automatically while working in this folder.
    pub automatic: bool,
    pub on_server: usize,
    pub on_this_computer: usize,
    /// Files moved aside instead of being overwritten (`.mp/safety`), kept 30 days.
    pub safety_bytes: u64,
    pub safety_freed_bytes: u64,
    pub free_disk_bytes: u64,
    pub low_disk: bool,
}

/// Parts written in the last few minutes are left alone, in case a checkpoint that uses them is
/// still being written by another process. `MP_CACHE_GRACE_SECS` overrides it (tests).
fn cache_grace() -> std::time::Duration {
    let secs = std::env::var("MP_CACHE_GRACE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(600);
    std::time::Duration::from_secs(secs)
}

/// Keep only checkpoint data that exists nowhere else, within the folder's space limit.
///
/// A part can be dropped when no checkpoint needs it, or when the server keeps it for good:
/// it belongs to a published version (history is never deleted) or to a checkpoint the server
/// holds, so rewinding can fetch it again. Over the limit, the oldest automatic checkpoints go
/// first, then undo checkpoints older than a week. Checkpoints people made themselves are never
/// removed. Files in `.mp/safety` older than 30 days are removed too.
pub async fn trim_cache(api: Option<&Api>, ws: &Workspace) -> Result<CacheReport> {
    let mut report = CacheReport {
        checked_server: api.is_some(),
        limit_is_automatic: ws.config.checkpoint_limit_mb.is_none(),
        storage: if ws.config.checkpoints_on_server() { "server" } else { "local" }.into(),
        automatic: ws.config.auto_checkpoints_on(),
        ..Default::default()
    };

    // What is in the cache now.
    let mut cached: HashMap<String, (std::path::PathBuf, u64, bool)> = HashMap::new();
    let mut junk = Vec::new();
    let grace = cache_grace();
    if let Ok(rd) = std::fs::read_dir(ws.cache_dir()) {
        for shard in rd.flatten() {
            let Ok(files) = std::fs::read_dir(shard.path()) else { continue };
            for f in files.flatten() {
                let Ok(meta) = f.metadata() else { continue };
                let name = f.file_name().to_string_lossy().to_string();
                let recent = meta.modified().ok().and_then(|t| t.elapsed().ok()).map(|e| e < grace).unwrap_or(true);
                if mp_core::manifest::is_hash(&name) {
                    cached.insert(name, (f.path(), meta.len(), recent));
                } else if !recent {
                    junk.push(f.path());
                }
            }
        }
    }
    let total_cached: u64 = cached.values().map(|(_, n, _)| *n).sum();
    let limit = limit_bytes(ws, total_cached);
    report.limit_bytes = limit;

    let published: HashSet<&str> = ws.index.files.values().map(|i| i.manifest_hash.as_str()).chain(ws.remote.files.values().map(|r| r.manifest_hash.as_str())).collect();
    // Parts of the published version each checkpointed file was edited from.
    let mut base_parts: HashMap<String, Option<HashSet<String>>> = HashMap::new();

    let week_ago = chrono::Utc::now() - chrono::Duration::days(7);
    let needed = loop {
        let cps = list_checkpoints(ws)?;
        report.on_server = cps.iter().filter(|c| c.on_server).count();
        report.on_this_computer = cps.len() - report.on_server;
        let mut needed: HashSet<String> = HashSet::new();
        for cp in cps.iter().filter(|c| !c.on_server) {
            for f in &cp.files {
                let Some(m) = &f.manifest else { continue };
                if published.contains(m.hash.as_str()) {
                    continue;
                }
                let base = ws.index.files.get(&f.path).map(|i| i.manifest_hash.clone()).or_else(|| ws.remote.files.get(&f.path).map(|r| r.manifest_hash.clone()));
                let shared = match (api, base) {
                    (Some(api), Some(base)) => {
                        if !base_parts.contains_key(&base) {
                            let parts = api.manifest(&ws.config.project_id, &base).await.ok().map(|bm| bm.chunks.into_iter().map(|c| c.hash).collect());
                            if parts.is_none() {
                                report.checked_server = false;
                            }
                            base_parts.insert(base.clone(), parts);
                        }
                        base_parts.get(&base).and_then(|p| p.as_ref())
                    }
                    _ => None,
                };
                for c in &m.chunks {
                    if !shared.map(|s| s.contains(&c.hash)).unwrap_or(false) {
                        needed.insert(c.hash.clone());
                    }
                }
            }
        }
        let used: u64 = needed.iter().filter_map(|h| cached.get(h)).map(|(_, n, _)| *n).sum();
        if used <= limit {
            break needed;
        }
        // Oldest first (the list is newest first). Checkpoints on the server use no space here.
        let local = || cps.iter().rev().filter(|c| !c.on_server);
        let victim = local().find(|c| c.auto).or_else(|| local().find(|c| c.system && chrono::DateTime::parse_from_rfc3339(&c.created_at).map(|t| t < week_ago).unwrap_or(false)));
        match victim {
            Some(v) => {
                std::fs::remove_file(checkpoints_dir(ws).join(format!("{}.json", v.id)))?;
                report.removed_checkpoints.push(v.id.clone());
            }
            None => break needed,
        }
    };

    for (hash, (path, size, recent)) in &cached {
        if needed.contains(hash) || *recent {
            report.kept_bytes += size;
            report.kept_parts += 1;
        } else if std::fs::remove_file(path).is_ok() {
            report.freed_bytes += size;
            report.freed_parts += 1;
        }
    }
    for p in junk {
        let _ = std::fs::remove_file(p);
    }

    // Files moved aside: keep a month, like a recycle bin.
    let cutoff = (chrono::Local::now() - chrono::Duration::days(SAFETY_DAYS)).format("%Y-%m-%d_%H%M%S").to_string();
    if let Ok(rd) = std::fs::read_dir(Workspace::mp_dir(&ws.root).join("safety")) {
        for e in rd.flatten() {
            let size: u64 = walkdir::WalkDir::new(e.path()).into_iter().flatten().filter(|f| f.file_type().is_file()).filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum();
            let name = e.file_name().to_string_lossy().to_string();
            // Folder names are timestamps, so they compare as text.
            if name.len() == cutoff.len() && name < cutoff && std::fs::remove_dir_all(e.path()).is_ok() {
                report.safety_freed_bytes += size;
            } else {
                report.safety_bytes += size;
            }
        }
    }
    report.free_disk_bytes = disk_free(ws);
    report.low_disk = report.free_disk_bytes < LOW_DISK;
    Ok(report)
}

/// Ask the server where this project keeps checkpoints and remember the answer.
pub async fn refresh_project_settings(api: &Api, ws: &mut Workspace) -> Result<bool> {
    let p = api.project(&ws.config.project_id).await?;
    let storage = Some(p.checkpoint_storage).filter(|s| !s.is_empty());
    if storage != ws.config.project_checkpoint_storage || p.auto_checkpoints != ws.config.project_auto_checkpoints {
        ws.config.project_checkpoint_storage = storage;
        ws.config.project_auto_checkpoints = p.auto_checkpoints;
        crate::workspace::write_json_atomic(&Workspace::mp_dir(&ws.root).join("workspace.json"), &ws.config)?;
        return Ok(true);
    }
    Ok(false)
}

// ---------------------------------------------------------------------------------------------
// Rewind

/// A checkpoint by id: from this folder, or (after a disk loss, or from another computer) from
/// the server.
pub async fn load_checkpoint(api: &Api, ws: &Workspace, id: &str) -> Result<Checkpoint> {
    if let Some(cp) = crate::workspace::read_json::<Checkpoint>(&checkpoints_dir(ws).join(format!("{id}.json")))? {
        return Ok(cp);
    }
    let v: serde_json::Value = api
        .get(&format!("/projects/{}/checkpoints/{id}", ws.config.project_id))
        .await
        .map_err(|_| anyhow::anyhow!("There is no checkpoint {id}. Run `mp checkpoints` to list them (`--server` for ones kept on the server)."))?;
    let entries: Vec<PublishEntry> = serde_json::from_value(v["entries"].clone())?;
    Ok(Checkpoint {
        id: id.to_string(),
        note: v["note"].as_str().map(str::to_string),
        auto: v["auto"].as_bool().unwrap_or(false),
        system: v["system"].as_bool().unwrap_or(false),
        on_server: true,
        created_at: v["created_at"].as_str().unwrap_or_default().to_string(),
        branch_id: v["branch_id"].as_str().unwrap_or_default().to_string(),
        files: entries.into_iter().map(|e| CheckpointFile { path: e.path, manifest: e.manifest, size: 0, mtime: 0 }).collect(),
    })
}

/// Put files back as they were at a checkpoint (all of its files, or those under `only`).
/// Takes a checkpoint of the current state first, so the rewind can be undone; changed files
/// that checkpoint does not cover are moved to `.mp/safety/` rather than overwritten.
pub async fn rewind(api: &Api, ws: &Workspace, id: &str, only: &[String]) -> Result<(Option<Checkpoint>, usize)> {
    let cp = load_checkpoint(api, ws, id).await?;
    let changed = scan::status(ws, &[])?.changed_paths();
    // Named after what was rewound to, as people see it.
    let note = match cp.note.as_deref() {
        Some(n) if !cp.system => format!("Before rewinding to \u{201c}{n}\u{201d}"),
        _ if cp.auto => "Before rewinding to an automatic checkpoint".to_string(),
        _ => format!("Before rewinding to {id}"),
    };
    let before = checkpoint(Some(api), ws, CheckpointOptions::undo(&note)).await?;
    let unprotected = before.unprotected(&changed);
    let s = stamp();
    let mut n = 0;
    for f in &cp.files {
        if !only.is_empty() && !only.iter().any(|o| mp_core::paths::within(&f.path, o)) {
            continue;
        }
        let exists = ws.abs(&f.path).exists();
        match &f.manifest {
            Some(m) => {
                if exists && unprotected.contains(&f.path) {
                    ws.move_to_safety(&f.path, &s)?;
                }
                write_from_manifest(api, ws, &f.path, m).await?;
            }
            None => {
                if exists {
                    ws.move_to_safety(&f.path, &s)?;
                }
            }
        }
        n += 1;
    }
    Ok((before.checkpoint, n))
}

// ---------------------------------------------------------------------------------------------
// The no-checkpoint list

/// Add a rule to the shared `.mpnocheckpoint` (published with the next change) or the personal
/// list in `.mp/nocheckpoint`.
pub fn add_no_checkpoint(ws: &Workspace, pattern: &str, personal: bool) -> Result<std::path::PathBuf> {
    let file = if personal { Workspace::mp_dir(&ws.root).join("nocheckpoint") } else { ws.root.join(scan::NO_CHECKPOINT_FILE) };
    let existing = std::fs::read_to_string(&file).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == pattern.trim()) {
        return Ok(file);
    }
    let mut text = existing;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(pattern.trim());
    text.push('\n');
    std::fs::write(&file, text)?;
    Ok(file)
}

/// Whether checkpoints leave this file out, and by which rule.
pub fn no_checkpoint_rule(ws: &Workspace, rel: &str) -> Result<Option<String>> {
    Ok(scan::Ignores::load_no_checkpoint(&ws.root)?.check(rel, false))
}
