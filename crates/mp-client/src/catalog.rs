//! Plugin sources and the catalog: where people find plugins to install.
//!
//! A source is either
//! - a GitHub repository with a `multiparity-plugins.json` index listing plugins (in that
//!   repository, or in others), or a repository that is itself one plugin
//!   (`multiparity-plugin.json` at its top); or
//! - a web address of a `multiparity-plugins.json`, whose plugins are GitHub repositories or
//!   archives (.zip, .tar.gz) on the web with versions and checksums.
//!
//! Each GitHub plugin can be installed from its default branch (the newest commit) or from any of
//! its releases. Sources live in `<config>/plugins/sources.json`. See docs/PLUGINS.md.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::plugins::{self, Manifest, Spec, Staged};

pub const INDEX: &str = "multiparity-plugins.json";
/// The source every app starts with: the examples in the MultiParity repository.
pub const DEFAULT_SOURCE: &str = "vltmedia/MultiParity";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    /// `github:owner/repo` or an https address.
    pub location: String,
    #[serde(default)]
    pub name: String,
    pub added_at: String,
}

/// The index file's shape.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Index {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub plugins: Vec<IndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexEntry {
    /// Another GitHub repository (`owner/repo`); left out: the index's own repository.
    #[serde(default)]
    pub repo: Option<String>,
    /// A folder in that repository holding the plugin.
    #[serde(default)]
    pub path: Option<String>,
    // For plugins published as archives on the web:
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub versions: Vec<ArchiveVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveVersion {
    pub version: String,
    pub url: String,
    /// Hex SHA-256 of the archive; checked when present.
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
}

/// One plugin in the catalog.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// Stable within the catalog: `gh:owner/repo[/path]` or `web:<index>#<id>`.
    pub key: String,
    pub source_id: String,
    pub source_name: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    /// The version on the default branch (GitHub) or the newest listed (web).
    pub version: String,
    pub homepage: String,
    /// `github` or `archive`.
    pub kind: String,
    /// For GitHub plugins: what to install (`owner/repo[/path]`, plus `@ref`).
    pub spec: Option<String>,
    pub manifest: Option<Manifest>,
    /// Why its details could not be read, if they could not.
    pub problem: Option<String>,
}

/// A version one can install.
#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub label: String,
    /// `release`, `prerelease` or `branch`.
    pub kind: String,
    /// The tag or branch (GitHub), or the version (archive).
    pub git_ref: String,
    pub published_at: Option<String>,
    pub notes: Option<String>,
    /// The one to pick unless there is a reason not to.
    pub recommended: bool,
}

// ---------------------------------------------------------------------------------------------
// Sources

fn sources_path() -> PathBuf {
    plugins::plugins_dir().join("sources.json")
}

pub fn sources() -> Result<Vec<Source>> {
    match std::fs::read(sources_path()) {
        Ok(b) => Ok(serde_json::from_slice(&b).context("reading sources.json")?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![Source {
            id: "multiparity".into(),
            location: format!("github:{DEFAULT_SOURCE}"),
            name: "MultiParity examples".into(),
            added_at: chrono::Utc::now().to_rfc3339(),
        }]),
        Err(e) => Err(e.into()),
    }
}

fn save_sources(list: &[Source]) -> Result<()> {
    std::fs::create_dir_all(plugins::plugins_dir())?;
    std::fs::write(sources_path(), serde_json::to_vec_pretty(list)?)?;
    Ok(())
}

/// `owner/repo`, a github.com address, or any other https address of an index.
pub fn normalize(location: &str) -> Result<String> {
    let l = location.trim();
    let is_github = l.starts_with("github:") || l.contains("github.com/") || (!l.contains("://") && l.split('/').filter(|x| !x.is_empty()).count() == 2);
    if is_github && !l.contains("raw.githubusercontent.com") {
        let s = Spec::parse(l)?;
        if !s.subdir.is_empty() || s.git_ref.is_some() {
            bail!("Add the repository itself (owner/repo); its {INDEX} lists the plugins in it.");
        }
        return Ok(format!("github:{}/{}", s.owner, s.repo));
    }
    if !l.starts_with("https://") && !l.starts_with("http://") {
        bail!("Add a GitHub repository (owner/repo) or the web address of a {INDEX}.");
    }
    Ok(l.to_string())
}

