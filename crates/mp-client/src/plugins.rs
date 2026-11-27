//! Plugins: GitHub repositories (or local folders) with a `multiparity-plugin.json` that add
//! hooks around publishing and getting the latest, operations on files and projects, and views
//! (web pages shown in the app). A plugin's code is any program, run per call with a JSON request
//! on stdin; it answers with JSON lines on stdout. See docs/PLUGINS.md.
//!
//! Installed plugins live in `<config>/plugins/<id>/<commit>/`, listed in `<config>/plugins/plugins.json`,
//! each pinned to the exact commit that was installed.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::accounts::config_dir;
use crate::workspace::Workspace;

pub const MANIFEST: &str = "multiparity-plugin.json";
pub const HOOKS: [&str; 3] = ["before_publish", "after_publish", "after_get_latest"];
/// What a plugin may say it needs; shown before installing.
pub const PERMISSIONS: [(&str, &str); 5] = [
    ("files:read", "Read files in your synced project folders"),
    ("files:write", "Change or add files in your synced project folders"),
    ("network", "Connect to the internet or other computers"),
    ("programs", "Run other programs on this computer (Blender, ffmpeg…)"),
    ("server", "Read from the MultiParity server as you"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub homepage: String,
    /// How to start it: the command and its arguments, with optional per-OS versions.
    #[serde(default)]
    pub run: Option<Run>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub views: Vec<View>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Run {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub windows: Option<Vec<String>>,
    #[serde(default)]
    pub macos: Option<Vec<String>>,
    #[serde(default)]
    pub linux: Option<Vec<String>>,
    /// Seconds before a call is stopped (default 600; hooks 120).
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Glob patterns of files it works on (`*.blend`, `Art/**/*.png`); none: a project action.
    #[serde(default)]
    pub files: Vec<String>,
    /// Asked before running, when set.
    #[serde(default)]
    pub confirm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub id: String,
    pub title: String,
    /// The page, a path inside the plugin (`ui/index.html`).
    pub page: String,
    /// `project` (a tab beside Your work and Files) or `app` (a page of its own in the sidebar).
    #[serde(default = "project_place")]
    pub place: String,
}

fn project_place() -> String {
    "project".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub manifest: Manifest,
    /// `github:owner/repo[/sub/dir]` or `local:<folder>`.
    pub source: String,
    /// What was asked for (a tag, branch or commit), if anything.
    #[serde(default)]
    pub git_ref: Option<String>,
    /// The exact commit installed (`local` for a linked folder).
    pub commit: String,
    pub dir: PathBuf,
    pub enabled: bool,
    pub installed_at: String,
    /// Installed because an organization requires it.
    #[serde(default)]
    pub required_by: Option<String>,
}

impl Manifest {
    pub fn check(&self) -> Result<()> {
        if self.id.is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
            bail!("The plugin's id must be letters, numbers, dashes, dots or underscores.");
        }
        if self.name.trim().is_empty() {
            bail!("The plugin has no name.");
        }
        for h in &self.hooks {
            if !HOOKS.contains(&h.as_str()) {
                bail!("The plugin asks for a hook MultiParity does not have: {h}. Hooks: {}.", HOOKS.join(", "));
            }
        }
        if (!self.hooks.is_empty() || !self.operations.is_empty()) && self.command().is_none() {
            bail!("The plugin has hooks or operations but no run command.");
        }
        for v in &self.views {
            if v.page.contains("..") || v.page.starts_with('/') {
                bail!("A view's page must be a path inside the plugin.");
            }
            if v.place != "project" && v.place != "app" {
                bail!("A view's place is project or app.");
            }
        }
        for o in &self.operations {
            for g in &o.files {
                globset::Glob::new(g).with_context(|| format!("{} is not a file pattern", g))?;
            }
        }
        Ok(())
    }

    /// The command for this computer's OS.
    pub fn command(&self) -> Option<Vec<String>> {
        let r = self.run.as_ref()?;
        let os = if cfg!(windows) { &r.windows } else if cfg!(target_os = "macos") { &r.macos } else { &r.linux };
        let c = os.clone().unwrap_or_else(|| r.command.clone());
        (!c.is_empty()).then_some(c)
    }
}

impl Operation {
    /// Does it apply to this file? (No patterns: it is a project action, not a file one.)
    pub fn matches(&self, path: &str) -> bool {
        let mut b = globset::GlobSetBuilder::new();
        for g in &self.files {
            if let Ok(x) = globset::GlobBuilder::new(g).case_insensitive(true).literal_separator(false).build() {
                b.add(x);
            }
        }
        b.build().map(|s| s.is_match(path) || s.is_match(path.rsplit('/').next().unwrap_or(path))).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------------------------
// The list of installed plugins

pub fn plugins_dir() -> PathBuf {
    config_dir().join("plugins")
}

fn registry_path() -> PathBuf {
    plugins_dir().join("plugins.json")
}

pub fn installed() -> Result<Vec<Installed>> {
    match std::fs::read(registry_path()) {
        Ok(b) => Ok(serde_json::from_slice(&b).context("reading plugins.json")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

fn save(list: &[Installed]) -> Result<()> {
    std::fs::create_dir_all(plugins_dir())?;
    let tmp = registry_path().with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(list)?)?;
    std::fs::rename(&tmp, registry_path())?;
    Ok(())
}

pub fn find(id: &str) -> Result<Installed> {
    installed()?.into_iter().find(|p| p.manifest.id == id).with_context(|| format!("There is no plugin called {id} installed."))
}

pub fn set_enabled(id: &str, on: bool) -> Result<Installed> {
    let mut list = installed()?;
    let p = list.iter_mut().find(|p| p.manifest.id == id).with_context(|| format!("There is no plugin called {id} installed."))?;
    p.enabled = on;
    let out = p.clone();
    save(&list)?;
    Ok(out)
}

pub fn remove(id: &str) -> Result<Installed> {
    let mut list = installed()?;
    let i = list.iter().position(|p| p.manifest.id == id).with_context(|| format!("There is no plugin called {id} installed."))?;
    let p = list.remove(i);
    save(&list)?;
    if !p.source.starts_with("local:") {
        let _ = std::fs::remove_dir_all(plugins_dir().join(&p.manifest.id));
    }
    Ok(p)
}

// ---------------------------------------------------------------------------------------------
// Getting a plugin

/// `owner/repo`, `owner/repo/sub/dir`, either with `@ref`, or a github.com address.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub owner: String,
    pub repo: String,
    pub subdir: String,
    pub git_ref: Option<String>,
}

impl Spec {
    pub fn parse(s: &str) -> Result<Spec> {
        let s = s.trim().trim_start_matches("github:");
        let (path, mut git_ref) = match s.rsplit_once('@') {
            Some((p, r)) if !p.contains("://") || p.matches('@').count() == 0 => (p.to_string(), Some(r.to_string())),
            _ => (s.to_string(), None),
        };
        let path = path.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches("github.com/").trim_end_matches(".git").trim_matches('/').to_string();
        let mut parts: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
        if parts.len() < 2 {
            bail!("Name a GitHub repository as owner/repo (optionally /folder and @tag).");
        }
        let (owner, repo) = (parts.remove(0).to_string(), parts.remove(0).to_string());
        // github.com/owner/repo/tree/<ref>/<dir>
        if parts.first() == Some(&"tree") && parts.len() >= 2 {
            git_ref = git_ref.or(Some(parts[1].to_string()));
            parts.drain(0..2);
        }
        let ok = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
        if !ok(&owner) || !ok(&repo) || parts.iter().any(|p| *p == ".." || !p.chars().all(|c| c.is_ascii_alphanumeric() || "-_. ".contains(c))) {
            bail!("That does not look like a GitHub repository: {s}");
        }
        Ok(Spec { owner, repo, subdir: parts.join("/"), git_ref: git_ref.filter(|r| !r.is_empty()) })
    }

    pub fn source(&self) -> String {
        if self.subdir.is_empty() { format!("github:{}/{}", self.owner, self.repo) } else { format!("github:{}/{}/{}", self.owner, self.repo, self.subdir) }
    }
}

/// A plugin fetched and checked but not installed yet, so its details can be shown first.
#[derive(Debug, Clone, Serialize)]
pub struct Staged {
    pub manifest: Manifest,
    pub source: String,
    pub git_ref: Option<String>,
    pub commit: String,
    /// The folder holding it, inside the staging area.
    pub dir: PathBuf,
    /// Already installed: the version and commit it would replace.
    pub replaces: Option<(String, String)>,
    /// Permissions it asks for that the installed version did not.
    pub new_permissions: Vec<String>,
}

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(concat!("MultiParity/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(120)).build()?)
}

async fn gh(client: &reqwest::Client, path: &str) -> Result<reqwest::Response> {
    let mut req = client.get(format!("https://api.github.com{path}")).header("Accept", "application/vnd.github+json");
    // A token lets people install from private repositories (and lifts GitHub's rate limit).
    if let Ok(t) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("MP_GITHUB_TOKEN")) {
        if !t.trim().is_empty() {
            req = req.bearer_auth(t.trim());
        }
    }
    Ok(req.send().await?)
}

/// The commit to install: the ref asked for, else the latest release, else the default branch.
async fn resolve(client: &reqwest::Client, spec: &Spec) -> Result<String> {
    let (o, r) = (&spec.owner, &spec.repo);
    let git_ref = match &spec.git_ref {
        Some(x) => x.clone(),
        None => {
            let rel = gh(client, &format!("/repos/{o}/{r}/releases/latest")).await?;
            if rel.status().is_success() {
                rel.json::<Value>().await?["tag_name"].as_str().unwrap_or_default().to_string()
            } else {
                let repo = gh(client, &format!("/repos/{o}/{r}")).await?;
                if repo.status() == reqwest::StatusCode::NOT_FOUND {
                    bail!("GitHub has no repository {o}/{r} (or it is private: set GITHUB_TOKEN to install from private repositories).");
                }
                repo.error_for_status_ref()?;
                repo.json::<Value>().await?["default_branch"].as_str().unwrap_or("main").to_string()
            }
        }
    };
    let c = gh(client, &format!("/repos/{o}/{r}/commits/{git_ref}")).await?;
    if !c.status().is_success() {
        bail!("{o}/{r} has no tag, branch or commit called {git_ref}.");
    }
    Ok(c.json::<Value>().await?["sha"].as_str().context("GitHub did not say which commit that is")?.to_string())
}

fn staging_dir() -> PathBuf {
    plugins_dir().join(".staging")
}

/// Download a plugin from GitHub and check it, without installing it.
pub async fn fetch(spec_text: &str) -> Result<Staged> {
    let spec = Spec::parse(spec_text)?;
    let client = http()?;
    let commit = resolve(&client, &spec).await?;
    let resp = gh(&client, &format!("/repos/{}/{}/tarball/{commit}", spec.owner, spec.repo)).await?.error_for_status()?;
    let bytes = resp.bytes().await?;
    if bytes.len() > 512 * 1024 * 1024 {
        bail!("That repository is over 512 MB; a plugin should be much smaller.");
    }
    let into = staging_dir().join(format!("{}-{}", &commit[..12.min(commit.len())], uuid::Uuid::new_v4().simple()));
    let into2 = into.clone();
    tokio::task::spawn_blocking(move || unpack(&bytes, &into2)).await??;
    // The tarball has one top folder, owner-repo-<sha>.
    let top = std::fs::read_dir(&into)?.filter_map(|e| e.ok()).find(|e| e.path().is_dir()).map(|e| e.path()).context("The download was empty.")?;
    let dir = if spec.subdir.is_empty() { top } else { top.join(&spec.subdir) };
    stage(dir, spec.source(), spec.git_ref.clone(), commit)
}

fn unpack(gz: &[u8], into: &Path) -> Result<()> {
    std::fs::create_dir_all(into)?;
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(gz));
    for entry in ar.entries()? {
        let mut e = entry?;
        // unpack_in refuses paths that would land outside `into`.
        if !e.unpack_in(into)? {
            bail!("The download has a file outside its folder: refused.");
        }
    }
    Ok(())
}

/// Download a plugin published as an archive (.zip or .tar.gz) on the web, check its SHA-256
/// when given, and stage it. `source` says where it came from (for updates).
pub async fn fetch_archive(url: &str, sha256: Option<&str>, source: String, version: Option<String>) -> Result<Staged> {
    // https only, except on this computer (for trying out an index while making one).
    let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
    if !url.starts_with("https://") && !local {
        bail!("Plugins on the web must be on an https address.");
    }
    let bytes = http()?.get(url).send().await?.error_for_status().with_context(|| format!("Could not download {url}"))?.bytes().await?;
    if bytes.len() > 512 * 1024 * 1024 {
        bail!("That archive is over 512 MB; a plugin should be much smaller.");
    }
    use sha2::Digest;
    let got = hex::encode(sha2::Sha256::digest(&bytes));
    if let Some(want) = sha256 {
        if !want.trim().eq_ignore_ascii_case(&got) {
            bail!("The download does not match its checksum, so it was not installed. The file may have been changed since it was published.");
        }
    }
    let into = staging_dir().join(format!("{}-{}", &got[..12], uuid::Uuid::new_v4().simple()));
    let into2 = into.clone();
    let is_zip = bytes.starts_with(b"PK");
    tokio::task::spawn_blocking(move || -> Result<()> {
        if is_zip {
            std::fs::create_dir_all(&into2)?;
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec()))?;
            // extract() refuses paths that would land outside the folder.
            z.extract(&into2)?;
            Ok(())
        } else {
            unpack(&bytes, &into2)
        }
    })
    .await??;
    // The manifest at the top, or in the one folder the archive holds.
    let dir = if into.join(MANIFEST).exists() {
        into.clone()
    } else {
        let dirs: Vec<PathBuf> = std::fs::read_dir(&into)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
        match dirs.as_slice() {
            [one] if one.join(MANIFEST).exists() => one.clone(),
            _ => bail!("The archive has no {MANIFEST} at its top."),
        }
    };
    stage(dir, source, version, got)
}

/// Use a folder on this computer as a plugin (for making one): changes there apply straight away.
pub fn link(folder: &Path) -> Result<Installed> {
    let dir = std::fs::canonicalize(folder).with_context(|| format!("There is no folder {}", folder.display()))?;
    // Without Windows' \\?\ prefix, which canonicalize adds.
    let dir = PathBuf::from(dir.display().to_string().trim_start_matches(r"\\?\"));
    let st = stage(dir.clone(), format!("local:{}", dir.display()), None, "local".into())?;
    install_from(st, true)
}

fn stage(dir: PathBuf, source: String, git_ref: Option<String>, commit: String) -> Result<Staged> {
    let raw = std::fs::read(dir.join(MANIFEST)).with_context(|| format!("There is no {MANIFEST} in {}.", if source.starts_with("local:") { dir.display().to_string() } else { source.clone() }))?;
    let manifest: Manifest = serde_json::from_slice(&raw).with_context(|| format!("{MANIFEST} is not valid"))?;
    manifest.check()?;
    let old = installed()?.into_iter().find(|p| p.manifest.id == manifest.id);
    let new_permissions = manifest.permissions.iter().filter(|p| old.as_ref().map(|o| !o.manifest.permissions.contains(p)).unwrap_or(true)).cloned().collect();
    Ok(Staged { replaces: old.map(|o| (o.manifest.version, o.commit)), new_permissions, manifest, source, git_ref, commit, dir })
}

/// Install what `fetch` staged: moved to `<plugins>/<id>/<commit>`, replacing an older version.
pub fn install(st: Staged) -> Result<Installed> {
    install_from(st, false)
}

fn install_from(st: Staged, linked: bool) -> Result<Installed> {
    let id = st.manifest.id.clone();
    let dir = if linked {
        st.dir.clone()
    } else {
        let dest = plugins_dir().join(&id).join(&st.commit[..12.min(st.commit.len())]);
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }
        std::fs::create_dir_all(dest.parent().unwrap())?;
        std::fs::rename(&st.dir, &dest).or_else(|_| copy_dir(&st.dir, &dest))?;
        let _ = std::fs::remove_dir_all(staging_dir());
        dest
    };
    let mut list = installed()?;
    let old = list.iter().position(|p| p.manifest.id == id).map(|i| list.remove(i));
    let p = Installed {
        manifest: st.manifest,
        source: st.source,
        git_ref: st.git_ref,
        commit: st.commit,
        dir: dir.clone(),
        enabled: old.as_ref().map(|o| o.enabled).unwrap_or(true),
        installed_at: chrono::Utc::now().to_rfc3339(),
        required_by: old.and_then(|o| o.required_by),
    };
    list.push(p.clone());
    list.sort_by(|a, b| a.manifest.name.to_lowercase().cmp(&b.manifest.name.to_lowercase()));
    save(&list)?;
    // Older versions' folders go.
    if !linked {
        if let Ok(rd) = std::fs::read_dir(plugins_dir().join(&id)) {
            for e in rd.flatten() {
                if e.path() != dir {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }
    Ok(p)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let t = to.join(e.file_name());
        if e.file_type()?.is_dir() { copy_dir(&e.path(), &t)? } else { std::fs::copy(e.path(), t).map(|_| ())? }
    }
    Ok(())
}

/// Fetch the newest version of an installed plugin from where it came from (same ref, or the
/// latest release). A plugin pinned to a commit stays where it is.
pub async fn fetch_update(id: &str) -> Result<Option<Staged>> {
    let p = find(id)?;
    if p.source.starts_with("web:") {
        return crate::catalog::fetch_web_update(&p.source, p.git_ref.as_deref()).await;
    }
    let Some(rest) = p.source.strip_prefix("github:") else { bail!("{} is a folder on this computer; it is always current.", p.manifest.name) };
    if p.git_ref.as_deref().map(|r| r.len() == 40 && r.chars().all(|c| c.is_ascii_hexdigit())).unwrap_or(false) {
        return Ok(None);
    }
    // Installed from a release: the newest release. From a branch: its newest commit.
    let target = match &p.git_ref {
        None => None,
        Some(r) => {
            let sp = Spec::parse(rest)?;
            let versions = crate::catalog::github_versions(&sp.owner, &sp.repo).await?;
            let was_release = versions.iter().any(|v| v.kind != "branch" && &v.git_ref == r);
            match versions.iter().find(|v| v.recommended && v.kind == "release") {
                Some(newest) if was_release => Some(newest.git_ref.clone()),
                _ => Some(r.clone()),
            }
        }
    };
    let spec = match &target { Some(r) => format!("{rest}@{r}"), None => rest.to_string() };
    let st = fetch(&spec).await?;
    if st.commit == p.commit {
        let _ = std::fs::remove_dir_all(staging_dir());
        return Ok(None);
    }
    Ok(Some(st))
}

// ---------------------------------------------------------------------------------------------
// Running a plugin

/// Where a call happens: the synced folder and its project.
pub fn context_of(ws: &Workspace) -> Value {
    json!({
        "root": ws.root.display().to_string().trim_start_matches(r"\\?\"),
        "project": { "id": ws.config.project_id, "name": ws.config.project_name },
        "organization": ws.config.org_name, "branch": ws.config.branch_name, "server": ws.config.server,
        "workspace_id": ws.config.id,
    })
}

/// Run one call. `on_line` sees each progress or log line. Returns the plugin's `result`.
pub async fn call(p: &Installed, request: Value, timeout: Duration, mut on_line: impl FnMut(&Value)) -> Result<Value> {
    let cmd = p.manifest.command().with_context(|| format!("{} has no run command.", p.manifest.name))?;
    // A program in the plugin's folder (./main.exe, bin/tool) runs from there; others from PATH.
    let first = &cmd[0];
    let program = if first.starts_with("./") || first.starts_with(".\\") || (first.contains('/') && !Path::new(first).is_absolute()) { p.dir.join(first) } else { PathBuf::from(first) };
    let mut child = tokio::process::Command::new(&program)
        .args(&cmd[1..])
        .current_dir(&p.dir)
        .env("MP_PLUGIN_DIR", &p.dir)
        .env("MP_PLUGIN_ID", &p.manifest.id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("Could not start {} ({}). Is it installed on this computer?", p.manifest.name, program.display()))?;
    let mut stdin = child.stdin.take().unwrap();
    let mut req = serde_json::to_vec(&request)?;
    req.push(b'\n');
    stdin.write_all(&req).await?;
    drop(stdin);
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let errs = tokio::spawn(async move {
        let mut b = Vec::new();
        let _ = stderr.read_to_end(&mut b).await;
        String::from_utf8_lossy(&b[b.len().saturating_sub(2000)..]).trim().to_string()
    });
    let work = async {
        let mut lines = BufReader::new(stdout).lines();
        let mut result: Option<Result<Value>> = None;
        while let Some(line) = lines.next_line().await? {
            let Ok(v) = serde_json::from_str::<Value>(line.trim()) else { continue };
            if let Some(r) = v.get("result") {
                result = Some(Ok(r.clone()));
            } else if let Some(e) = v.get("error") {
                result = Some(Err(anyhow::anyhow!("{}", e.as_str().map(str::to_string).unwrap_or_else(|| e["message"].as_str().unwrap_or("failed").to_string()))));
            } else {
                on_line(&v);
            }
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((result, status))
    };
    let (result, status) = match tokio::time::timeout(timeout, work).await {
        Ok(r) => r?,
        Err(_) => bail!("{} took longer than {} s and was stopped.", p.manifest.name, timeout.as_secs()),
    };
    let stderr = errs.await.unwrap_or_default();
    match result {
        Some(r) => r,
        None if status.success() => Ok(json!({})),
        None => bail!("{} stopped with an error{}", p.manifest.name, if stderr.is_empty() { ".".into() } else { format!(": {stderr}") }),
    }
}

fn timeout_of(p: &Installed, default: u64) -> Duration {
    Duration::from_secs(p.manifest.run.as_ref().and_then(|r| r.timeout).unwrap_or(default).clamp(5, 24 * 3600))
}

/// Run every enabled plugin that has this hook. A `before_` hook that answers `{ok: false}` (or
/// fails) stops what was about to happen, with its message; `after_` hooks only report.
pub async fn hook(name: &str, ws: &Workspace, data: Value) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    for p in installed()?.into_iter().filter(|p| p.enabled && p.manifest.hooks.iter().any(|h| h == name)) {
        crate::progress::phase(&format!("{}: checking", p.manifest.name), 0, 0);
        let req = json!({ "protocol": 1, "kind": "hook", "hook": name, "workspace": context_of(ws), "data": data });
        let label = p.manifest.name.clone();
        let r = call(&p, req, timeout_of(&p, 120), |line| {
            if let Some(m) = line["message"].as_str() {
                crate::progress::phase(&format!("{label}: {m}"), 0, 0);
            }
        })
        .await;
        let stopping = name.starts_with("before_");
        match r {
            Ok(v) if v["ok"] == json!(false) => {
                let why = v["message"].as_str().unwrap_or("it said no");
                if stopping {
                    bail!("{label} stopped this: {why} (Turn the plugin off under Plugins to go ahead without it.)");
                }
                notes.push(format!("{label}: {why}"));
            }
            Ok(v) => {
                if let Some(m) = v["message"].as_str().filter(|m| !m.is_empty()) {
                    notes.push(format!("{label}: {m}"));
                }
            }
            Err(e) if stopping => bail!("{label} could not check this, so it was stopped: {e:#} (Turn the plugin off under Plugins to go ahead without it.)"),
            Err(e) => {
                tracing::warn!("plugin {} hook {name}: {e:#}", p.manifest.id);
                notes.push(format!("{label}: {e:#}"));
            }
        }
    }
    Ok(notes)
}

/// Run a plugin's operation on files (paths inside the folder) or on the whole project.
pub async fn run_operation(id: &str, op: &str, ws: &Workspace, paths: &[String]) -> Result<Value> {
    let p = find(id)?;
    if !p.enabled {
        bail!("{} is turned off. Turn it on under Plugins.", p.manifest.name);
    }
    let o = p.manifest.operations.iter().find(|o| o.id == op).with_context(|| format!("{} has no operation {op}.", p.manifest.name))?;
    if !o.files.is_empty() {
        if paths.is_empty() {
            bail!("Choose the files for {}.", o.title);
        }
        if let Some(bad) = paths.iter().find(|x| !o.matches(x)) {
            bail!("{} does not work on {bad}.", o.title);
        }
    }
    let files: Vec<Value> = paths.iter().map(|x| json!({ "path": x, "abs": ws.abs(x).display().to_string().trim_start_matches(r"\\?\") })).collect();
    let req = json!({ "protocol": 1, "kind": "operation", "operation": op, "workspace": context_of(ws), "files": files });
    crate::progress::phase(&format!("{}: {}", p.manifest.name, o.title), 0, 0);
    let label = format!("{}: {}", p.manifest.name, o.title);
    call(&p, req, timeout_of(&p, 600), |line| {
        if let Some(m) = line["message"].as_str() {
            crate::progress::phase(&format!("{label}: {m}"), 0, 0);
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs() {
        let s = Spec::parse("vltmedia/MultiParity/plugins/example@v1.0").unwrap();
        assert_eq!((s.owner.as_str(), s.repo.as_str(), s.subdir.as_str(), s.git_ref.as_deref()), ("vltmedia", "MultiParity", "plugins/example", Some("v1.0")));
        let s = Spec::parse("https://github.com/a/b/tree/main/tools/x").unwrap();
        assert_eq!((s.subdir.as_str(), s.git_ref.as_deref()), ("tools/x", Some("main")));
        let s = Spec::parse("github:a/b.git").unwrap();
        assert_eq!((s.repo.as_str(), s.git_ref), ("b", None));
        assert!(Spec::parse("just-one").is_err());
        assert!(Spec::parse("a/b/../../etc").is_err());
    }

    #[test]
    fn operation_patterns() {
        let o = Operation { id: "x".into(), title: "X".into(), description: String::new(), files: vec!["*.blend".into()], confirm: None };
        assert!(o.matches("Scenes/level.blend"));
        assert!(o.matches("hero.BLEND"));
        assert!(!o.matches("hero.png"));
    }

    #[test]
    fn manifest_checks() {
        let m: Manifest = serde_json::from_value(json!({ "id": "a", "name": "A", "hooks": ["before_publish"] })).unwrap();
        assert!(m.check().is_err(), "hooks need a run command");
        let m: Manifest = serde_json::from_value(json!({ "id": "a", "name": "A", "hooks": ["on_lunch"], "run": { "command": ["x"] } })).unwrap();
        assert!(m.check().is_err());
        let m: Manifest = serde_json::from_value(json!({ "id": "a", "name": "A", "views": [{ "id": "v", "title": "V", "page": "../x.html" }] })).unwrap();
        assert!(m.check().is_err());
        let m: Manifest = serde_json::from_value(json!({ "id": "ok-1", "name": "A", "run": { "command": ["python", "main.py"] }, "hooks": ["before_publish"],
            "operations": [{ "id": "o", "title": "O", "files": ["*.png"] }], "views": [{ "id": "v", "title": "V", "page": "ui/index.html", "place": "app" }] })).unwrap();
        m.check().unwrap();
    }
}
