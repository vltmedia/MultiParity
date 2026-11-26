//! Finding what changed in a connected folder.
//!
//! Ignore rules come from, in order: built-in defaults, every `.mpignore` in the project (shared,
//! published like any file), and `.mp/ignore` (personal). Later and deeper rules win, and `!`
//! makes an exception, as in `.gitignore`. Files that are already tracked are never treated as
//! ignored: an ignore rule only stops new files from being added.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use serde::Serialize;

use crate::workspace::{mtime_of, Workspace, MP_DIR};

pub const DEFAULT_IGNORES: &[&str] = &[".DS_Store", "Thumbs.db", "desktop.ini", "*.tmp", "*~", "~$*", ".~lock.*", "*.swp"];

/// Render output and simulation caches: they can be made again, and are far too big to copy
/// every few minutes. Checkpoints skip them unless asked (`mp checkpoint --all`). Matched
/// without regard to case. Projects add their own in `.mpnocheckpoint`; `!pattern` there makes
/// an exception (for example `!Textures/**/*.exr`).
pub const DEFAULT_NO_CHECKPOINT: &[&str] = &[
    "*.exr", "*.dpx", "*.cin", "*.vdb", "*.abc", "*.bgeo", "*.bgeo.sc", "*.bgeo.gz", "*.sim", "*.simdata",
    "renders/", "render/", "comp/out/", "cache/", "caches/", "playblasts/", "flipbooks/",
];

pub const NO_CHECKPOINT_FILE: &str = ".mpnocheckpoint";

pub struct Ignores {
    root: PathBuf,
    defaults: Gitignore,
    /// (folder relative to root, matcher), shallowest first.
    shared: Vec<(String, Gitignore)>,
    personal: Option<Gitignore>,
    shared_name: &'static str,
}

impl Ignores {
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_rules(root, DEFAULT_IGNORES, ".mpignore", "ignore", false)
    }

    /// The rules for what checkpoints leave out: built-in defaults, every `.mpnocheckpoint` in
    /// the project (shared), and `.mp/nocheckpoint` (personal).
    pub fn load_no_checkpoint(root: &Path) -> Result<Self> {
        Self::load_rules(root, DEFAULT_NO_CHECKPOINT, NO_CHECKPOINT_FILE, "nocheckpoint", true)
    }

    fn load_rules(root: &Path, defaults: &[&str], shared_name: &'static str, personal_name: &str, any_case: bool) -> Result<Self> {
        let mut b = GitignoreBuilder::new(root);
        b.case_insensitive(any_case)?;
        for d in defaults {
            b.add_line(None, d)?;
        }
        let defaults = b.build()?;
        let mut shared = Vec::new();
        for entry in walkdir::WalkDir::new(root).into_iter().filter_entry(|e| e.file_name() != MP_DIR).flatten() {
            if entry.file_type().is_file() && entry.file_name() == shared_name {
                let dir = entry.path().parent().unwrap_or(root);
                let mut b = GitignoreBuilder::new(dir);
                b.case_insensitive(any_case)?;
                b.add(entry.path());
                if let Ok(g) = b.build() {
                    shared.push((rel_of(root, dir), g));
                }
            }
        }
        shared.sort_by_key(|(d, _)| d.matches('/').count() + usize::from(!d.is_empty()));
        let personal_path = root.join(MP_DIR).join(personal_name);
        let personal = if personal_path.exists() {
            let mut b = GitignoreBuilder::new(root);
            b.case_insensitive(any_case)?;
            b.add(&personal_path);
            b.build().ok()
        } else {
            None
        };
        Ok(Ignores { root: root.to_path_buf(), defaults, shared, personal, shared_name })
    }

    /// Whether `rel` (a file or folder) is ignored, and by which rule.
    pub fn check(&self, rel: &str, is_dir: bool) -> Option<String> {
        let abs = self.root.join(rel);
        let mut verdict: Option<String> = None;
        let mut apply = |m: Match<&ignore::gitignore::Glob>, src: &str| match m {
            Match::Ignore(g) => verdict = Some(format!("{} ({src})", g.original())),
            Match::Whitelist(_) => verdict = None,
            Match::None => {}
        };
        apply(self.defaults.matched_path_or_any_parents(&abs, is_dir), "built-in");
        for (dir, g) in &self.shared {
            if mp_core::paths::within(rel, dir) && rel != dir {
                let src = if dir.is_empty() { self.shared_name.to_string() } else { format!("{dir}/{}", self.shared_name) };
                apply(g.matched_path_or_any_parents(&abs, is_dir), &src);
            }
        }
        if let Some(p) = &self.personal {
            apply(p.matched_path_or_any_parents(&abs, is_dir), "only for you");
        }
        verdict
    }
}

