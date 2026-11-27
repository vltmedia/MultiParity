//! Workspace operations. None of them delete or overwrite a file that differs from what the
//! server last gave this computer without moving it to `.mp/safety/` first.

use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use futures::{StreamExt, TryStreamExt};
use mp_core::chunk::{chunk_reader, ChunkParams};
use mp_core::Manifest;
use serde::Serialize;

use crate::accounts::Accounts;
use crate::api::Api;
use crate::progress;
use crate::scan::{self, Status};
use crate::types::*;
use crate::workspace::{mtime_of, IndexEntry, RemoteEntry, RemoteTree, Workspace, WorkspaceConfig};

const PARALLEL: usize = 6;

pub fn device_name() -> String {
    std::env::var("MP_DEVICE")
        .ok()
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "this computer".into())
}

pub fn api_for(ws: &Workspace) -> Result<Api> {
    let accounts = Accounts::load()?;
    Ok(Api::for_account(accounts.by_id(&ws.config.account_id)?))
}

pub(crate) fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

// ---------------------------------------------------------------------------------------------
// Connecting a folder

#[derive(Debug, Clone, Serialize, Default)]
pub struct ConnectReport {
    pub matched: usize,
    pub different: Vec<String>,
    pub local_only: usize,
    pub ignored: usize,
    pub to_download: usize,
    pub download_bytes: u64,
    pub downloaded: usize,
}

pub struct ConnectOptions<'a> {
    pub account_id: &'a str,
    pub project: &'a ProjectSummary,
    pub branch: &'a Branch,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub download: bool,
    pub dry_run: bool,
}

/// Connect a project to a folder, adopting whatever is already there. Files identical to the
/// server are marked up to date; different or local-only files are kept as they are.
pub async fn connect(api: &Api, root: &Path, opts: ConnectOptions<'_>) -> Result<(Option<Workspace>, ConnectReport)> {
    std::fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let root = std::fs::canonicalize(root)?;
    if Workspace::find(&root).is_ok() {
        anyhow::bail!("{} is already inside a connected folder. Choose another folder.", root.display());
    }
    for other in crate::workspace::Registry::workspaces()? {
        if root.starts_with(&other) || other.starts_with(&root) {
            anyhow::bail!("{} overlaps {}, which is already connected. Two projects cannot share or nest folders.", root.display(), other.display());
        }
    }
    let tree = api.tree(&opts.project.id, &opts.branch.id, 0).await?;
    let accounts = Accounts::load()?;
    let account = accounts.by_id(opts.account_id)?;
    let config = WorkspaceConfig {
        id: uuid::Uuid::new_v4().to_string(),
        server: api.server.clone(),
        account_id: account.id.clone(),
        account_email: account.email.clone(),
        project_id: opts.project.id.clone(),
        project_name: opts.project.name.clone(),
        org_name: opts.project.org.name.clone(),
        branch_id: opts.branch.id.clone(),
        branch_name: opts.branch.name.clone(),
        synced_change: tree.head,
        include: opts.include.clone(),
        exclude: opts.exclude.clone(),
        auto_claim: true,
        auto_checkpoints: None,
        project_auto_checkpoints: opts.project.auto_checkpoints,
        checkpoint_limit_mb: None,
        checkpoint_storage: None,
        project_checkpoint_storage: Some(opts.project.checkpoint_storage.clone()).filter(|s| !s.is_empty()),
    };
    let mut ws = Workspace { root: root.clone(), config, index: Default::default(), remote: remote_from(&tree) };

    let mut report = ConnectReport::default();
    let mut downloads = Vec::new();
    for (path, r) in &ws.remote.files {
        if !ws.in_scope(path) {
            continue;
        }
        let abs = ws.abs(path);
        match std::fs::metadata(&abs) {
            Ok(meta) if meta.is_file() => {
                let (hash, size) = scan::hash_file(&abs)?;
                if hash == r.file_hash {
                    report.matched += 1;
                    ws.index.files.insert(path.clone(), IndexEntry { file_hash: hash, manifest_hash: r.manifest_hash.clone(), size, mtime: mtime_of(&meta), change: r.change });
                } else {
                    // Kept as is. Indexing the server's version (with a zero mtime so the next
                    // scan hashes it) makes it show as Edited rather than New.
                    report.different.push(path.clone());
                    ws.index.files.insert(path.clone(), IndexEntry { file_hash: r.file_hash.clone(), manifest_hash: r.manifest_hash.clone(), size: r.size, mtime: 0, change: r.change });
                }
            }
            _ => {
                report.to_download += 1;
                report.download_bytes += r.size;
                downloads.push(path.clone());
            }
        }
    }
    // Local-only and ignored counts, from a scan against the new index.
    let st = scan::status(&ws, &[])?;
    report.local_only = st.new.len();
    report.ignored = st.ignored.len();
    if opts.dry_run {
        return Ok((None, report));
    }
    let ws_saved = Workspace::create(&root, ws.config.clone())?;
    drop(ws_saved);
    ws.save()?;
    if opts.download {
        report.downloaded = download_paths(api, &mut ws, &downloads, |_| {}).await?;
        ws.save()?;
    }
    Ok((Some(ws), report))
}

