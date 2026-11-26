use tracing_subscriber::EnvFilter;

const HELP: &str = "mp-server: the MultiParity server

  mp-server                     run the server (settings from the environment, see .env.example)
  mp-server backup-now          take one backup now (needs MP_BACKUP_TARGET) and exit
  mp-server backups             list the backups on the backup target
  mp-server restore [options]   restore a backup into this server's empty database and storage
      --from <spec>             where the backups are (default: MP_BACKUP_TARGET), e.g. mount:/data/backups
      --snapshot <id>|latest    which backup (default: latest)
      --db-only                 restore the database but not the file data
      --force                   replace everything in a database that is not empty
";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=warn,sqlx=warn")))
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = mp_server::Config::from_env()?;
    match args.first().map(String::as_str) {
        None | Some("serve") => mp_server::serve(config).await,
        Some("backup-now") => {
            let cfg = config.backup.clone().ok_or_else(|| anyhow::anyhow!("Set MP_BACKUP_TARGET to say where backups go."))?;
            let (_, state) = mp_server::build_app(config).await?;
            mp_server::backup::run_recorded(&state, &cfg, None).await?;
            println!("Backup done.");
            Ok(())
        }
        Some("backups") => {
            let cfg = config.backup.clone().ok_or_else(|| anyhow::anyhow!("Set MP_BACKUP_TARGET to say where backups are."))?;
            let target = mp_server::store::BlobStore::open(&cfg.target).await?;
            for m in mp_server::backup::snapshots(&target).await? {
                println!("{}  {}  {} tables, {} file parts", m.id, m.created_at.format("%Y-%m-%d %H:%M UTC"), m.tables.len(), m.chunks);
            }
            Ok(())
        }
        Some("restore") => {
            let mut from = None;
            let mut snapshot = None;
            let (mut db_only, mut force) = (false, false);
            let mut it = args.iter().skip(1);
            while let Some(a) = it.next() {
                match a.as_str() {
                    "--from" => from = it.next().cloned(),
                    "--snapshot" => snapshot = it.next().cloned(),
                    "--db-only" => db_only = true,
                    "--force" => force = true,
                    other => anyhow::bail!("unknown option {other}\n\n{HELP}"),
                }
            }
            let from = match from {
                Some(spec) => mp_server::store::parse_spec("backup", &spec)?,
                None => config.backup.clone().map(|b| b.target).ok_or_else(|| anyhow::anyhow!("Say where the backups are with --from, or set MP_BACKUP_TARGET."))?,
            };
            mp_server::backup::restore(&config, mp_server::backup::RestoreOptions { from, snapshot, db_only, force }).await
        }
        Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown command {other}\n\n{HELP}"),
    }
}