/// Add a source after checking it can be read; returns it with how many plugins it has.
pub async fn add_source(location: &str) -> Result<(Source, usize)> {
    let loc = normalize(location)?;
    let mut list = sources()?;
    if list.iter().any(|s| s.location.eq_ignore_ascii_case(&loc)) {
        bail!("That source is already in the list.");
    }
    let (name, entries) = read_source(&Source { id: "new".into(), location: loc.clone(), name: String::new(), added_at: String::new() }).await?;
    let src = Source {
        id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
        location: loc.clone(),
        name: if name.is_empty() { loc.trim_start_matches("github:").to_string() } else { name },
        added_at: chrono::Utc::now().to_rfc3339(),
    };
    list.push(src.clone());
    save_sources(&list)?;
    CACHE.lock().unwrap().clear();
    Ok((src, entries.len()))
}

pub fn remove_source(id: &str) -> Result<Source> {
    let mut list = sources()?;
    let i = list.iter().position(|s| s.id == id).with_context(|| format!("There is no source {id}."))?;
    let s = list.remove(i);
    save_sources(&list)?;
    CACHE.lock().unwrap().clear();
    Ok(s)
}

// ---------------------------------------------------------------------------------------------
// Reading sources

fn http() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder().user_agent(concat!("MultiParity/", env!("CARGO_PKG_VERSION"))).timeout(Duration::from_secs(60)).build()?)
}

/// A file from a GitHub repository at a ref, or None when it is not there.
async fn raw(client: &reqwest::Client, owner: &str, repo: &str, git_ref: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let url = format!("https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{}", path.trim_start_matches('/'));
    let mut req = client.get(url);
    if let Ok(t) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("MP_GITHUB_TOKEN")) {
        if !t.trim().is_empty() {
            req = req.bearer_auth(t.trim());
        }
    }
    let r = req.send().await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(r.error_for_status()?.bytes().await?.to_vec()))
}

fn join(a: &str, b: &str) -> String {
    [a.trim_matches('/'), b.trim_matches('/')].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join("/")
}

async fn github_entry(client: &reqwest::Client, src: &Source, owner: &str, repo: &str, path: &str) -> Entry {
    let spec = if path.is_empty() { format!("{owner}/{repo}") } else { format!("{owner}/{repo}/{path}") };
    let mut e = Entry {
        key: format!("gh:{spec}"),
        source_id: src.id.clone(),
        source_name: src.name.clone(),
        id: String::new(),
        name: spec.clone(),
        description: String::new(),
        author: String::new(),
        version: String::new(),
        homepage: format!("https://github.com/{owner}/{repo}{}", if path.is_empty() { String::new() } else { format!("/tree/HEAD/{path}") }),
        kind: "github".into(),
        spec: Some(spec),
        manifest: None,
        problem: None,
    };
    match raw(client, owner, repo, "HEAD", &join(path, plugins::MANIFEST)).await {
        Ok(Some(b)) => match serde_json::from_slice::<Manifest>(&b) {
            Ok(m) => {
                e.id = m.id.clone();
                e.name = m.name.clone();
                e.description = m.description.clone();
                e.author = m.author.clone();
                e.version = m.version.clone();
                if !m.homepage.is_empty() {
                    e.homepage = m.homepage.clone();
                }
                if let Err(err) = m.check() {
                    e.problem = Some(format!("{err:#}"));
                }
                e.manifest = Some(m);
            }
            Err(err) => e.problem = Some(format!("Its {} is not valid: {err}", plugins::MANIFEST)),
        },
        Ok(None) => e.problem = Some(format!("No {} there.", plugins::MANIFEST)),
        Err(err) => e.problem = Some(format!("Could not read it: {err}")),
    }
    e
}

