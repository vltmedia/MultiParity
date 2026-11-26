//! A project connected to a folder on this computer.
//!
//! Everything MultiParity keeps locally lives in `<folder>/.mp/`:
//! - `workspace.json`: which server, account, project and branch; selective sync folders.
//! - `index.json`: for every file on disk that matches the server, the version it matches.
//! - `remote.json`: the server's whole tree for this branch as of the last get latest (used for
//!   folder sizes in selective sync without asking the server).
//! - `cache/`: compressed parts of checkpointed versions that exist nowhere else. Synced and
//!   published files are never copied here; see `ops::trim_cache`.
//! - `checkpoints/`: private save points of unpublished work.
//! - `safety/`: files moved aside instead of being overwritten or deleted.
//! - `ignore`: personal ignore rules (never published).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::accounts::config_dir;

pub const MP_DIR: &str = ".mp";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub id: String,
    pub server: String,
    pub account_id: String,
    pub account_email: String,
    pub project_id: String,
    pub project_name: String,
    pub org_name: String,
    pub branch_id: String,
    pub branch_name: String,
    pub synced_change: i64,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Claim claimable files automatically when they are first edited (daemon).
    #[serde(default = "yes")]
    pub auto_claim: bool,
    /// Automatic checkpoints while working (daemon), for this folder: `Some(true/false)`, or
    /// `None` to follow the project. Off unless the project or the person turns them on.
    #[serde(default)]
    pub auto_checkpoints: Option<bool>,
    /// What the project said last time we asked.
    #[serde(default)]
    pub project_auto_checkpoints: bool,
    /// Most disk space checkpoints may use in `.mp/cache`, in MB. `None`: a tenth of the free
    /// space on this disk. Over it, the oldest automatic checkpoints are removed. Checkpoints
    /// people make themselves are always kept.
    #[serde(default)]
    pub checkpoint_limit_mb: Option<u64>,
    /// Where this folder keeps checkpoints: `Some("server")`, `Some("local")`, or `None` to
    /// follow the project.
    #[serde(default)]
    pub checkpoint_storage: Option<String>,
    /// What the project said last time we asked ("server" or "local").
    #[serde(default)]
    pub project_checkpoint_storage: Option<String>,
}

fn yes() -> bool {
    true
}

impl WorkspaceConfig {
    /// True when the daemon should take checkpoints by itself in this folder.
    pub fn auto_checkpoints_on(&self) -> bool {
        self.auto_checkpoints.unwrap_or(self.project_auto_checkpoints)
    }

    /// True when checkpoints go to the server instead of this computer's disk.
    pub fn checkpoints_on_server(&self) -> bool {
        self.checkpoint_storage.as_deref().or(self.project_checkpoint_storage.as_deref()) == Some("server")
    }
}