pub fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalFile {
    pub path: String,
    pub size: u64,
    #[serde(skip)]
    pub mtime: i128,
    /// Set when the content was hashed during the scan.
    #[serde(skip)]
    pub file_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Status {
    pub new: Vec<LocalFile>,
    pub edited: Vec<LocalFile>,
    pub deleted: Vec<LocalFile>,
    pub unchanged: usize,
    /// Ignored files and folders (a folder counts once; its contents are not walked).
    pub ignored: Vec<String>,
    /// Index entries whose mtime changed but content did not; refreshing them speeds the next scan.
    #[serde(skip)]
    pub touched: Vec<(String, i128)>,
}

impl Status {
    pub fn is_clean(&self) -> bool {
        self.new.is_empty() && self.edited.is_empty() && self.deleted.is_empty()
    }
    pub fn changed_paths(&self) -> Vec<String> {
        self.new.iter().chain(&self.edited).chain(&self.deleted).map(|f| f.path.clone()).collect()
    }
}

/// Walk the folder and compare with the index. `only` limits the scan to those paths/folders.
pub fn status(ws: &Workspace, only: &[String]) -> Result<Status> {
    let ign = Ignores::load(&ws.root)?;
    let tracked_dirs: HashSet<String> = ws
        .index
        .files
        .keys()
        .flat_map(|p| {
            let parts: Vec<&str> = p.split('/').collect();
            (1..parts.len()).map(move |i| parts[..i].join("/")).collect::<Vec<_>>()
        })
        .collect();
    let mut st = Status::default();
    let mut seen: HashSet<String> = HashSet::new();
    let wanted = |rel: &str| only.is_empty() || only.iter().any(|o| mp_core::paths::within(rel, o));

    let root = ws.root.clone();
    let mut it = walkdir::WalkDir::new(&root).min_depth(1).into_iter();
    while let Some(entry) = it.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = %e, "skipping unreadable path");
                continue;
            }
        };
        let rel = rel_of(&root, entry.path());
        if rel == MP_DIR || rel.starts_with(".mp/") {
            if entry.file_type().is_dir() {
                it.skip_current_dir();
            }
            continue;
        }
        if entry.file_type().is_dir() {
            if !ws.in_scope_dir(&rel) {
                it.skip_current_dir();
                continue;
            }
            if !tracked_dirs.contains(&rel) && ign.check(&rel, true).is_some() {
                st.ignored.push(format!("{rel}/"));
                it.skip_current_dir();
            }
            continue;
        }
        if !entry.file_type().is_file() || !ws.in_scope(&rel) || !wanted(&rel) {
            continue;
        }
        let Some(norm) = mp_core::paths::normalize(&rel) else { continue };
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len();
        let mtime = mtime_of(&meta);
        seen.insert(norm.clone());
        match ws.index.files.get(&norm) {
            Some(ix) if ix.size == size && ix.mtime == mtime => st.unchanged += 1,
            Some(ix) => {
                let (hash, _) = hash_file(entry.path())?;
                if hash == ix.file_hash {
                    st.unchanged += 1;
                    st.touched.push((norm, mtime));
                } else {
                    st.edited.push(LocalFile { path: norm, size, mtime, file_hash: Some(hash) });
                }
            }
            None => {
                if ign.check(&norm, false).is_some() {
                    st.ignored.push(norm);
                } else {
                    st.new.push(LocalFile { path: norm, size, mtime, file_hash: None });
                }
            }
        }
    }
    for (path, ix) in &ws.index.files {
        if !seen.contains(path) && ws.in_scope(path) && wanted(path) {
            st.deleted.push(LocalFile { path: path.clone(), size: ix.size, mtime: 0, file_hash: None });
        }
    }
    Ok(st)
}

impl Workspace {
    /// A folder is walked when any part of it could be in scope.
    pub fn in_scope_dir(&self, rel: &str) -> bool {
        if self.config.exclude.iter().any(|e| mp_core::paths::within(rel, e)) {
            return false;
        }
        self.config.include.is_empty()
            || self.config.include.iter().any(|i| mp_core::paths::within(rel, i) || mp_core::paths::within(i, rel))
    }
}

pub fn hash_file(p: &Path) -> Result<(String, u64)> {
    let f = std::fs::File::open(p)?;
    mp_core::chunk::hash_reader(std::io::BufReader::with_capacity(1 << 20, f))
}

/// Files and folders on this computer, for ignore suggestions: path -> (size, tracked).
pub fn all_local_files(ws: &Workspace) -> BTreeMap<String, (u64, bool)> {
    let mut out = BTreeMap::new();
    for e in walkdir::WalkDir::new(&ws.root).min_depth(1).into_iter().filter_entry(|e| e.file_name() != MP_DIR).flatten() {
        if e.file_type().is_file() {
            let rel = rel_of(&ws.root, e.path());
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            let tracked = ws.index.files.contains_key(&rel);
            out.insert(rel, (size, tracked));
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct IgnoreChoice {
    pub label: String,
    pub pattern: String,
    pub matches: usize,
    pub bytes: u64,
    pub tracked: usize,
}

/// The right-click "Ignore" choices for one file, with what each would match here.
pub fn ignore_choices(ws: &Workspace, rel: &str) -> Vec<IgnoreChoice> {
    let files = all_local_files(ws);
    mp_core::ignore::suggest(rel)
        .into_iter()
        .map(|s| {
            let (mut matches, mut bytes, mut tracked) = (0, 0, 0);
            for (path, (size, is_tracked)) in &files {
                if mp_core::ignore::pattern_matches(&s.pattern, path) {
                    matches += 1;
                    bytes += size;
                    tracked += usize::from(*is_tracked);
                }
            }
            IgnoreChoice { label: s.label, pattern: s.pattern, matches, bytes, tracked }
        })
        .collect()
}