fn remote_from(tree: &TreeDelta) -> RemoteTree {
    let mut r = RemoteTree { head: tree.head, files: Default::default() };
    apply_delta(&mut r, tree);
    r
}

fn apply_delta(r: &mut RemoteTree, tree: &TreeDelta) {
    r.head = r.head.max(tree.head);
    for e in &tree.entries {
        match (&e.file_hash, &e.manifest_hash) {
            (Some(f), Some(m)) => {
                r.files.insert(e.path.clone(), RemoteEntry { file_hash: f.clone(), manifest_hash: m.clone(), size: e.size as u64, change: e.change });
            }
            _ => {
                r.files.remove(&e.path);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Downloading

/// Fetch a manifest's chunks (local cache first) and write the file atomically, checking its
/// hash. Returns the written file's metadata. Downloaded parts are not kept in the cache: the
/// file itself is the copy, and the server has them.
pub async fn fetch_file(api: &Api, ws: &Workspace, rel: &str, manifest_hash: &str) -> Result<std::fs::Metadata> {
    let m = api.manifest(&ws.config.project_id, manifest_hash).await?;
    write_from_manifest(api, ws, rel, &m).await
}

pub async fn write_from_manifest(api: &Api, ws: &Workspace, rel: &str, m: &Manifest) -> Result<std::fs::Metadata> {
    write_manifest_to(api, ws, rel, &ws.abs(rel), m).await
}

/// Like `write_from_manifest`, to any path (`rel` names the file in messages).
pub async fn write_manifest_to(api: &Api, ws: &Workspace, rel: &str, dest: &Path, m: &Manifest) -> Result<std::fs::Metadata> {
    let tmp = ws.tmp_dir()?.join(format!("dl-{}", uuid::Uuid::new_v4()));
    let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    let mut hasher = blake3::Hasher::new();
    let project = ws.config.project_id.clone();
    let mut stream = futures::stream::iter(m.chunks.clone())
        .map(|c| {
            let api = api.clone();
            let project = project.clone();
            let cached = ws.cached_chunk(&c.hash);
            async move {
                let comp = match std::fs::read(&cached) {
                    Ok(b) => b,
                    Err(_) => api.get_chunk(&project, &c.hash).await?.to_vec(),
                };
                let raw = mp_core::decompress(&comp, c.len as usize)?;
                anyhow::ensure!(mp_core::hash_bytes(&raw) == c.hash, "a downloaded part of {rel} was corrupted; try again");
                Ok::<_, anyhow::Error>(raw)
            }
        })
        .buffered(PARALLEL);
    while let Some(raw) = stream.try_next().await? {
        hasher.update(&raw);
        f.write_all(&raw)?;
        progress::bytes(raw.len() as u64);
    }
    f.flush()?;
    drop(f);
    let got = hasher.finalize().to_hex().to_string();
    if got != m.file_hash {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!("{rel} did not match its expected content after downloading. Try again; if it keeps happening, ask an admin to run a storage check.");
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let moved = std::fs::rename(&tmp, dest).or_else(|e| {
        // Another drive: copy instead.
        if crosses_devices(&e) {
            std::fs::copy(&tmp, dest).map(|_| ()).and_then(|_| std::fs::remove_file(&tmp))
        } else {
            Err(e)
        }
    });
    if let Err(e) = moved {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {} (is it open in another app?)", dest.display()));
    }
    Ok(std::fs::metadata(dest)?)
}

// ---------------------------------------------------------------------------------------------
// Older versions

pub async fn versions(api: &Api, ws: &Workspace, rel: &str) -> Result<Vec<FileVersion>> {
    api.versions(&ws.config.project_id, &ws.config.branch_id, rel).await
}

async fn version_manifest(api: &Api, ws: &Workspace, rel: &str, change: i64) -> Result<Manifest> {
    let list = versions(api, ws, rel).await?;
    let v = list.iter().find(|v| v.change == change).with_context(|| format!("{rel} has no version in change {change}. `mp versions {rel}` lists them."))?;
    let mh = v.manifest_hash.as_deref().with_context(|| format!("Change {change} deleted {rel}, so there is no file in it to get. Pick an earlier version."))?;
    api.manifest(&ws.config.project_id, mh).await
}

/// Where a saved copy of an older version goes by default: beside the file, as
/// `name (change 12).ext`.
pub fn version_copy_path(ws: &Workspace, rel: &str, change: i64) -> std::path::PathBuf {
    let abs = ws.abs(rel);
    let stem = abs.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let name = match abs.extension() {
        Some(ext) => format!("{stem} (change {change}).{}", ext.to_string_lossy()),
        None => format!("{stem} (change {change})"),
    };
    abs.with_file_name(name)
}

/// Save a copy of an older version somewhere (by default beside the file), leaving the file
/// itself alone. A copy saved inside the folder shows up as a new file; it is meant for looking
/// at, so delete it or ignore it rather than publishing it.
pub async fn save_version_copy(api: &Api, ws: &Workspace, rel: &str, change: i64, to: Option<&Path>) -> Result<std::path::PathBuf> {
    let m = version_manifest(api, ws, rel, change).await?;
    let dest = to.map(Path::to_path_buf).unwrap_or_else(|| version_copy_path(ws, rel, change));
    write_manifest_to(api, ws, rel, &dest, &m).await?;
    Ok(dest)
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BringBackReport {
    pub path: String,
    pub change: i64,
    /// The checkpoint taken first, holding your version if you had changed the file.
    pub checkpoint: Option<Checkpoint>,
}

/// Put an older version into the folder in place of the file. It then shows as edited: publish
/// it to make it the latest for everyone. Your own unpublished edits are kept in a checkpoint
/// first (or moved to `.mp/safety/` if checkpoints cannot hold them).
pub async fn bring_back_version(api: &Api, ws: &Workspace, rel: &str, change: i64) -> Result<BringBackReport> {
    let m = version_manifest(api, ws, rel, change).await?;
    let changed = scan::status(ws, &[rel.to_string()])?.changed_paths();
    let mut report = BringBackReport { path: rel.to_string(), change, checkpoint: None };
    if changed.iter().any(|p| p == rel) {
        let note = format!("Before bringing back change {change} of {}", rel.rsplit('/').next().unwrap_or(rel));
        let before = checkpoint(Some(api), ws, CheckpointOptions::undo(&note)).await?;
        if before.unprotected(&changed).contains(rel) && ws.abs(rel).exists() {
            ws.move_to_safety(rel, &stamp())?;
        }
        report.checkpoint = before.checkpoint;
    }
    write_from_manifest(api, ws, rel, &m).await?;
    Ok(report)
}

/// Download these remote paths into the folder, updating the index.
pub async fn download_paths(api: &Api, ws: &mut Workspace, paths: &[String], mut progress: impl FnMut(&str)) -> Result<usize> {
    let mut n = 0;
    let total: u64 = paths.iter().filter_map(|p| ws.remote.files.get(p)).map(|r| r.size).sum();
    progress::phase("Downloading", paths.len() as u64, total);
    for p in paths {
        let Some(r) = ws.remote.files.get(p).cloned() else { continue };
        progress(p);
        progress::file(p);
        let meta = fetch_file(api, ws, p, &r.manifest_hash).await?;
        progress::file_done();
        ws.index.files.insert(p.clone(), IndexEntry { file_hash: r.file_hash, manifest_hash: r.manifest_hash, size: meta.len(), mtime: mtime_of(&meta), change: r.change });
        n += 1;
        if n % 50 == 0 {
            ws.save()?;
        }
    }
    Ok(n)
}

// ---------------------------------------------------------------------------------------------
// Get latest

#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncReport {
    pub head: i64,
    pub downloaded: Vec<String>,
    pub removed: Vec<String>,
    /// Changed on the server and on this computer: kept as is.
    pub kept_yours: Vec<String>,
    pub bytes: u64,
}

pub async fn get_latest(api: &Api, ws: &mut Workspace, mut progress: impl FnMut(&str)) -> Result<SyncReport> {
    progress::phase("Checking for newer files", 0, 0);
    let delta = api.tree(&ws.config.project_id, &ws.config.branch_id, ws.config.synced_change).await?;
    apply_delta(&mut ws.remote, &delta);
    let mut report = SyncReport { head: delta.head, ..Default::default() };
    let todo: Vec<&TreeEntry> = delta.entries.iter().filter(|e| ws.in_scope(&e.path)).collect();
    let bytes: u64 = todo.iter().filter(|e| e.manifest_hash.is_some()).map(|e| e.size as u64).sum();
    progress::phase("Downloading", todo.len() as u64, bytes);
    for e in &delta.entries {
        if !ws.in_scope(&e.path) {
            continue;
        }
        report_apply(api, ws, e, &mut report, &mut progress).await?;
    }
    ws.config.synced_change = delta.head;
    ws.save()?;
    if !report.downloaded.is_empty() || !report.removed.is_empty() {
        let _ = crate::plugins::hook("after_get_latest", ws, serde_json::json!({ "change": report.head, "downloaded": report.downloaded, "removed": report.removed })).await;
    }
    Ok(report)
}

/// Bring one path to the server's version unless the local copy has unpublished edits.
async fn report_apply(api: &Api, ws: &mut Workspace, e: &TreeEntry, report: &mut SyncReport, progress: &mut impl FnMut(&str)) -> Result<()> {
    let abs = ws.abs(&e.path);
    let local = std::fs::metadata(&abs).ok().filter(|m| m.is_file());
    let indexed = ws.index.files.get(&e.path).cloned();
    let local_hash = match (&local, &indexed) {
        (Some(m), Some(ix)) if m.len() == ix.size && mtime_of(m) == ix.mtime => Some(ix.file_hash.clone()),
        (Some(_), _) => Some(scan::hash_file(&abs)?.0),
        (None, _) => None,
    };
    let unchanged_locally = local_hash.as_deref() == indexed.as_ref().map(|i| i.file_hash.as_str());
    match (&e.file_hash, &e.manifest_hash) {
        (Some(fh), Some(mh)) => {
            if local_hash.as_deref() == Some(fh.as_str()) {
                progress::skip(e.size as u64);
                let m = local.unwrap();
                ws.index.files.insert(e.path.clone(), IndexEntry { file_hash: fh.clone(), manifest_hash: mh.clone(), size: m.len(), mtime: mtime_of(&m), change: e.change });
            } else if unchanged_locally {
                progress(&e.path);
                progress::file(&e.path);
                let m = fetch_file(api, ws, &e.path, mh).await?;
                progress::file_done();
                ws.index.files.insert(e.path.clone(), IndexEntry { file_hash: fh.clone(), manifest_hash: mh.clone(), size: m.len(), mtime: mtime_of(&m), change: e.change });
                report.downloaded.push(e.path.clone());
                report.bytes += e.size as u64;
            } else {
                progress::skip(e.size as u64);
                report.kept_yours.push(e.path.clone());
            }
        }
        _ => {
            progress::skip(0);
            if local.is_none() {
                ws.index.files.remove(&e.path);
            } else if unchanged_locally && indexed.is_some() {
                std::fs::remove_file(&abs)?;
                remove_empty_parents(ws, &e.path);
                ws.index.files.remove(&e.path);
                report.removed.push(e.path.clone());
            } else {
                report.kept_yours.push(e.path.clone());
            }
        }
    }
    Ok(())
}

fn remove_empty_parents(ws: &Workspace, rel: &str) {
    let mut p = ws.abs(rel);
    while p.pop() && p != ws.root {
        if std::fs::remove_dir(&p).is_err() {
            break;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Chunking local files

/// Work out a file's manifest without storing anything.
pub fn manifest_of(ws: &Workspace, rel: &str) -> Result<Manifest> {
    let f = std::fs::File::open(ws.abs(rel)).with_context(|| format!("reading {rel}"))?;
    chunk_reader(std::io::BufReader::with_capacity(1 << 20, f), ChunkParams::default(), |c| {
        progress::bytes(c.data.len() as u64);
        Ok(())
    })
}

/// Where to read one part from when uploading: a range of a file in the folder.
#[derive(Clone)]
pub(crate) struct PartSource {
    pub path: String,
    pub offset: u64,
    pub len: u64,
}

/// Read one part straight from its file and compress it, checking it is still what was chunked.
pub(crate) fn read_part(ws: &Workspace, hash: &str, src: &PartSource) -> Result<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(ws.abs(&src.path)).with_context(|| format!("reading {}", src.path))?;
    f.seek(SeekFrom::Start(src.offset))?;
    let mut raw = vec![0u8; src.len as usize];
    f.read_exact(&mut raw).with_context(|| format!("{} changed while it was being published. Let it finish saving, then publish again.", src.path))?;
    anyhow::ensure!(
        mp_core::hash_bytes(&raw) == hash,
        "{} changed while it was being published. Let it finish saving, then publish again.",
        src.path
    );
    mp_core::compress(&raw)
}

/// Upload any parts of these files (path → manifest) the server does not have yet. Parts are
/// read straight from the files, so publishing needs no extra disk space.
pub async fn upload_missing(api: &Api, ws: &Workspace, files: &BTreeMap<String, Manifest>, mut progress: impl FnMut(usize, usize)) -> Result<(usize, u64)> {
    let mut sources: HashMap<String, PartSource> = HashMap::new();
    for (path, m) in files {
        let mut offset = 0u64;
        for c in &m.chunks {
            sources.entry(c.hash.clone()).or_insert_with(|| PartSource { path: path.clone(), offset, len: c.len });
            offset += c.len;
        }
    }
    let hashes: Vec<String> = sources.keys().cloned().collect();
    let missing = api.missing_chunks(&ws.config.project_id, &hashes).await?;
    let total = missing.len();
    let send_bytes: u64 = missing.iter().filter_map(|h| sources.get(h)).map(|s| s.len).sum();
    progress::phase("Sending", 0, send_bytes);
    let project = ws.config.project_id.clone();
    let mut done = 0;
    let mut bytes = 0u64;
    let mut stream = futures::stream::iter(missing)
        .map(|h| {
            let api = api.clone();
            let project = project.clone();
            let src = sources.get(&h).cloned();
            async move {
                let src = src.ok_or_else(|| anyhow::anyhow!("the server asked for a part ({}) that is not in these files", &h[..12.min(h.len())]))?;
                let data = read_part(ws, &h, &src)?;
                let n = data.len() as u64;
                api.put_chunk(&project, &h, data).await?;
                progress::bytes(src.len);
                Ok::<_, anyhow::Error>(n)
            }
        })
        .buffer_unordered(PARALLEL);
    while let Some(n) = stream.try_next().await? {
        done += 1;
        bytes += n;
        progress(done, total);
    }
    Ok((total, bytes))
}

// ---------------------------------------------------------------------------------------------
// Publish

#[derive(Debug, Clone, Serialize)]
pub struct PublishReport {
    pub change: Change,
    pub files: usize,
    pub uploaded_chunks: usize,
    pub uploaded_bytes: u64,
}

/// Publish new, edited and deleted files (all of them, or those under `only`).
///
/// `keep_mine`: paths that changed on the server too, where this person has seen the newer
/// version (after get latest) and chooses to publish theirs over it.
pub async fn publish(api: &Api, ws: &mut Workspace, message: &str, only: &[String], keep_mine: &[String], mut progress: impl FnMut(&str)) -> Result<PublishReport> {
    let st = scan::status(ws, only)?;
    if st.is_clean() {
        anyhow::bail!("Nothing to publish: no new, edited or deleted files{}.", if only.is_empty() { "" } else { " in the paths you named" });
    }
    // Plugins with a before_publish hook can stop it (a naming rule, a missing texture…).
    let files: Vec<serde_json::Value> = st
        .new
        .iter()
        .map(|f| (f, "add"))
        .chain(st.edited.iter().map(|f| (f, "edit")))
        .chain(st.deleted.iter().map(|f| (f, "delete")))
        .map(|(f, op)| serde_json::json!({ "path": f.path, "abs": ws.abs(&f.path).display().to_string().trim_start_matches(r"\\?\"), "op": op, "size": f.size }))
        .collect();
    crate::plugins::hook("before_publish", ws, serde_json::json!({ "message": message, "files": files })).await?;
    let (mut entries, manifests) = build_entries(ws, &st, &mut progress)?;
    for e in entries.iter_mut() {
        if keep_mine.iter().any(|k| mp_core::paths::within(&e.path, k)) {
            e.prev_file_hash = ws.remote.files.get(&e.path).map(|r| r.file_hash.clone());
        }
    }
    progress("Sending file data");
    let (uploaded_chunks, uploaded_bytes) = upload_missing(api, ws, &manifests, |_, _| {}).await?;
    progress("Publishing");
    progress::phase("Publishing", 0, 0);
    let change = api.publish(&ws.config.project_id, &ws.config.branch_id, message, &entries).await?;
    for e in &entries {
        if e.op == "delete" {
            ws.index.files.remove(&e.path);
            ws.remote.files.remove(&e.path);
        } else if let Some(m) = &e.manifest {
            let meta = std::fs::metadata(ws.abs(&e.path))?;
            ws.index.files.insert(e.path.clone(), IndexEntry { file_hash: m.file_hash.clone(), manifest_hash: m.hash.clone(), size: m.size, mtime: mtime_of(&meta), change: change.number });
            ws.remote.files.insert(e.path.clone(), RemoteEntry { file_hash: m.file_hash.clone(), manifest_hash: m.hash.clone(), size: m.size, change: change.number });
        }
    }
    // Our own change is already applied; nothing in between was skipped if we were current.
    if ws.config.synced_change == change.number - 1 {
        ws.config.synced_change = change.number;
    }
    ws.save()?;
    // Checkpoint parts that are now on the server no longer need a copy here.
    let _ = trim_cache(Some(api), ws).await;
    let paths: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let _ = crate::plugins::hook("after_publish", ws, serde_json::json!({ "change": change.number, "message": message, "paths": paths })).await;
    Ok(PublishReport { change, files: entries.len(), uploaded_chunks, uploaded_bytes })
}

pub fn build_entries(ws: &Workspace, st: &Status, progress: &mut impl FnMut(&str)) -> Result<(Vec<PublishEntry>, BTreeMap<String, Manifest>)> {
    let mut entries = Vec::new();
    let mut manifests = BTreeMap::new();
    let n = (st.new.len() + st.edited.len()) as u64;
    progress::phase("Checking your files", n, st.new.iter().chain(&st.edited).map(|f| f.size).sum());
    for f in st.new.iter().chain(&st.edited) {
        progress(&f.path);
        progress::file(&f.path);
        let m = manifest_of(ws, &f.path)?;
        progress::file_done();
        entries.push(PublishEntry {
            path: f.path.clone(),
            op: "put".into(),
            prev_file_hash: ws.index.files.get(&f.path).map(|i| i.file_hash.clone()),
            manifest: Some(m.clone()),
        });
        manifests.insert(f.path.clone(), m);
    }
    for f in &st.deleted {
        entries.push(PublishEntry { path: f.path.clone(), op: "delete".into(), prev_file_hash: ws.index.files.get(&f.path).map(|i| i.file_hash.clone()), manifest: None });
    }
    Ok((entries, manifests))
}

pub use crate::checkpoints::{checkpoint, list_checkpoints, rewind, trim_cache, CacheReport, Checkpoint, CheckpointFile, CheckpointKind, CheckpointOptions, CheckpointOutcome};

// ---------------------------------------------------------------------------------------------
// Discard

/// Throw away local edits: edited and deleted files become the latest server version this
/// computer knows of, new files move to `.mp/safety/`. The current state is checkpointed first.
pub async fn discard(api: &Api, ws: &mut Workspace, paths: &[String]) -> Result<(Option<Checkpoint>, usize)> {
    let st = scan::status(ws, paths)?;
    if st.is_clean() {
        return Ok((None, 0));
    }
    let before = checkpoint(Some(api), ws, CheckpointOptions::undo("Before discarding")).await?;
    // Edited files the undo checkpoint does not cover (render output, or too big to copy) are
    // moved aside instead of being overwritten.
    let unprotected = before.unprotected(&st.changed_paths());
    let before = before.checkpoint;
    let s = stamp();
    let mut n = 0;
    for f in &st.new {
        ws.move_to_safety(&f.path, &s)?;
        n += 1;
    }
    for f in st.edited.iter().chain(&st.deleted) {
        match ws.remote.files.get(&f.path).cloned() {
            Some(r) => {
                if unprotected.contains(&f.path) && ws.abs(&f.path).exists() {
                    ws.move_to_safety(&f.path, &s)?;
                }
                let meta = fetch_file(api, ws, &f.path, &r.manifest_hash).await?;
                ws.index.files.insert(f.path.clone(), IndexEntry { file_hash: r.file_hash, manifest_hash: r.manifest_hash, size: meta.len(), mtime: mtime_of(&meta), change: r.change });
            }
            None => {
                // Deleted on the server since: keep the local copy aside, stop tracking it.
                if ws.abs(&f.path).exists() {
                    ws.move_to_safety(&f.path, &s)?;
                }
                ws.index.files.remove(&f.path);
            }
        }
        n += 1;
    }
    ws.save()?;
    Ok((before, n))
}

// ---------------------------------------------------------------------------------------------
// Drafts

pub async fn save_draft(api: &Api, ws: &Workspace, title: &str) -> Result<serde_json::Value> {
    let st = scan::status(ws, &[])?;
    if st.is_clean() {
        anyhow::bail!("Nothing to save: no new, edited or deleted files.");
    }
    let (entries, manifests) = build_entries(ws, &st, &mut |_| {})?;
    upload_missing(api, ws, &manifests, |_, _| {}).await?;
    api.post(&format!("/projects/{}/drafts", ws.config.project_id), &serde_json::json!({ "branch": ws.config.branch_id, "title": title, "entries": entries })).await
}

/// Put a draft's files into the folder. The current state is checkpointed first.
pub async fn restore_draft(api: &Api, ws: &Workspace, id: &str) -> Result<usize> {
    let d: serde_json::Value = api.get(&format!("/projects/{}/drafts/{id}", ws.config.project_id)).await?;
    let entries: Vec<PublishEntry> = serde_json::from_value(d["entries"].clone())?;
    let changed = scan::status(ws, &[])?.changed_paths();
    let unprotected = checkpoint(Some(api), ws, CheckpointOptions::undo("Before picking up a draft")).await?.unprotected(&changed);
    let s = stamp();
    for e in &entries {
        match &e.manifest {
            Some(m) => {
                if unprotected.contains(&e.path) && ws.abs(&e.path).exists() {
                    ws.move_to_safety(&e.path, &s)?;
                }
                write_from_manifest(api, ws, &e.path, m).await?;
            }
            None => {
                if ws.abs(&e.path).exists() {
                    ws.move_to_safety(&e.path, &s)?;
                }
            }
        }
    }
    Ok(entries.len())
}

// ---------------------------------------------------------------------------------------------
// Branches

/// Switch this folder to another branch. Refuses while there are unpublished changes (save a
/// draft or publish first) so nothing is mixed between branches.
pub async fn switch_branch(api: &Api, ws: &mut Workspace, branch: &Branch, mut progress: impl FnMut(&str)) -> Result<SyncReport> {
    let st = scan::status(ws, &[])?;
    if !st.is_clean() {
        anyhow::bail!(
            "You have {} unpublished file(s) on {}. Publish them or save a draft before switching branches.",
            st.new.len() + st.edited.len() + st.deleted.len(),
            ws.config.branch_name
        );
    }
    let tree = api.tree(&ws.config.project_id, &branch.id, 0).await?;
    let new_remote = remote_from(&tree);
    let mut report = SyncReport { head: tree.head, ..Default::default() };
    // Paths that differ between the two branches.
    let mut paths: Vec<String> = new_remote.files.keys().chain(ws.remote.files.keys()).cloned().collect();
    paths.sort();
    paths.dedup();
    for path in paths {
        if !ws.in_scope(&path) {
            continue;
        }
        let new = new_remote.files.get(&path);
        let cur = ws.index.files.get(&path).map(|i| i.file_hash.clone());
        if new.map(|n| &n.file_hash) == cur.as_ref() {
            if let (Some(n), Some(ix)) = (new, ws.index.files.get_mut(&path)) {
                ix.change = n.change;
            }
            continue;
        }
        let entry = TreeEntry {
            path: path.clone(),
            file_hash: new.map(|n| n.file_hash.clone()),
            manifest_hash: new.map(|n| n.manifest_hash.clone()),
            size: new.map(|n| n.size as i64).unwrap_or(0),
            change: new.map(|n| n.change).unwrap_or(0),
        };
        report_apply(api, ws, &entry, &mut report, &mut progress).await?;
    }
    ws.remote = new_remote;
    ws.config.branch_id = branch.id.clone();
    ws.config.branch_name = branch.name.clone();
    ws.config.synced_change = tree.head;
    ws.save()?;
    Ok(report)
}

// ---------------------------------------------------------------------------------------------
// Selective sync

#[derive(Debug, Clone, Serialize, Default)]
pub struct ScopeReport {
    pub downloaded: usize,
    pub removed: usize,
    /// Left in place because they have unpublished edits or are new.
    pub kept: Vec<String>,
}

/// Change which folders are on this computer. Newly included files are downloaded; files that
/// leave the scope are removed only when they are identical to the server.
pub async fn set_scope(api: &Api, ws: &mut Workspace, include: Vec<String>, exclude: Vec<String>) -> Result<ScopeReport> {
    let clean = |v: Vec<String>| -> Vec<String> {
        let mut v: Vec<String> = v.into_iter().filter_map(|x| if x.trim_matches('/').is_empty() { None } else { mp_core::paths::normalize(&x) }).collect();
        v.sort();
        v.dedup();
        v
    };
    let (include, exclude) = (clean(include), clean(exclude));
    let old_include = std::mem::replace(&mut ws.config.include, include);
    let old_exclude = std::mem::replace(&mut ws.config.exclude, exclude);
    let was_in = |p: &str| mp_core::paths::in_scope(p, &old_include, &old_exclude);
    let mut report = ScopeReport::default();

    // Leaving the scope.
    let leaving: Vec<String> = ws.index.files.keys().filter(|p| was_in(p) && !ws.in_scope(p)).cloned().collect();
    for p in leaving {
        let abs = ws.abs(&p);
        let ix = ws.index.files[&p].clone();
        match std::fs::metadata(&abs) {
            Ok(m) => {
                let same = (m.len() == ix.size && mtime_of(&m) == ix.mtime) || scan::hash_file(&abs)?.0 == ix.file_hash;
                if same {
                    std::fs::remove_file(&abs)?;
                    remove_empty_parents(ws, &p);
                    ws.index.files.remove(&p);
                    report.removed += 1;
                } else {
                    report.kept.push(p);
                }
            }
            Err(_) => {
                ws.index.files.remove(&p);
            }
        }
    }
    // Entering the scope: fetch the latest tree first so we download current versions.
    let delta = api.tree(&ws.config.project_id, &ws.config.branch_id, ws.config.synced_change).await?;
    apply_delta(&mut ws.remote, &delta);
    let entering: Vec<String> = ws.remote.files.keys().filter(|p| ws.in_scope(p) && !ws.index.files.contains_key(*p) && !ws.abs(p).exists()).cloned().collect();
    report.downloaded = download_paths(api, ws, &entering, |_| {}).await?;
    ws.save()?;
    Ok(report)
}

// ---------------------------------------------------------------------------------------------
// Moving a synced folder

#[derive(Debug, Clone, Serialize)]
pub struct MoveReport {
    pub from: String,
    pub to: String,
    /// "renamed" (same drive: instant) or "copied" (another drive).
    pub method: &'static str,
    pub files: u64,
    pub bytes: u64,
    /// Set when the copy succeeded but the old folder could not be fully removed.
    pub old_folder_left: Option<String>,
}

fn crosses_devices(e: &std::io::Error) -> bool {
    // EXDEV on Linux and macOS (18), ERROR_NOT_SAME_DEVICE on Windows (17).
    e.kind() == std::io::ErrorKind::CrossesDevices || matches!(e.raw_os_error(), Some(18) if cfg!(unix)) || matches!(e.raw_os_error(), Some(17) if cfg!(windows))
}

/// Copy a folder tree, keeping modification times so nothing looks edited afterwards.
fn copy_tree(from: &Path, to: &Path, progress: &mut impl FnMut(u64, u64)) -> Result<(u64, u64)> {
    let total: u64 = walkdir::WalkDir::new(from).into_iter().flatten().filter(|e| e.file_type().is_file()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum();
    let (mut files, mut bytes) = (0u64, 0u64);
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(from)?;
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            let n = std::fs::copy(entry.path(), &dest).with_context(|| format!("copying {}", entry.path().display()))?;
            if let Some(t) = entry.metadata().ok().and_then(|m| m.modified().ok()) {
                let _ = std::fs::File::options().write(true).open(&dest).and_then(|f| f.set_modified(t));
            }
            files += 1;
            bytes += n;
            progress(bytes, total);
        }
    }
    anyhow::ensure!(bytes == total, "the copy came out at {bytes} bytes instead of {total}; the old folder was left as it was");
    Ok((files, bytes))
}

/// Move a synced folder to a new place on this computer. On the same drive it is renamed, which
/// is instant; to another drive everything is copied (modification times kept, so nothing needs
/// downloading or reading again), checked, and then the old folder is removed.
pub fn move_folder(ws: &Workspace, to: &Path, mut progress: impl FnMut(u64, u64)) -> Result<MoveReport> {
    let from = ws.root.clone();
    let to = std::path::absolute(to).with_context(|| format!("{} is not a usable folder path", to.display()))?;
    let to_cmp = std::fs::canonicalize(to.parent().unwrap_or(&to)).map(|p| p.join(to.file_name().unwrap_or_default())).unwrap_or_else(|_| to.clone());
    anyhow::ensure!(to_cmp != from, "That is where the folder already is.");
    anyhow::ensure!(!to_cmp.starts_with(&from), "The new place is inside the folder itself. Choose a place outside it.");
    for other in crate::workspace::Registry::workspaces()? {
        if other != from && (to_cmp.starts_with(&other) || other.starts_with(&to_cmp)) {
            anyhow::bail!("{} overlaps {}, which is another synced project. Choose another place.", to.display(), other.display());
        }
    }
    if to.exists() {
        let empty = to.is_dir() && std::fs::read_dir(&to)?.next().is_none();
        anyhow::ensure!(empty, "{} already has files in it. Choose a new or empty folder.", to.display());
        std::fs::remove_dir(&to)?;
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let report = match std::fs::rename(&from, &to) {
        Ok(()) => {
            let files = walkdir::WalkDir::new(&to).into_iter().flatten().filter(|e| e.file_type().is_file()).count() as u64;
            let bytes = ws.index.files.values().map(|i| i.size).sum();
            MoveReport { from: from.display().to_string(), to: to.display().to_string(), method: "renamed", files, bytes, old_folder_left: None }
        }
        Err(e) if crosses_devices(&e) => {
            let (files, bytes) = match copy_tree(&from, &to, &mut progress) {
                Ok(r) => r,
                Err(err) => {
                    let _ = std::fs::remove_dir_all(&to);
                    return Err(err);
                }
            };
            let left = std::fs::remove_dir_all(&from).err().map(|e| format!("{} could not be fully removed ({e}). Everything was copied; delete it yourself once nothing is using it.", from.display()));
            MoveReport { from: from.display().to_string(), to: to.display().to_string(), method: "copied", files, bytes, old_folder_left: left }
        }
        Err(e) => anyhow::bail!("Could not move {}: {e}. Close any app that has files from it open, then try again.", from.display()),
    };
    crate::workspace::Registry::remove(&from)?;
    crate::workspace::Registry::add(&to)?;
    Ok(report)
}

/// Folder tree with sizes for the selective sync screen.
#[derive(Debug, Clone, Serialize)]
pub struct FolderNode {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub local_size: u64,
    pub included: &'static str,
    pub children: Vec<FolderNode>,
}

pub fn folder_tree(ws: &Workspace) -> FolderNode {
    #[derive(Default)]
    struct Acc {
        size: u64,
        local: u64,
        files_in: usize,
        files: usize,
        children: BTreeMap<String, Acc>,
    }
    let mut root = Acc::default();
    for (path, r) in &ws.remote.files {
        let on = ws.index.files.contains_key(path);
        let inc = ws.in_scope(path);
        let parts: Vec<&str> = path.split('/').collect();
        let mut node = &mut root;
        for (i, part) in parts.iter().enumerate() {
            node.size += r.size;
            node.files += 1;
            node.files_in += usize::from(inc);
            if on {
                node.local += r.size;
            }
            if i + 1 == parts.len() {
                break;
            }
            node = node.children.entry(part.to_string()).or_default();
        }
    }
    fn build(name: &str, path: &str, a: &Acc) -> FolderNode {
        FolderNode {
            path: path.to_string(),
            name: name.to_string(),
            size: a.size,
            local_size: a.local,
            included: if a.files_in == 0 { "none" } else if a.files_in == a.files { "all" } else { "some" },
            children: a.children.iter().map(|(n, c)| build(n, &if path.is_empty() { n.clone() } else { format!("{path}/{n}") }, c)).collect(),
        }
    }
    build(&ws.config.project_name, "", &root)
}

// ---------------------------------------------------------------------------------------------
// Ignore list

/// Add a rule to the shared `.mpignore` (published with the next change) or the personal list.
pub fn add_ignore(ws: &Workspace, pattern: &str, personal: bool) -> Result<std::path::PathBuf> {
    let file = if personal { Workspace::mp_dir(&ws.root).join("ignore") } else { ws.root.join(".mpignore") };
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

pub const STARTER_LISTS: &[(&str, &[&str])] = &[
    ("unreal", &["Binaries/", "Intermediate/", "Saved/", "DerivedDataCache/", "Build/", ".vs/", "*.sln", "*.VC.db", "*.opensdf", "*.sdf", "*.suo"]),
    ("unity", &["Library/", "Temp/", "Obj/", "Build/", "Builds/", "Logs/", "UserSettings/", "MemoryCaptures/", "*.csproj", "*.sln", "*.pidb.meta", "*.pdb.meta", "*.mdb.meta"]),
    ("godot", &[".godot/", ".import/", "export.cfg", "export_presets.cfg", "*.translation", "mono_crash.*.json"]),
    ("blender", &["*.blend1", "*.blend2", "*.blend@", "__pycache__/"]),
    ("maya", &["*.swatches", "incrementalSave/", "workspace.mel.bak", "*.mayaSwatches"]),
    ("houdini", &["backup/", "*.hip_bak*", "*.hiplc_bak*", "*_bak*.hip", "*.sim", "*.simdata"]),
    ("photoshop", &["*.psd.tmp", "~*.psd", "*.psb.tmp"]),
    ("os", &[".DS_Store", "Thumbs.db", "desktop.ini", "._*", ".Spotlight-V100", ".Trashes", "$RECYCLE.BIN/"]),
];

/// Group the claims returned by the server by holder for display.
pub fn claims_by_holder(claims: &[Claim]) -> HashMap<String, Vec<&Claim>> {
    let mut m: HashMap<String, Vec<&Claim>> = HashMap::new();
    for c in claims {
        m.entry(c.user.as_ref().map(|u| u.name.clone()).unwrap_or_default()).or_default().push(c);
    }
    m
}