/// What a file on disk matches on the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexEntry {
    pub file_hash: String,
    pub manifest_hash: String,
    pub size: u64,
    /// Modification time in nanoseconds since the epoch; a quick "unchanged" check.
    pub mtime: i128,
    pub change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Index {
    pub files: BTreeMap<String, IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub file_hash: String,
    pub manifest_hash: String,
    pub size: u64,
    pub change: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteTree {
    pub head: i64,
    pub files: BTreeMap<String, RemoteEntry>,
}

pub struct Workspace {
    pub root: PathBuf,
    pub config: WorkspaceConfig,
    pub index: Index,
    pub remote: RemoteTree,
}

impl Workspace {
    pub fn mp_dir(root: &Path) -> PathBuf {
        root.join(MP_DIR)
    }

    /// Find the workspace containing `start` (it or any parent).
    pub fn find(start: &Path) -> Result<PathBuf> {
        let start = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
        let mut cur: Option<&Path> = Some(&start);
        while let Some(dir) = cur {
            if dir.join(MP_DIR).join("workspace.json").exists() {
                return Ok(dir.to_path_buf());
            }
            cur = dir.parent();
        }
        anyhow::bail!("This folder is not connected to a MultiParity project. Run `mp connect <project> <folder>` first.")
    }

    pub fn open(root: &Path) -> Result<Self> {
        let dir = Self::mp_dir(root);
        let config: WorkspaceConfig = read_json(&dir.join("workspace.json"))?.context("workspace.json is missing")?;
        Ok(Workspace {
            root: root.to_path_buf(),
            config,
            index: read_json(&dir.join("index.json"))?.unwrap_or_default(),
            remote: read_json(&dir.join("remote.json"))?.unwrap_or_default(),
        })
    }

    pub fn create(root: &Path, config: WorkspaceConfig) -> Result<Self> {
        let dir = Self::mp_dir(root);
        std::fs::create_dir_all(dir.join("cache")).with_context(|| format!("creating {}", dir.display()))?;
        hide_dir(&dir);
        let ws = Workspace { root: root.to_path_buf(), config, index: Index::default(), remote: RemoteTree::default() };
        ws.save()?;
        Registry::add(root)?;
        Ok(ws)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::mp_dir(&self.root);
        write_json_atomic(&dir.join("workspace.json"), &self.config)?;
        write_json_atomic(&dir.join("index.json"), &self.index)?;
        write_json_atomic(&dir.join("remote.json"), &self.remote)?;
        Ok(())
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in rel.split('/') {
            p.push(part);
        }
        p
    }

    pub fn in_scope(&self, rel: &str) -> bool {
        mp_core::paths::in_scope(rel, &self.config.include, &self.config.exclude)
    }

    pub fn cache_dir(&self) -> PathBuf {
        Self::mp_dir(&self.root).join("cache")
    }

    pub fn tmp_dir(&self) -> Result<PathBuf> {
        let d = Self::mp_dir(&self.root).join("tmp");
        std::fs::create_dir_all(&d)?;
        Ok(d)
    }

    /// Path of a cached compressed chunk.
    pub fn cached_chunk(&self, hash: &str) -> PathBuf {
        self.cache_dir().join(&hash[..2]).join(hash)
    }

    pub fn put_cached_chunk(&self, hash: &str, compressed: &[u8]) -> Result<()> {
        let p = self.cached_chunk(hash);
        if p.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(p.parent().unwrap())?;
        let tmp = p.with_extension(format!("tmp{}", std::process::id()));
        std::fs::write(&tmp, compressed)?;
        std::fs::rename(&tmp, &p)?;
        Ok(())
    }

    /// Move a file into `.mp/safety/<stamp>/<rel>` instead of deleting or overwriting it.
    pub fn move_to_safety(&self, rel: &str, stamp: &str) -> Result<PathBuf> {
        let dest = Self::mp_dir(&self.root).join("safety").join(stamp).join(rel);
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::rename(self.abs(rel), &dest).or_else(|_| std::fs::copy(self.abs(rel), &dest).map(|_| ()).and_then(|_| std::fs::remove_file(self.abs(rel))))?;
        Ok(dest)
    }
}

pub fn mtime_of(meta: &std::fs::Metadata) -> i128 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i128)
        .unwrap_or(0)
}

pub fn read_json<T: DeserializeOwned>(p: &Path) -> Result<Option<T>> {
    match std::fs::read(p) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b).with_context(|| format!("{} is not valid", p.display()))?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", p.display())),
    }
}

/// Write JSON to a temp file, flush, then rename over the target, so a crash never leaves a
/// half-written file.
pub fn write_json_atomic<T: Serialize>(p: &Path, v: &T) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = p.with_extension(format!("json.tmp{}", std::process::id()));
    {
        let mut f = std::fs::File::create(&tmp).with_context(|| format!("writing {}", tmp.display()))?;
        f.write_all(&serde_json::to_vec_pretty(v)?)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, p).with_context(|| format!("replacing {}", p.display()))?;
    Ok(())
}

fn hide_dir(_dir: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("attrib").arg("+h").arg(_dir).status();
    }
}

/// The list of connected folders on this computer, used by the daemon and `mp workspaces`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Registry {
    pub roots: Vec<PathBuf>,
}

impl Registry {
    fn path() -> PathBuf {
        config_dir().join("workspaces.json")
    }
    pub fn load() -> Result<Self> {
        Ok(read_json(&Self::path())?.unwrap_or_default())
    }
    pub fn add(root: &Path) -> Result<()> {
        let mut r = Self::load()?;
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        if !r.roots.contains(&root) {
            r.roots.push(root);
            write_json_atomic(&Self::path(), &r)?;
        }
        Ok(())
    }
    pub fn remove(root: &Path) -> Result<()> {
        let mut r = Self::load()?;
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        r.roots.retain(|x| x != &root);
        write_json_atomic(&Self::path(), &r)
    }
    /// Connected folders that still exist.
    pub fn workspaces() -> Result<Vec<PathBuf>> {
        Ok(Self::load()?.roots.into_iter().filter(|r| r.join(MP_DIR).join("workspace.json").exists()).collect())
    }
}