/// The folders of a repository (on its default branch) that hold a multiparity-plugin.json.
async fn plugin_folders(client: &reqwest::Client, owner: &str, repo: &str) -> Result<Vec<String>> {
    let r = gh_api(client, &format!("/repos/{owner}/{repo}/git/trees/HEAD?recursive=1")).await?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("GitHub has no repository {owner}/{repo} (or it is private: set GITHUB_TOKEN).");
    }
    if r.status() == reqwest::StatusCode::FORBIDDEN {
        bail!("GitHub is limiting requests from this computer for now. Try again later, or set GITHUB_TOKEN.");
    }
    let tree: Value = r.error_for_status()?.json().await?;
    let mut out: Vec<String> = tree["tree"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| t["path"].as_str())
        .filter_map(|p| p.strip_suffix(&format!("/{}", plugins::MANIFEST)))
        .filter(|p| !p.split('/').any(|x| x.starts_with('.') || x == "node_modules"))
        .map(str::to_string)
        .collect();
    out.sort();
    Ok(out)
}

/// A source's name and its plugins.
pub async fn read_source(src: &Source) -> Result<(String, Vec<Entry>)> {
    let client = http()?;
    let mut out = Vec::new();
    let (index, base): (Index, Option<(String, String)>) = if let Some(rest) = src.location.strip_prefix("github:") {
        let (owner, repo) = rest.split_once('/').context("bad source")?;
        match raw(&client, owner, repo, "HEAD", INDEX).await? {
            Some(b) => (serde_json::from_slice(&b).with_context(|| format!("{rest}'s {INDEX} is not valid"))?, Some((owner.into(), repo.into()))),
            None => {
                // A repository that is itself a plugin.
                let e = github_entry(&client, src, owner, repo, "").await;
                if e.manifest.is_some() {
                    return Ok((e.name.clone(), vec![e]));
                }
                // Or one with plugins in its folders: every multiparity-plugin.json in it.
                let folders = plugin_folders(&client, owner, repo).await?;
                if folders.is_empty() {
                    bail!("{rest} has no {INDEX} and no {} in any folder: it is not a MultiParity plugin source.", plugins::MANIFEST);
                }
                for f in folders.iter().take(100) {
                    out.push(github_entry(&client, src, owner, repo, f).await);
                }
                return Ok((rest.to_string(), out));
            }
        }
    } else {
        let r = client
            .get(&src.location)
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Could not reach {}. Check the address and your connection.", src.location))?
            .error_for_status()
            .with_context(|| format!("Could not read {}", src.location))?;
        (r.json().await.with_context(|| format!("{} is not a valid {INDEX}", src.location))?, None)
    };
    for (i, p) in index.plugins.iter().enumerate() {
        if !p.versions.is_empty() {
            let id = p.id.clone().unwrap_or_else(|| format!("plugin-{i}"));
            out.push(Entry {
                key: format!("web:{}#{id}", src.location),
                source_id: src.id.clone(),
                source_name: if index.name.is_empty() { src.name.clone() } else { index.name.clone() },
                id: id.clone(),
                name: p.name.clone().unwrap_or(id),
                description: p.description.clone().unwrap_or_default(),
                author: p.author.clone().unwrap_or_default(),
                version: p.versions[0].version.clone(),
                homepage: String::new(),
                kind: "archive".into(),
                spec: None,
                manifest: None,
                problem: None,
            });
            continue;
        }
        let (owner, repo) = match (&p.repo, &base) {
            (Some(r), _) => {
                let s = Spec::parse(r)?;
                (s.owner, s.repo)
            }
            (None, Some((o, r))) => (o.clone(), r.clone()),
            (None, None) => bail!("An entry in {} has no repo and no versions.", src.location),
        };
        let mut e = github_entry(&client, src, &owner, &repo, p.path.as_deref().unwrap_or("")).await;
        if !index.name.is_empty() {
            e.source_name = index.name.clone();
        }
        out.push(e);
    }
    Ok((index.name, out))
}

