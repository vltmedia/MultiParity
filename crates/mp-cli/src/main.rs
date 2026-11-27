//! `mp`: the MultiParity command line. Every command takes `--json` for scripts.
//!
//! Perforce words work as aliases: `submit` = publish, `sync` = get-latest, `edit` = claim,
//! `revert` = discard, `shelve` = draft save.



use std::path::PathBuf;

mod automate;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use mp_client::accounts::{Account, Accounts};
use mp_client::api::{conflict_details, Api};
use mp_client::ops;
use mp_client::scan;
use mp_client::types::*;
use mp_client::workspace::{Registry, Workspace};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(name = "mp", version, about = "MultiParity: version control for game and art teams", propagate_version = true)]
struct Cli {
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    /// Folder to act in (defaults to the current folder).
    #[arg(short = 'C', long = "dir", global = true)]
    dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Sign in to a server. You can be signed in to several accounts at once.
    Login {
        server: String,
        #[arg(long)]
        email: Option<String>,
        /// Read the password from this environment variable instead of asking.
        #[arg(long, env = "MP_PASSWORD", hide_env_values = true)]
        password: Option<String>,
        /// A name for this account, such as "work" or "freelance".
        #[arg(long)]
        label: Option<String>,
    },
    /// Sign out of an account (other accounts and your files are left alone).
    Logout { account: String },
    /// List signed-in accounts.
    Accounts,
    /// List projects from every signed-in account.
    Projects {
        #[arg(long)]
        account: Option<String>,
    },
    /// Connect a project to a folder. Files already in the folder are kept, never deleted.
    Connect {
        /// Project name, short name or id.
        project: String,
        /// Folder (created if missing). Defaults to a new folder named after the project.
        folder: Option<PathBuf>,
        #[arg(long)]
        account: Option<String>,
        #[arg(long, default_value = "Main")]
        branch: String,
        /// Only bring these folders to this computer (repeatable).
        #[arg(long = "only")]
        only: Vec<String>,
        /// Leave out these folders (repeatable).
        #[arg(long = "skip")]
        skip: Vec<String>,
        /// Connect without downloading what is missing.
        #[arg(long)]
        no_download: bool,
        /// Show what would happen and change nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// List folders connected on this computer.
    Workspaces,
    /// Show new, edited and deleted files.
    Status {
        paths: Vec<String>,
    },
    /// Bring this folder up to date. Files you have edited are kept.
    #[command(alias = "sync", alias = "get")]
    GetLatest,
    /// Publish your new, edited and deleted files as one change.
    #[command(alias = "submit")]
    Publish {
        #[arg(short, long)]
        message: String,
        /// Only these files or folders.
        paths: Vec<String>,
        /// Publish your version of these files over a newer one on the server (after get latest).
        #[arg(long = "keep-mine")]
        keep_mine: Vec<String>,
    },
    /// Claim files so nobody else can publish them.
    #[command(alias = "edit", alias = "lock")]
    Claim { paths: Vec<String> },
    /// Give claims back without publishing.
    #[command(alias = "unlock")]
    Release { paths: Vec<String> },
    /// Who has what claimed in this project.
    Claims,
    /// Throw away local edits (your current files are checkpointed first).
    #[command(alias = "revert")]
    Discard { paths: Vec<String> },
    /// Save a private checkpoint of your unpublished work on this computer.
    Checkpoint {
        #[arg(short, long)]
        message: Option<String>,
        /// Include render output and caches, which checkpoints normally leave out.
        #[arg(long)]
        all: bool,
        /// Behave like the background app's automatic checkpoint (obeys the space limit).
        #[arg(long, hide = true)]
        auto: bool,
    },
    /// List checkpoints.
    Checkpoints {
        /// List the ones the server keeps for you, from every computer.
        #[arg(long)]
        server: bool,
    },
    /// Leave files like this out of checkpoints (render output, caches).
    NoCheckpoint {
        /// A pattern as in the ignore list, e.g. "*.exr" or "renders/".
        pattern: Option<String>,
        /// Only for you, not shared with the project.
        #[arg(long)]
        personal: bool,
        /// Say whether checkpoints leave this file out, and why.
        #[arg(long)]
        check: Option<String>,
    },
    /// Move this synced folder somewhere else on this computer, another drive included. Nothing
    /// is downloaded again.
    MoveFolder {
        /// The new place, e.g. D:\Projects\Emberfall (a new or empty folder).
        to: PathBuf,
    },
    /// Disk space MultiParity uses in this folder beyond your files, and free what is not needed.
    Space {
        /// Most space checkpoints may use here: a number of MB, or "auto" for a tenth of the
        /// free disk (the default). Over it, the oldest automatic checkpoints are removed; ones
        /// you made yourself are always kept.
        #[arg(long)]
        limit: Option<String>,
        /// Where this folder keeps checkpoints: "server", "local", or "project" to follow the
        /// project's setting (the default).
        #[arg(long)]
        keep: Option<String>,
        /// Take checkpoints automatically while you work in this folder: "on", "off", or
        /// "project" to follow the project's setting (the default; projects have it off unless
        /// an admin turns it on).
        #[arg(long)]
        auto_checkpoints: Option<String>,
    },
    /// Put files back as they were at a checkpoint.
    Rewind {
        checkpoint: String,
        paths: Vec<String>,
    },
    /// Older versions of a file: list them, put one back, save a copy, or make one the latest.
    Versions {
        path: String,
        /// Put the version from this change into the folder in place of the file. It shows as
        /// edited; publish it to make it the latest for everyone.
        #[arg(long, value_name = "CHANGE")]
        get: Option<i64>,
        /// Save a copy of the version from this change, beside the file (or at --to), leaving
        /// the file alone.
        #[arg(long, value_name = "CHANGE")]
        copy: Option<i64>,
        #[arg(long, requires = "copy")]
        to: Option<PathBuf>,
        /// Make the version from this change the latest on the server now, as a new change.
        #[arg(long, value_name = "CHANGE")]
        make_latest: Option<i64>,
        #[arg(short, long, requires = "make_latest")]
        message: Option<String>,
    },
    /// Drafts: unfinished work saved on the server, visible only to you.
    #[command(subcommand)]
    Draft(DraftCmd),
    /// Branches.
    #[command(subcommand)]
    Branch(BranchCmd),
    /// Bring the parent branch's new changes into this branch.
    Update {
        /// Resolve a clash: path=mine or path=parent (repeatable).
        #[arg(long = "keep")]
        keep: Vec<String>,
    },
    /// Merge this branch into its parent as one change.
    Merge {
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Ignore files: add a rule, check a name, or see the choices for a file.
    Ignore(IgnoreArgs),
    /// Choose which folders are on this computer.
    Folders(FoldersArgs),
    /// Recent changes in this project.
    Log {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
    },
    /// Tags on files and folders.
    #[command(subcommand)]
    Tag(TagCmd),
    /// Administration (people, invites, claims, storage).
    #[command(subcommand)]
    Admin(AdminCmd),
    /// Print events as they happen (one JSON line each), or run a command for each one with the
    /// event as JSON on stdin and in MP_EVENT_* variables.
    Watch {
        /// Only this project (name or id).
        #[arg(long)]
        project: Option<String>,
        /// Only these events, like change.published or change.* (repeatable).
        #[arg(long = "event")]
        events: Vec<String>,
        /// Run this through the shell for each event instead of printing it.
        #[arg(long)]
        exec: Option<String>,
        /// Which signed-in account, when there are several.
        #[arg(long)]
        account: Option<String>,
    },
    /// Serve MultiParity to AI assistants (Claude and others) over the Model Context Protocol, on
    /// stdin and stdout. Add it to an assistant as a command: `mp mcp`.
    Mcp {
        #[arg(long)]
        account: Option<String>,
    },
    /// Run the background daemon: local API for the desktop app, auto-claim, auto-checkpoints.
    Daemon {
        #[arg(long, default_value = "127.0.0.1:18519", env = "MP_DAEMON_LISTEN")]
        listen: String,
        /// Seconds between folder scans.
        #[arg(long, default_value_t = 20)]
        interval: u64,
    },
}

#[derive(Subcommand)]
enum DraftCmd {
    /// Save your unpublished work as a draft on the server.
    #[command(alias = "shelve")]
    Save {
        #[arg(short, long)]
        message: String,
    },
    List,
    /// Put a draft's files back into this folder.
    #[command(alias = "unshelve")]
    Restore { id: String },
    Delete { id: String },
}

#[derive(Subcommand)]
enum BranchCmd {
    List,
    /// Make a branch from the current one (or --from).
    New {
        name: String,
        #[arg(long)]
        from: Option<String>,
        /// Give the branch its own claims instead of sharing them with its parent.
        #[arg(long)]
        own_claims: bool,
    },
    /// Switch this folder to another branch.
    Switch { name: String },
    Archive { name: String },
}

#[derive(Args)]
struct IgnoreArgs {
    /// Pattern to add (gitignore syntax).
    pattern: Option<String>,
    /// Add to your personal list instead of the shared .mpignore.
    #[arg(long)]
    mine: bool,
    /// Say whether a path is ignored, and by which rule.
    #[arg(long)]
    check: Option<String>,
    /// Show the right-click choices for a file.
    #[arg(long)]
    suggest: Option<String>,
    /// Add a starter list: unreal, unity, godot, blender, maya, houdini, photoshop, os.
    #[arg(long)]
    starter: Option<String>,
    /// List what is currently ignored.
    #[arg(long)]
    list: bool,
}

#[derive(Args)]
struct FoldersArgs {
    /// Bring only these folders (replaces the current list). Use "" for everything.
    #[arg(long = "only")]
    only: Option<Vec<String>>,
    /// Leave out these folders (replaces the current list).
    #[arg(long = "skip")]
    skip: Option<Vec<String>>,
}

#[derive(Subcommand)]
enum TagCmd {
    List,
    New {
        name: String,
        #[arg(long)]
        color: Option<String>,
        #[arg(long)]
        group: Option<String>,
    },
    Add { tag: String, paths: Vec<String> },
    Remove { tag: String, paths: Vec<String> },
}

#[derive(Subcommand)]
enum AdminCmd {
    Overview,
    Users {
        #[arg(long)]
        q: Option<String>,
    },
    Suspend {
        email: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Reactivate { email: String },
    /// Invite people to an organization; prints the invite links.
    Invite {
        org: String,
        emails: Vec<String>,
        #[arg(long, default_value = "member")]
        role: String,
    },
    /// Create an organization (server admins).
    NewOrg { name: String },
    /// Create a project in an organization.
    NewProject { org: String, name: String },
    /// Claims older than this many hours.
    StaleClaims {
        #[arg(long, default_value_t = 72)]
        hours: i64,
    },
    /// Verify stored data for a project (or all, for server admins).
    Check {
        #[arg(long)]
        project: Option<String>,
    },
    Audit {
        #[arg(short = 'n', long, default_value_t = 30)]
        limit: usize,
    },
}

fn mb(b: u64) -> String {
    if b >= 1_073_741_824 {
        format!("{:.1} GB", b as f64 / 1_073_741_824.0)
    } else {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_env("MP_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")))
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let json_out = cli.json;
    if let Err(e) = run(cli).await {
        if json_out {
            let details = conflict_details(&e).cloned().unwrap_or(Value::Null);
            println!("{}", json!({ "error": { "message": format!("{e:#}"), "details": details } }));
        } else {
            eprintln!("{}", friendly(&e));
            if let Some(d) = conflict_details(&e) {
                let stale: Vec<&str> = d["stale"].as_array().into_iter().flatten().filter_map(|p| p.as_str()).collect();
                for p in &stale {
                    eprintln!("  changed on the server: {p}");
                }
                if !stale.is_empty() {
                    eprintln!("Run `mp get-latest`. Then either `mp discard <file>` to take theirs, or publish again with --keep-mine <file> to put yours over it.");
                }
                for c in d["claimed"].as_array().into_iter().flatten() {
                    eprintln!("  claimed by {}: {}", c["user"]["name"].as_str().unwrap_or("someone"), c["path"].as_str().unwrap_or_default());
                }
            }
        }
        std::process::exit(1);
    }
}

fn friendly(e: &anyhow::Error) -> String {
    let mut s = e.to_string();
    for cause in e.chain().skip(1) {
        let c = cause.to_string();
        if !s.contains(&c) {
            s.push_str(&format!(": {c}"));
        }
    }
    s
}

struct Ctx {
    json: bool,
    dir: PathBuf,
}

impl Ctx {
    fn out(&self, v: Value, text: impl FnOnce() -> String) {
        if self.json {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        } else {
            let t = text();
            if !t.is_empty() {
                println!("{t}");
            }
        }
    }
    fn ws(&self) -> Result<Workspace> {
        Workspace::open(&Workspace::find(&self.dir)?)
    }
    fn say(&self, s: &str) {
        if !self.json {
            eprintln!("{s}");
        }
    }
}

/// Run a long operation with one live progress line on a terminal (step, percent, amount, speed,
/// time left, file). Elsewhere (a pipe, a log, --json) nothing extra is printed.
async fn with_bar<F: std::future::Future>(show: bool, op: &str, f: F) -> F::Output {
    use std::io::{IsTerminal, Write};
    if !show || !std::io::stderr().is_terminal() {
        return f.await;
    }
    let p = mp_client::progress::Progress::new(op);
    let ticker = {
        let p = p.clone();
        tokio::spawn(async move {
            let mut last = 0usize;
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let s = p.snapshot();
                let wrapping = s.bytes_total > 0 && s.bytes_done >= s.bytes_total;
                let mut line = format!("  {}", if wrapping { "Finishing" } else { s.phase.as_str() });
                if s.bytes_total > 0 && !wrapping {
                    let pct = (s.bytes_done as f64 / s.bytes_total as f64 * 100.0).min(100.0);
                    line += &format!("  {pct:>3.0}%  {} of {}", human(s.bytes_done), human(s.bytes_total));
                    if s.rate > 0 {
                        line += &format!("  {}/s", human(s.rate));
                        let left = (s.bytes_total.saturating_sub(s.bytes_done)) / s.rate.max(1);
                        if left > 2 {
                            line += &format!("  about {} left", if left < 90 { format!("{left} s") } else { format!("{} min", (left + 30) / 60) });
                        }
                    }
                } else if s.files_total > 0 {
                    line += &format!("  {} of {} files", s.files_done, s.files_total);
                }
                if let Some(c) = &s.current {
                    line += &format!("  {}", c.rsplit('/').next().unwrap_or(c));
                }
                let shown: String = line.chars().take(118).collect();
                let pad = last.saturating_sub(shown.chars().count());
                eprint!("\r{shown}{}", " ".repeat(pad));
                let _ = std::io::stderr().flush();
                last = shown.chars().count();
            }
        })
    };
    let out = mp_client::progress::scope(p, f).await;
    ticker.abort();
    eprint!("\r{}\r", " ".repeat(120));
    out
}

pub fn human(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < units.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{bytes} B") } else if v >= 100.0 { format!("{v:.0} {}", units[i]) } else { format!("{v:.1} {}", units[i]) }
}

/// Make user-typed paths project-relative (they may be relative to the current folder).
fn rel_paths(ws: &Workspace, cwd: &std::path::Path, paths: &[String]) -> Vec<String> {
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    paths
        .iter()
        .filter_map(|p| {
            let abs = if std::path::Path::new(p).is_absolute() { PathBuf::from(p) } else { cwd.join(p) };
            let abs = std::fs::canonicalize(&abs).unwrap_or(abs);
            let rel = scan::rel_of(&ws.root, &abs);
            if rel.is_empty() { Some(String::new()) } else { mp_core::paths::normalize(&rel) }
        })
        .collect()
}

async fn run(cli: Cli) -> Result<()> {
    let ctx = Ctx { json: cli.json, dir: cli.dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_default()) };
    match cli.cmd {
        Cmd::Login { server, email, password, label } => {
            let server = if server.starts_with("http://") || server.starts_with("https://") { server } else { format!("https://{server}") };
            let email = match email {
                Some(e) => e,
                None => prompt("Email: ")?,
            };
            let password = match password {
                Some(p) => p,
                None => rpassword::prompt_password("Password: ")?,
            };
            let api = Api::new(&server, None);
            let (token, me) = api.login(&email, &password, &ops::device_name()).await?;
            let mut accounts = Accounts::load()?;
            let id = accounts.upsert(Account { id: uuid::Uuid::new_v4().to_string(), server: api.server.clone(), email: me.email.clone(), name: me.name.clone(), token, label });
            accounts.save()?;
            ctx.out(json!({ "account_id": id, "user": me }), || format!("Signed in as {} on {}.", me.email, api.server));
        }
        Cmd::Logout { account } => {
            let mut accounts = Accounts::load()?;
            let a = accounts.find(&account)?.clone();
            let _ = Api::for_account(&a).post::<_, Value>("/auth/logout", &json!({})).await;
            accounts.accounts.retain(|x| x.id != a.id);
            accounts.save()?;
            ctx.out(json!({ "signed_out": a.id }), || format!("Signed out of {}. Folders that use it stay as they are.", a.display()));
        }
        Cmd::Accounts => {
            let accounts = Accounts::load()?;
            ctx.out(json!(accounts.accounts.iter().map(|a| json!({ "id": a.id, "server": a.server, "email": a.email, "name": a.name, "label": a.label })).collect::<Vec<_>>()), || {
                if accounts.accounts.is_empty() {
                    return "Not signed in. Run `mp login <server>`.".into();
                }
                accounts.accounts.iter().map(|a| format!("{}  {}  {}", &a.id[..8], a.display(), a.name)).collect::<Vec<_>>().join("\n")
            });
        }
        Cmd::Projects { account } => {
            let accounts = Accounts::load()?;
            let list: Vec<&Account> = match &account {
                Some(k) => vec![accounts.find(k)?],
                None => accounts.accounts.iter().collect(),
            };
            let mut all = Vec::new();
            for a in list {
                match Api::for_account(a).projects().await {
                    Ok(ps) => all.extend(ps.into_iter().map(|p| (a.clone(), p))),
                    Err(e) => ctx.say(&format!("{}: {}", a.display(), friendly(&e))),
                }
            }
            let local = Registry::workspaces()?.into_iter().filter_map(|r| Workspace::open(&r).ok()).collect::<Vec<_>>();
            ctx.out(
                json!(all.iter().map(|(a, p)| json!({ "account": a.email, "server": a.server, "project": p,
                    "folders": local.iter().filter(|w| w.config.project_id == p.id).map(|w| w.root.display().to_string()).collect::<Vec<_>>() })).collect::<Vec<_>>()),
                || {
                    let mut lines = Vec::new();
                    let mut last = String::new();
                    for (a, p) in &all {
                        let head = format!("{} / {}", a.display(), p.org.name);
                        if head != last {
                            lines.push(format!("\n{head}"));
                            last = head;
                        }
                        let here = local.iter().find(|w| w.config.project_id == p.id).map(|w| format!("  on this computer: {}", w.root.display())).unwrap_or_default();
                        lines.push(format!("  {:<28} change {:<6} {:>9}  {} claims{}", p.name, p.head_change, human(p.size as u64), p.claims, here));
                    }
                    if lines.is_empty() { "No projects. Ask an admin to add you to one.".into() } else { lines.join("\n").trim_start().to_string() }
                },
            );
        }
        Cmd::Connect { project, folder, account, branch, only, skip, no_download, dry_run } => {
            let accounts = Accounts::load()?;
            let (acct, proj) = find_project(&accounts, account.as_deref(), &project).await?;
            let api = Api::for_account(&acct);
            let detail = api.project(&proj.id).await?;
            let br = detail
                .branches_list
                .iter()
                .find(|b| b.name.eq_ignore_ascii_case(&branch) || b.id == branch)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{} has no branch called {branch}.", proj.name))?;
            let folder = folder.unwrap_or_else(|| ctx.dir.join(&proj.name));
            if !dry_run {
                ctx.say(&format!("Connecting {} to {} …", proj.name, folder.display()));
            }
            let (ws, report) = with_bar(
                !ctx.json && !dry_run,
                "connect",
                ops::connect(
                    &api,
                    &folder,
                    ops::ConnectOptions { account_id: &acct.id, project: &detail, branch: &br, include: only, exclude: skip, download: !no_download, dry_run },
                ),
            )
            .await?;
            ctx.out(json!({ "report": report, "root": ws.as_ref().map(|w| w.root.display().to_string()) }), || {
                let mut s = vec![format!(
                    "{} match the server. {} differ and were kept as Edited. {} are only on this computer and show as New. {} ignored.",
                    report.matched,
                    report.different.len(),
                    report.local_only,
                    report.ignored
                )];
                if dry_run {
                    s.push(format!("{} files ({}) would be downloaded. Nothing was changed.", report.to_download, human(report.download_bytes)));
                } else if no_download {
                    s.push(format!("{} files ({}) are on the server but not here; run `mp get-latest` to download them.", report.to_download, human(report.download_bytes)));
                } else {
                    s.push(format!("Downloaded {} files ({}).", report.downloaded, human(report.download_bytes)));
                }
                s.join("\n")
            });
        }
        Cmd::Workspaces => {
            let list: Vec<Workspace> = Registry::workspaces()?.into_iter().filter_map(|r| Workspace::open(&r).ok()).collect();
            ctx.out(json!(list.iter().map(|w| json!({ "root": w.root, "config": w.config })).collect::<Vec<_>>()), || {
                list.iter()
                    .map(|w| format!("{}  {} / {} on {}  (as {}, change {})", w.root.display(), w.config.org_name, w.config.project_name, w.config.branch_name, w.config.account_email, w.config.synced_change))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Cmd::Status { paths } => {
            let ws = ctx.ws()?;
            let only = rel_paths(&ws, &ctx.dir, &paths);
            let st = scan::status(&ws, &only)?;
            persist_touched(&ws, &st);
            ctx.out(json!({ "branch": ws.config.branch_name, "synced_change": ws.config.synced_change, "status": st }), || {
                let mut s = vec![format!("{} / {} on {}, up to change {}", ws.config.org_name, ws.config.project_name, ws.config.branch_name, ws.config.synced_change)];
                if st.is_clean() {
                    s.push("Nothing to publish.".into());
                }
                for f in &st.new {
                    s.push(format!("  new      {}  ({})", f.path, human(f.size)));
                }
                for f in &st.edited {
                    s.push(format!("  edited   {}  ({})", f.path, human(f.size)));
                }
                for f in &st.deleted {
                    s.push(format!("  deleted  {}", f.path));
                }
                if !st.ignored.is_empty() {
                    s.push(format!("{} ignored (mp ignore --list to see them)", st.ignored.len()));
                }
                s.join("\n")
            });
        }
        Cmd::GetLatest => {
            let mut ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let json_mode = ctx.json;
            let live = !json_mode && std::io::IsTerminal::is_terminal(&std::io::stderr());
            let r = with_bar(!json_mode, "get-latest", ops::get_latest(&api, &mut ws, |p| {
                if !json_mode && !live {
                    eprintln!("  getting {p}");
                }
            }))
            .await?;
            ctx.out(json!(r), || {
                let mut s = vec![format!("Up to date with change {}. {} downloaded ({}), {} removed.", r.head, r.downloaded.len(), human(r.bytes), r.removed.len())];
                for p in &r.kept_yours {
                    s.push(format!("  kept your version of {p} (it also changed on the server; publish to see if it clashes)"));
                }
                s.join("\n")
            });
        }
        Cmd::Publish { message, paths, keep_mine } => {
            let mut ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let only = rel_paths(&ws, &ctx.dir, &paths);
            let keep = rel_paths(&ws, &ctx.dir, &keep_mine);
            let json_mode = ctx.json;
            let live = !json_mode && std::io::IsTerminal::is_terminal(&std::io::stderr());
            let r = with_bar(!json_mode, "publish", ops::publish(&api, &mut ws, &message, &only, &keep, |p| {
                if !json_mode && !live {
                    eprintln!("  {p}");
                }
            }))
            .await?;
            ctx.out(json!(r), || format!("Published change {}: {} file(s), {} sent.", r.change.number, r.files, human(r.uploaded_bytes)));
        }
        Cmd::Claim { paths } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            let (claimed, held) = api.claim(&ws.config.project_id, &ws.config.branch_id, &rel, &ops::device_name()).await?;
            ctx.out(json!({ "claimed": claimed, "held": held }), || {
                let mut s: Vec<String> = claimed.iter().map(|p| format!("Claimed {p}")).collect();
                s.extend(held.iter().map(|c| format!("{} is claimed by {}. Ask them to release it.", c.path, c.user.as_ref().map(|u| u.name.as_str()).unwrap_or("someone"))));
                s.join("\n")
            });
        }
        Cmd::Release { paths } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            let released = api.release(&ws.config.project_id, &ws.config.branch_id, &rel).await?;
            ctx.out(json!({ "released": released }), || if released.is_empty() { "You had none of those claimed.".into() } else { format!("Released {}.", released.join(", ")) });
        }
        Cmd::Claims => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let claims = api.claims(&ws.config.project_id).await?;
            ctx.out(json!(claims), || {
                if claims.is_empty() {
                    return "Nobody has anything claimed.".into();
                }
                let mut s = Vec::new();
                for (who, list) in ops::claims_by_holder(&claims) {
                    s.push(format!("{who}:"));
                    for c in list {
                        s.push(format!("  {}  since {}", c.path, &c.created_at[..16.min(c.created_at.len())]));
                    }
                }
                s.join("\n")
            });
        }
        Cmd::Discard { paths } => {
            let mut ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            if rel.is_empty() {
                anyhow::bail!("Name the files or folders to discard (use . for everything here).");
            }
            let (cp, n) = ops::discard(&api, &mut ws, &rel).await?;
            ctx.out(json!({ "checkpoint": cp.as_ref().map(|c| &c.id), "files": n }), || match cp {
                Some(c) => format!("Discarded {n} file(s). Your versions are in checkpoint {} (mp rewind {} to undo).", c.id, c.id),
                None => "Nothing to discard.".into(),
            });
        }
        Cmd::Checkpoint { message, all, auto } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws).ok();
            let mut opts = if auto { mp_client::checkpoints::CheckpointOptions::auto() } else { mp_client::checkpoints::CheckpointOptions::manual(message.as_deref()) };
            opts.include_all = all;
            let out = mp_client::checkpoints::checkpoint(api.as_ref(), &ws, opts).await?;
            if let (Some(t), false) = (&out.too_big, auto) {
                anyhow::bail!(
                    "This checkpoint needs {} and this computer has {} to spare. Save a draft instead (mp draft save -m ...) to keep the work on the server.",
                    mb(t.needed_bytes),
                    mb(t.available_bytes)
                );
            }
            ctx.out(json!(out), || {
                let left = if out.left_out.is_empty() { String::new() } else { format!(" Left out {} render or cache file(s) ({}); --all includes them.", out.left_out.len(), mb(out.left_out_bytes)) };
                match &out.checkpoint {
                    Some(c) => format!("Saved checkpoint {} ({} files{}).{left}", c.id, c.files.len(), if c.on_server { ", kept on the server" } else { "" }),
                    None if !out.left_out.is_empty() => format!("Nothing to checkpoint except render output and caches.{left}"),
                    None => "Nothing to checkpoint: no unpublished changes.".into(),
                }
            });
        }
        Cmd::Checkpoints { server } => {
            let ws = ctx.ws()?;
            if server {
                let api = ops::api_for(&ws)?;
                let list = mp_client::checkpoints::list_on_server(&api, &ws).await?;
                ctx.out(json!(list), || {
                    if list.is_empty() {
                        return "The server keeps no checkpoints of yours for this project.".into();
                    }
                    list.iter()
                        .map(|c| {
                            format!(
                                "{}  {:>3} files  {}  {}{}",
                                c["id"].as_str().unwrap_or_default(),
                                c["file_count"],
                                c["device"].as_str().unwrap_or("-"),
                                c["note"].as_str().unwrap_or_default(),
                                if c["auto"].as_bool().unwrap_or(false) { " (automatic)" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            } else {
                let list = ops::list_checkpoints(&ws)?;
                ctx.out(json!(list), || {
                    if list.is_empty() {
                        return "No checkpoints yet.".into();
                    }
                    list.iter()
                        .map(|c| {
                            format!(
                                "{}  {:>3} files  {}{}{}",
                                c.id,
                                c.files.len(),
                                c.note.clone().unwrap_or_default(),
                                if c.auto { " (automatic)" } else { "" },
                                if c.on_server { " (on the server)" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                });
            }
        }
        Cmd::NoCheckpoint { pattern, personal, check } => {
            let ws = ctx.ws()?;
            if let Some(path) = check {
                let rel = rel_paths(&ws, &ctx.dir, &[path]).into_iter().next().unwrap_or_default();
                let rule = mp_client::checkpoints::no_checkpoint_rule(&ws, &rel)?;
                ctx.out(json!({ "path": rel, "left_out": rule.is_some(), "rule": rule }), || match &rule {
                    Some(r) => format!("Checkpoints leave {rel} out: {r}"),
                    None => format!("Checkpoints include {rel}."),
                });
            } else if let Some(pattern) = pattern {
                let file = mp_client::checkpoints::add_no_checkpoint(&ws, &pattern, personal)?;
                ctx.out(json!({ "pattern": pattern, "file": file.display().to_string() }), || {
                    if personal {
                        format!("Checkpoints on this computer now leave out {pattern}.")
                    } else {
                        format!("Checkpoints now leave out {pattern}. Publish .mpnocheckpoint to share the rule with the project.")
                    }
                });
            } else {
                let list: Vec<&str> = mp_client::scan::DEFAULT_NO_CHECKPOINT.to_vec();
                let shared = std::fs::read_to_string(ws.root.join(mp_client::scan::NO_CHECKPOINT_FILE)).unwrap_or_default();
                ctx.out(json!({ "built_in": list, "project": shared.lines().collect::<Vec<_>>() }), || {
                    format!("Built in: {}\nThis project (.mpnocheckpoint):\n{}", list.join("  "), if shared.trim().is_empty() { "  (nothing added)" } else { shared.trim_end() })
                });
            }
        }
        Cmd::MoveFolder { to } => {
            let ws = ctx.ws()?;
            let to = if to.is_absolute() { to } else { ctx.dir.join(to) };
            let quiet = ctx.json;
            let mut last = 0u64;
            let r = ops::move_folder(&ws, &to, |done, total| {
                if !quiet && total > 0 && done - last > total / 20 {
                    last = done;
                    eprintln!("  copied {} of {}", mb(done), mb(total));
                }
            })?;
            ctx.out(json!(r), || {
                let how = if r.method == "renamed" { "Moved".to_string() } else { format!("Copied {} files ({}) and removed the old folder", r.files, mb(r.bytes)) };
                let mut s = format!("{how}: {} is now at {}.", ws.config.project_name, r.to);
                if let Some(left) = &r.old_folder_left {
                    s.push_str(&format!("\n{left}"));
                }
                s
            });
        }
        Cmd::Space { limit, keep, auto_checkpoints } => {
            let mut ws = ctx.ws()?;
            let mut changed = false;
            if let Some(l) = limit {
                ws.config.checkpoint_limit_mb = match l.as_str() {
                    "auto" => None,
                    n => Some(n.parse().map_err(|_| anyhow::anyhow!("Give the limit as a number of MB, or auto."))?),
                };
                changed = true;
            }
            if let Some(k) = keep {
                ws.config.checkpoint_storage = match k.as_str() {
                    "project" => None,
                    "server" | "local" => Some(k),
                    _ => anyhow::bail!("Keep checkpoints on: server, local, or project (follow the project's setting)."),
                };
                changed = true;
            }
            if let Some(a) = auto_checkpoints {
                ws.config.auto_checkpoints = match a.as_str() {
                    "project" => None,
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => anyhow::bail!("Automatic checkpoints can be: on, off, or project (follow the project's setting)."),
                };
                changed = true;
            }
            if changed {
                ws.save()?;
            }
            let api = ops::api_for(&ws).ok();
            if let Some(api) = &api {
                let _ = mp_client::checkpoints::refresh_project_settings(api, &mut ws).await;
            }
            let r = ops::trim_cache(api.as_ref(), &ws).await?;
            ctx.out(json!(r), || {
                let mut s = Vec::new();
                if r.storage == "server" {
                    s.push(format!("This folder keeps checkpoints on the server ({} there, {} here).", r.on_server, r.on_this_computer));
                }
                s.push(if r.automatic { "Checkpoints are taken automatically while you work here.".into() } else { "Checkpoints are taken only when you save one (automatic checkpoints are off).".to_string() });
                s.push(format!(
                    "Checkpoints use {} here; the limit is {}{}.",
                    mb(r.kept_bytes),
                    mb(r.limit_bytes),
                    if r.limit_is_automatic { " (a tenth of the free disk)" } else { "" }
                ));
                if r.freed_parts > 0 {
                    s.push(format!("Freed {}.", mb(r.freed_bytes)));
                }
                if !r.removed_checkpoints.is_empty() {
                    s.push(format!("Removed {} old automatic checkpoint(s) to stay under the limit.", r.removed_checkpoints.len()));
                }
                if r.safety_bytes > 0 || r.safety_freed_bytes > 0 {
                    s.push(format!("Files moved aside instead of being overwritten use {} (kept 30 days){}.", mb(r.safety_bytes), if r.safety_freed_bytes > 0 { format!("; removed {} older than that", mb(r.safety_freed_bytes)) } else { String::new() }));
                }
                if r.low_disk {
                    s.push(format!("This disk is nearly full: {} free.", mb(r.free_disk_bytes)));
                }
                if !r.checked_server {
                    s.push("Could not reach the server, so only data no checkpoint uses was freed.".into());
                }
                s.join("\n")
            });
        }
        Cmd::Rewind { checkpoint, paths } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            let (before, n) = ops::rewind(&api, &ws, &checkpoint, &rel).await?;
            ctx.out(json!({ "files": n, "undo": before.as_ref().map(|c| &c.id) }), || {
                format!("Rewound {n} file(s) to {checkpoint}.{}", before.map(|c| format!(" To undo: mp rewind {}", c.id)).unwrap_or_default())
            });
        }
        Cmd::Versions { path, get, copy, to, make_latest, message } => {
            let mut ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let rel = rel_paths(&ws, &ctx.dir, &[path.clone()]).pop().filter(|r| !r.is_empty()).with_context(|| format!("{path} is not a file in this folder."))?;
            let picked = [get.is_some(), copy.is_some(), make_latest.is_some()].iter().filter(|x| **x).count();
            anyhow::ensure!(picked <= 1, "Use one of --get, --copy and --make-latest at a time.");
            if let Some(n) = get {
                let r = ops::bring_back_version(&api, &ws, &rel, n).await?;
                ctx.out(json!(r), || {
                    let mut s = format!("Put back {rel} as of change {n}. It shows as edited: publish it to make it the latest for everyone.");
                    if let Some(c) = &r.checkpoint {
                        s.push_str(&format!("\nYour version is in checkpoint {}: mp rewind {} {rel}", c.id, c.id));
                    }
                    s
                });
            } else if let Some(n) = copy {
                let dest = ops::save_version_copy(&api, &ws, &rel, n, to.as_deref()).await?;
                ctx.out(json!({ "path": rel, "change": n, "saved_to": dest }), || format!("Saved {rel} as of change {n} to {}.", dest.display()));
            } else if let Some(n) = make_latest {
                let c = api.restore_version(&ws.config.project_id, &ws.config.branch_id, &rel, n, message.as_deref()).await?;
                // Bring this folder up to date with it, unless the file has edits here.
                let synced = ops::get_latest(&api, &mut ws, |_| {}).await;
                ctx.out(json!({ "change": c, "got_latest": synced.is_ok() }), || {
                    let kept = synced.as_ref().map(|r| r.kept_yours.iter().any(|p| p == &rel)).unwrap_or(false);
                    format!("Change {}: {rel} as of change {n} is the latest again.{}", c.number, if kept { " Your folder keeps your edited copy; discard it to get this version." } else { "" })
                });
            } else {
                let list = ops::versions(&api, &ws, &rel).await?;
                ctx.out(json!(list), || {
                    if list.is_empty() {
                        return format!("{rel} has not been published yet.");
                    }
                    let mut s = String::new();
                    for v in &list {
                        let who = v.author.as_ref().map(|a| a.name.as_str()).unwrap_or("someone");
                        let what = match v.op.as_str() {
                            "delete" => "deleted".to_string(),
                            _ => human(v.size as u64),
                        };
                        let on = if v.branch_id != ws.config.branch_id { format!(" [{}]", v.branch_name) } else { String::new() };
                        s.push_str(&format!("{:>6}  {}  {:<10}  {}  {}{}{}\n", v.change, v.created_at.get(..16).unwrap_or(&v.created_at).replace('T', " "), what, who, v.message, on, if v.current { "  (latest)" } else { "" }));
                    }
                    s.push_str(&format!("Put one back with: mp versions {path} --get <change>"));
                    s
                });
            }
        }
        Cmd::Draft(d) => draft(&ctx, d).await?,
        Cmd::Branch(b) => branch(&ctx, b).await?,
        Cmd::Update { keep } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let resolutions: serde_json::Map<String, Value> = keep
                .iter()
                .filter_map(|k| k.rsplit_once('=').map(|(p, v)| (p.to_string(), json!(v))))
                .collect();
            let v: Value = api.post(&format!("/projects/{}/branches/{}/update", ws.config.project_id, ws.config.branch_id), &json!({ "resolutions": resolutions })).await?;
            let report: MergeReport = serde_json::from_value(v.clone())?;
            ctx.out(v.clone(), || {
                if !report.conflicts.is_empty() {
                    let mut s = vec![format!("{} file(s) changed on both branches and need a choice. Nothing was changed.", report.conflicts.len())];
                    for c in &report.conflicts {
                        s.push(format!("  {} ({})   choose with --keep \"{}=mine\" or --keep \"{}=parent\"", c.path, c.kind, c.path, c.path));
                    }
                    return s.join("\n");
                }
                match v["change"]["number"].as_i64() {
                    Some(n) => format!("Updated: change {n}. {} file(s) came in, {} merged automatically. Run `mp get-latest` to bring them to this folder.", report.only_parent, report.auto_merged.len()),
                    None => "Already up to date with the parent branch.".into(),
                }
            });
        }
        Cmd::Merge { message } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let v: Value = api.post(&format!("/projects/{}/branches/{}/merge", ws.config.project_id, ws.config.branch_id), &json!({ "message": message })).await?;
            ctx.out(v.clone(), || format!("Merged {} as change {}.", ws.config.branch_name, v["change"]["number"]));
        }
        Cmd::Ignore(a) => ignore_cmd(&ctx, a).await?,
        Cmd::Folders(a) => {
            let mut ws = ctx.ws()?;
            if a.only.is_none() && a.skip.is_none() {
                let tree = ops::folder_tree(&ws);
                ctx.out(json!({ "include": ws.config.include, "exclude": ws.config.exclude, "tree": tree }), || {
                    let mut s = vec![format!(
                        "Only: {}\nSkip: {}",
                        if ws.config.include.is_empty() { "everything".into() } else { ws.config.include.join(", ") },
                        if ws.config.exclude.is_empty() { "nothing".into() } else { ws.config.exclude.join(", ") }
                    )];
                    for c in &tree.children {
                        let mark = match c.included { "all" => "[x]", "some" => "[-]", _ => "[ ]" };
                        s.push(format!("  {mark} {:<30} {:>9} on server, {:>9} here", c.name, human(c.size), human(c.local_size)));
                    }
                    s.join("\n")
                });
            } else {
                let api = ops::api_for(&ws)?;
                let include = a.only.unwrap_or_else(|| ws.config.include.clone());
                let exclude = a.skip.unwrap_or_else(|| ws.config.exclude.clone());
                let r = ops::set_scope(&api, &mut ws, include, exclude).await?;
                ctx.out(json!(r), || {
                    let mut s = vec![format!("Downloaded {} file(s), removed {} that matched the server.", r.downloaded, r.removed)];
                    if !r.kept.is_empty() {
                        s.push(format!("Kept {} file(s) with unpublished edits outside the chosen folders.", r.kept.len()));
                    }
                    s.join("\n")
                });
            }
        }
        Cmd::Log { limit } => {
            let ws = ctx.ws()?;
            let api = ops::api_for(&ws)?;
            let list: Vec<Change> = api.get(&format!("/projects/{}/changes?branch={}&limit={limit}", ws.config.project_id, ws.config.branch_id)).await?;
            ctx.out(json!(list), || {
                list.iter()
                    .map(|c| format!("{:<6} {}  {:<14} {}", c.number, &c.created_at[..16.min(c.created_at.len())], c.author.as_ref().map(|a| a.name.as_str()).unwrap_or("-"), c.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        Cmd::Tag(t) => tag_cmd(&ctx, t).await?,
        Cmd::Admin(a) => admin_cmd(&ctx, a).await?,
        Cmd::Watch { project, events, exec, account } => automate::watch(account, project, events, exec).await?,
        Cmd::Mcp { account } => automate::mcp(account).await?,
        Cmd::Daemon { listen, interval } => mp_daemon::run(&listen, interval).await?,
    }
    Ok(())
}

fn persist_touched(ws: &Workspace, st: &scan::Status) {
    if st.touched.is_empty() {
        return;
    }
    if let Ok(mut w) = Workspace::open(&ws.root) {
        for (p, m) in &st.touched {
            if let Some(ix) = w.index.files.get_mut(p) {
                ix.mtime = *m;
            }
        }
        let _ = w.save();
    }
}

fn prompt(q: &str) -> Result<String> {
    use std::io::Write;
    eprint!("{q}");
    std::io::stderr().flush()?;
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

async fn find_project(accounts: &Accounts, account: Option<&str>, key: &str) -> Result<(Account, ProjectSummary)> {
    let list: Vec<&Account> = match account {
        Some(k) => vec![accounts.find(k)?],
        None => accounts.accounts.iter().collect(),
    };
    if list.is_empty() {
        anyhow::bail!("You are not signed in. Run `mp login <server>` first.");
    }
    let mut hits = Vec::new();
    for a in list {
        if let Ok(ps) = Api::for_account(a).projects().await {
            for p in ps {
                if p.id == key || p.slug.eq_ignore_ascii_case(key) || p.name.eq_ignore_ascii_case(key) || format!("{}/{}", p.org.slug, p.slug).eq_ignore_ascii_case(key) {
                    hits.push((a.clone(), p));
                }
            }
        }
    }
    match hits.len() {
        1 => Ok(hits.remove(0)),
        0 => anyhow::bail!("No project called {key} in your accounts. Run `mp projects` to see them."),
        _ => anyhow::bail!("Several projects match {key}. Use org/project or --account."),
    }
}

async fn draft(ctx: &Ctx, d: DraftCmd) -> Result<()> {
    let ws = ctx.ws()?;
    let api = ops::api_for(&ws)?;
    match d {
        DraftCmd::Save { message } => {
            let v = ops::save_draft(&api, &ws, &message).await?;
            ctx.out(v.clone(), || format!("Saved draft \"{}\" on the server ({} files). Only you can see it.", message, v["file_count"]));
        }
        DraftCmd::List => {
            let list: Vec<Value> = api.get(&format!("/projects/{}/drafts", ws.config.project_id)).await?;
            ctx.out(json!(list), || {
                if list.is_empty() {
                    return "No drafts.".into();
                }
                list.iter().map(|d| format!("{}  {:>3} files  {}", d["id"].as_str().unwrap_or(""), d["file_count"], d["title"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")
            });
        }
        DraftCmd::Restore { id } => {
            let n = ops::restore_draft(&api, &ws, &id).await?;
            ctx.out(json!({ "files": n }), || format!("Put {n} file(s) from the draft into this folder. Your previous state is in a checkpoint."));
        }
        DraftCmd::Delete { id } => {
            let _: Value = api.delete(&format!("/projects/{}/drafts/{id}", ws.config.project_id)).await?;
            ctx.out(json!({}), || "Deleted the draft.".into());
        }
    }
    Ok(())
}

async fn branch(ctx: &Ctx, b: BranchCmd) -> Result<()> {
    let mut ws = ctx.ws()?;
    let api = ops::api_for(&ws)?;
    let branches = api.branches(&ws.config.project_id).await?;
    let find = |name: &str| -> Result<Branch> {
        branches
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(name) || b.id == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("There is no branch called {name}. Run `mp branch list`."))
    };
    match b {
        BranchCmd::List => {
            ctx.out(json!(branches), || {
                branches
                    .iter()
                    .filter(|b| !b.archived)
                    .map(|b| {
                        let here = if b.id == ws.config.branch_id { "*" } else { " " };
                        let rel = if b.parent_id.is_some() { format!("  {} to merge, parent has {} new", b.ahead, b.behind) } else { String::new() };
                        format!("{here} {:<24} change {}{}{}", b.name, b.head_change, rel, if b.protected { "  protected" } else { "" })
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        BranchCmd::New { name, from, own_claims } => {
            let from = match from {
                Some(f) => find(&f)?.id,
                None => ws.config.branch_id.clone(),
            };
            let nb: Branch = api.post(&format!("/projects/{}/branches", ws.config.project_id), &json!({ "name": name, "from": from, "shares_claims": !own_claims })).await?;
            ctx.out(json!(nb), || format!("Made branch {}. Switch to it with `mp branch switch {}`.", nb.name, nb.name));
        }
        BranchCmd::Switch { name } => {
            let target = find(&name)?;
            let json_mode = ctx.json;
            let r = ops::switch_branch(&api, &mut ws, &target, |p| {
                if !json_mode {
                    eprintln!("  {p}");
                }
            })
            .await?;
            ctx.out(json!(r), || format!("Switched to {}. {} file(s) changed, {} removed.", target.name, r.downloaded.len(), r.removed.len()));
        }
        BranchCmd::Archive { name } => {
            let target = find(&name)?;
            let _: Value = api.post(&format!("/projects/{}/branches/{}/archive", ws.config.project_id, target.id), &json!({})).await?;
            ctx.out(json!({}), || format!("Archived {}. It stays in history.", target.name));
        }
    }
    Ok(())
}

async fn ignore_cmd(ctx: &Ctx, a: IgnoreArgs) -> Result<()> {
    let ws = ctx.ws()?;
    if let Some(p) = a.check {
        let rel = rel_paths(&ws, &ctx.dir, &[p.clone()]).pop().unwrap_or(p.clone());
        let ign = scan::Ignores::load(&ws.root)?;
        let is_dir = ws.abs(&rel).is_dir();
        let rule = ign.check(&rel, is_dir);
        let tracked = ws.index.files.contains_key(&rel);
        ctx.out(json!({ "path": rel, "ignored": rule.is_some() && !tracked, "rule": rule, "tracked": tracked }), || match (&rule, tracked) {
            (Some(r), false) => format!("{rel} is ignored by {r}."),
            (Some(r), true) => format!("{rel} matches {r}, but it is already published, so it stays tracked."),
            (None, _) => format!("{rel} is not ignored."),
        });
        return Ok(());
    }
    if let Some(p) = a.suggest {
        let rel = rel_paths(&ws, &ctx.dir, &[p.clone()]).pop().unwrap_or(p);
        let choices = scan::ignore_choices(&ws, &rel);
        ctx.out(json!(choices), || {
            choices
                .iter()
                .map(|c| format!("  {:<36} {:<44} {} file(s), {}{}", c.label, c.pattern, c.matches, human(c.bytes), if c.tracked > 0 { format!(", {} already published", c.tracked) } else { String::new() }))
                .collect::<Vec<_>>()
                .join("\n")
        });
        return Ok(());
    }
    if let Some(name) = a.starter {
        let (_, pats) = ops::STARTER_LISTS.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)).ok_or_else(|| {
            anyhow::anyhow!("No starter list called {name}. Choose from: {}.", ops::STARTER_LISTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))
        })?;
        for p in *pats {
            ops::add_ignore(&ws, p, a.mine)?;
        }
        ctx.out(json!({ "added": pats }), || format!("Added the {name} starter list ({} rules).", pats.len()));
        return Ok(());
    }
    if let Some(pattern) = a.pattern {
        let file = ops::add_ignore(&ws, &pattern, a.mine)?;
        ctx.out(json!({ "file": file }), || {
            if a.mine {
                format!("Ignoring {pattern} on this computer only.")
            } else {
                format!("Added {pattern} to .mpignore. It applies to everyone once you publish .mpignore.")
            }
        });
        return Ok(());
    }
    let st = scan::status(&ws, &[])?;
    ctx.out(json!({ "ignored": st.ignored }), || if st.ignored.is_empty() { "Nothing is ignored.".into() } else { st.ignored.join("\n") });
    let _ = a.list;
    Ok(())
}

async fn tag_cmd(ctx: &Ctx, t: TagCmd) -> Result<()> {
    let ws = ctx.ws()?;
    let api = ops::api_for(&ws)?;
    let p = ws.config.project_id.clone();
    let tags: Vec<Value> = api.get(&format!("/projects/{p}/tags")).await?;
    let find = |name: &str| -> Result<String> {
        tags.iter()
            .find(|t| t["name"].as_str().map(|n| n.eq_ignore_ascii_case(name)).unwrap_or(false))
            .and_then(|t| t["id"].as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("No tag called {name}. Create it with `mp tag new {name}`."))
    };
    match t {
        TagCmd::List => ctx.out(json!(tags), || tags.iter().map(|t| format!("{}  {}", t["name"].as_str().unwrap_or(""), t["group"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n")),
        TagCmd::New { name, color, group } => {
            let v: Value = api.post(&format!("/projects/{p}/tags"), &json!({ "name": name, "color": color, "group": group })).await?;
            ctx.out(v, || format!("Made tag {name}."));
        }
        TagCmd::Add { tag, paths } => {
            let id = find(&tag)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            let _: Value = api.post(&format!("/projects/{p}/tags/apply"), &json!({ "tag": id, "paths": rel })).await?;
            ctx.out(json!({}), || format!("Tagged {} file(s) or folder(s) {tag}.", rel.len()));
        }
        TagCmd::Remove { tag, paths } => {
            let id = find(&tag)?;
            let rel = rel_paths(&ws, &ctx.dir, &paths);
            let _: Value = api.post(&format!("/projects/{p}/tags/remove"), &json!({ "tag": id, "paths": rel })).await?;
            ctx.out(json!({}), || format!("Removed {tag}."));
        }
    }
    Ok(())
}

async fn admin_api(ctx: &Ctx) -> Result<Api> {
    // Inside a connected folder use its account; otherwise the only (or first) account.
    if let Ok(ws) = ctx.ws() {
        return ops::api_for(&ws);
    }
    let accounts = Accounts::load()?;
    Ok(Api::for_account(accounts.pick(None).or_else(|_| accounts.accounts.first().context("You are not signed in. Run `mp login <server>` first."))?))
}

async fn admin_cmd(ctx: &Ctx, a: AdminCmd) -> Result<()> {
    let api = admin_api(ctx).await?;
    let user_by_email = |list: &Vec<Value>, email: &str| -> Result<String> {
        list.iter()
            .find(|u| u["email"].as_str().map(|e| e.eq_ignore_ascii_case(email)).unwrap_or(false))
            .and_then(|u| u["id"].as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("No one with the email {email}."))
    };
    let org_by_name = |orgs: &Vec<Value>, key: &str| -> Result<String> {
        orgs.iter()
            .find(|o| o["slug"].as_str() == Some(key) || o["name"].as_str().map(|n| n.eq_ignore_ascii_case(key)).unwrap_or(false) || o["id"].as_str() == Some(key))
            .and_then(|o| o["id"].as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("No organization called {key}."))
    };
    match a {
        AdminCmd::Overview => {
            let v: Value = api.get("/admin/overview").await?;
            ctx.out(v.clone(), || {
                format!(
                    "{} people ({} suspended), {} organizations, {} projects, {} claims ({} older than 3 days), {} open invites{}",
                    v["users"], v["suspended"], v["orgs"], v["projects"], v["active_claims"], v["stale_claims"], v["open_invites"],
                    v["storage_bytes"].as_u64().map(|b| format!(", {} stored", human(b))).unwrap_or_default()
                )
            });
        }
        AdminCmd::Users { q } => {
            let path = match q {
                Some(q) => format!("/admin/users?q={}", q.replace(' ', "%20")),
                None => "/admin/users".into(),
            };
            let list: Vec<Value> = api.get(&path).await?;
            ctx.out(json!(list), || {
                list.iter()
                    .map(|u| format!("{:<28} {:<24} {:<10} {} claims", u["email"].as_str().unwrap_or(""), u["name"].as_str().unwrap_or(""), u["status"].as_str().unwrap_or(""), u["claims"]))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        AdminCmd::Suspend { email, reason } => {
            let list: Vec<Value> = api.get("/admin/users").await?;
            let id = user_by_email(&list, &email)?;
            let v: Value = api.post(&format!("/admin/users/{id}/suspend"), &json!({ "reason": reason })).await?;
            ctx.out(v, || format!("Suspended {email}. They are signed out everywhere; their claims stay."));
        }
        AdminCmd::Reactivate { email } => {
            let list: Vec<Value> = api.get("/admin/users").await?;
            let id = user_by_email(&list, &email)?;
            let v: Value = api.post(&format!("/admin/users/{id}/reactivate"), &json!({})).await?;
            ctx.out(v, || format!("Reactivated {email}."));
        }
        AdminCmd::Invite { org, emails, role } => {
            let orgs: Vec<Value> = api.get("/orgs").await?;
            let id = org_by_name(&orgs, &org)?;
            let list: Vec<Value> = api.post(&format!("/admin/orgs/{id}/invites"), &json!({ "emails": emails, "role": role })).await?;
            ctx.out(json!(list), || list.iter().map(|i| format!("{}  {}", i["email"].as_str().unwrap_or("anyone with the link"), i["url"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n"));
        }
        AdminCmd::NewOrg { name } => {
            let v: Value = api.post("/orgs", &json!({ "name": name })).await?;
            ctx.out(v.clone(), || format!("Made organization {} ({}).", v["name"].as_str().unwrap_or(""), v["slug"].as_str().unwrap_or("")));
        }
        AdminCmd::NewProject { org, name } => {
            let orgs: Vec<Value> = api.get("/orgs").await?;
            let id = org_by_name(&orgs, &org)?;
            let v: Value = api.post(&format!("/orgs/{id}/projects"), &json!({ "name": name })).await?;
            ctx.out(v.clone(), || format!("Made project {}. Connect a folder with `mp connect {}`.", v["name"].as_str().unwrap_or(""), v["slug"].as_str().unwrap_or("")));
        }
        AdminCmd::StaleClaims { hours } => {
            let list: Vec<Value> = api.get(&format!("/admin/claims?older_than_hours={hours}")).await?;
            ctx.out(json!(list), || {
                if list.is_empty() {
                    return format!("No claims older than {hours} hours.");
                }
                list.iter()
                    .map(|c| format!("{:<20} {:<16} {}  ({} h)", c["project"]["name"].as_str().unwrap_or(""), c["user"]["name"].as_str().unwrap_or(""), c["path"].as_str().unwrap_or(""), c["age_hours"]))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
        AdminCmd::Check { project } => {
            let pid = match project {
                Some(k) => Some(api.projects().await?.into_iter().find(|p| p.id == k || p.slug == k || p.name.eq_ignore_ascii_case(&k)).map(|p| p.id).ok_or_else(|| anyhow::anyhow!("No project called {k}."))?),
                None => None,
            };
            let v: Value = api.post("/admin/storage/check", &json!({ "project": pid })).await?;
            ctx.out(v.clone(), || {
                if v["ok"].as_bool() == Some(true) {
                    format!("All {} stored parts are present.", v["checked"])
                } else {
                    format!("{} of {} stored parts are missing. Run with --json for the affected changes.", v["missing_count"], v["checked"])
                }
            });
        }
        AdminCmd::Audit { limit } => {
            let list: Vec<Value> = api.get(&format!("/admin/audit?limit={limit}")).await?;
            ctx.out(json!(list), || {
                list.iter()
                    .map(|e| format!("{}  {:<14} {:<24} {}", &e["created_at"].as_str().unwrap_or("")[..16.min(e["created_at"].as_str().unwrap_or("").len())], e["actor"]["name"].as_str().unwrap_or("-"), e["kind"].as_str().unwrap_or(""), e["data"]))
                    .collect::<Vec<_>>()
                    .join("\n")
            });
        }
    }
    Ok(())
}