static CACHE: std::sync::LazyLock<Mutex<HashMap<String, (Instant, Vec<Entry>, Option<String>)>>> = std::sync::LazyLock::new(Default::default);

/// Every source's plugins (kept for ten minutes unless `refresh`), and each source's problem.
pub async fn catalog(refresh: bool) -> Result<Vec<(Source, Vec<Entry>, Option<String>)>> {
    let mut out = Vec::new();
    for src in sources()? {
        let cached = CACHE.lock().unwrap().get(&src.location).filter(|(t, _, _)| !refresh && t.elapsed() < Duration::from_secs(600)).cloned();
        let (entries, problem) = match cached {
            Some((_, e, p)) => (e, p),
            None => {
                let (e, p) = match read_source(&src).await {
                    Ok((_, e)) => (e, None),
                    Err(err) => (vec![], Some(format!("{err:#}"))),
                };
                CACHE.lock().unwrap().insert(src.location.clone(), (Instant::now(), e.clone(), p.clone()));
                (e, p)
            }
        };
        out.push((src, entries, problem));
    }
    Ok(out)
}

pub async fn find_entry(key: &str) -> Result<Entry> {
    for (_, entries, _) in catalog(false).await? {
        if let Some(e) = entries.into_iter().find(|e| e.key == key) {
            return Ok(e);
        }
    }
    bail!("That plugin is not in your sources any more. Refresh the list.")
}

// ---------------------------------------------------------------------------------------------
// Versions

async fn gh_api(client: &reqwest::Client, path: &str) -> Result<reqwest::Response> {
    let mut req = client.get(format!("https://api.github.com{path}")).header("Accept", "application/vnd.github+json");
    if let Ok(t) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("MP_GITHUB_TOKEN")) {
        if !t.trim().is_empty() {
            req = req.bearer_auth(t.trim());
        }
    }
    Ok(req.send().await?)
}

/// What can be installed: the releases (newest first; the newest full release recommended) and
/// the default branch's newest commit.
pub async fn github_versions(owner: &str, repo: &str) -> Result<Vec<Version>> {
    let client = http()?;
    let mut out = Vec::new();
    let r = gh_api(&client, &format!("/repos/{owner}/{repo}/releases?per_page=30")).await?;
    if r.status() == reqwest::StatusCode::FORBIDDEN {
        bail!("GitHub is limiting requests from this computer for now. Try again later, or set GITHUB_TOKEN.");
    }
    let releases: Vec<Value> = if r.status().is_success() { r.json().await? } else { vec![] };
    let mut recommended_done = false;
    for x in releases.iter().filter(|x| x["draft"] != json!(true)) {
        let pre = x["prerelease"] == json!(true);
        let tag = x["tag_name"].as_str().unwrap_or_default().to_string();
        let title = x["name"].as_str().filter(|n| !n.trim().is_empty()).unwrap_or(&tag);
        let rec = !pre && !recommended_done;
        recommended_done |= rec;
        out.push(Version {
            label: if title == tag { tag.clone() } else { format!("{title} ({tag})") },
            kind: if pre { "prerelease" } else { "release" }.into(),
            git_ref: tag,
            published_at: x["published_at"].as_str().map(str::to_string),
            notes: x["body"].as_str().map(|b| b.chars().take(400).collect()),
            recommended: rec,
        });
    }
    let info = gh_api(&client, &format!("/repos/{owner}/{repo}")).await?;
    let branch = if info.status().is_success() { info.json::<Value>().await?["default_branch"].as_str().unwrap_or("main").to_string() } else { "main".into() };
    out.push(Version {
        label: format!("{branch}: the newest commit"),
        kind: "branch".into(),
        git_ref: branch,
        published_at: None,
        notes: Some("Whatever is on the branch now, released or not. Updates follow the branch.".into()),
        recommended: !recommended_done,
    });
    Ok(out)
}

/// The versions of a catalog entry.
pub async fn versions(key: &str) -> Result<Vec<Version>> {
    let e = find_entry(key).await?;
    match e.kind.as_str() {
        "github" => {
            let s = Spec::parse(e.spec.as_deref().unwrap_or_default())?;
            github_versions(&s.owner, &s.repo).await
        }
        _ => {
            let (_, idx) = web_entry(key).await?;
            Ok(idx
                .versions
                .iter()
                .enumerate()
                .map(|(i, v)| Version { label: v.version.clone(), kind: "release".into(), git_ref: v.version.clone(), published_at: v.published_at.clone(), notes: v.notes.clone(), recommended: i == 0 })
                .collect())
        }
    }
}

/// The index entry behind a `web:` key.
async fn web_entry(key: &str) -> Result<(String, IndexEntry)> {
    let rest = key.strip_prefix("web:").context("not a web plugin")?;
    let (loc, id) = rest.rsplit_once('#').context("bad key")?;
    let src = Source { id: String::new(), location: loc.to_string(), name: String::new(), added_at: String::new() };
    let r = http()?.get(&src.location).send().await?.error_for_status()?;
    let idx: Index = r.json().await?;
    let e = idx.plugins.into_iter().enumerate().find(|(i, p)| p.id.clone().unwrap_or_else(|| format!("plugin-{i}")) == id).map(|(_, p)| p).context("That plugin is no longer in its index.")?;
    Ok((loc.to_string(), e))
}

/// Fetch a catalog entry at a version (a tag or branch for GitHub; a listed version for web
/// archives) and check it, ready to show before installing.
pub async fn fetch_entry(key: &str, version: Option<&str>) -> Result<Staged> {
    let e = find_entry(key).await?;
    match e.kind.as_str() {
        "github" => {
            let spec = e.spec.context("no spec")?;
            plugins::fetch(&match version { Some(v) => format!("{spec}@{v}"), None => spec }).await
        }
        _ => {
            let (loc, entry) = web_entry(key).await?;
            let v = match version {
                Some(want) => entry.versions.iter().find(|x| x.version == want).with_context(|| format!("There is no version {want}."))?,
                None => entry.versions.first().context("It lists no versions.")?,
            };
            let id = entry.id.clone().unwrap_or_default();
            plugins::fetch_archive(&v.url, v.sha256.as_deref(), format!("web:{loc}#{id}"), Some(v.version.clone())).await
        }
    }
}

/// The newest version of an installed plugin from a web index, if it is not the one installed.
pub async fn fetch_web_update(source: &str, installed_version: Option<&str>) -> Result<Option<Staged>> {
    let (_, entry) = web_entry(source).await?;
    let newest = entry.versions.first().context("It lists no versions.")?;
    if Some(newest.version.as_str()) == installed_version {
        return Ok(None);
    }
    let (loc, id) = source.trim_start_matches("web:").rsplit_once('#').context("bad source")?;
    Ok(Some(plugins::fetch_archive(&newest.url, newest.sha256.as_deref(), format!("web:{loc}#{id}"), Some(newest.version.clone())).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizing() {
        assert_eq!(normalize("vltmedia/MultiParity").unwrap(), "github:vltmedia/MultiParity");
        assert_eq!(normalize("https://github.com/a/b").unwrap(), "github:a/b");
        assert_eq!(normalize("https://plugins.example.com/multiparity-plugins.json").unwrap(), "https://plugins.example.com/multiparity-plugins.json");
        assert!(normalize("a/b/c").is_err());
        assert!(normalize("ftp://x").is_err());
    }

    #[test]
    fn index_shape() {
        let i: Index = serde_json::from_value(json!({ "name": "Studio", "plugins": [
            { "path": "plugins/checks" }, { "repo": "a/b" },
            { "id": "farm", "name": "Farm", "versions": [{ "version": "1.0.0", "url": "https://x/farm.zip", "sha256": "ab" }] }
        ] })).unwrap();
        assert_eq!(i.plugins.len(), 3);
        assert_eq!(i.plugins[2].versions[0].version, "1.0.0");
    }
}
