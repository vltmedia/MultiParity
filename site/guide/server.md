# Run a server

You need a machine with Docker (Linux, or Docker Desktop on Windows or macOS) and room for your
projects' files. Everything runs in containers: the server, its Postgres database, and
optionally HTTPS (Caddy) and S3-compatible storage (MinIO).

## Start it

```bash
git clone https://github.com/vltmedia/MultiParity.git
cd MultiParity
cp .env.example .env
```

Edit `.env` and set at least:

| Setting | What to put |
|---|---|
| `MP_ADMIN_EMAIL`, `MP_ADMIN_PASSWORD` | The first server admin (password of 10 or more characters) |
| `POSTGRES_PASSWORD` | Any long random password, for the database |
| `MP_PUBLIC_URL` | The address people will use, like `http://studio-server:18500` or `https://mp.yourstudio.com` |

Then:

```bash
docker compose up -d
```

Open `MP_PUBLIC_URL` and sign in as the admin. The first organization is made for you. From
**Admin** add projects and invite people: each invite is a link (emailed for you once
[email](/docs/EMAIL) is set up).

## Where files are kept

`MP_STORAGE` in `.env` chooses where file data goes:

| `MP_STORAGE` | Where |
|---|---|
| `local` | A Docker volume on the server (the default) |
| `mount` | A folder or network share on the host (`MP_BLOB_DIR`). The server refuses to start if it is missing, rather than filling the wrong disk. |
| `s3` | Any S3-compatible service: AWS, Backblaze B2, Cloudflare R2, Wasabi, MinIO. `docker compose --profile s3 up -d` starts a bundled MinIO. |

You can add more named locations (`MP_STORAGE_LOCATIONS`), for example a NAS for active projects
and a bucket for finished ones, and move a project between them from **Admin > Projects** while
people keep working.

## Next steps

- **[Backups](/docs/BACKUP)**: nightly backups of the database and all file data, with a tested
  restore. Point `MP_BACKUP_TARGET` at a second drive or a bucket elsewhere.
- **[Working remotely](/docs/REMOTE)**: Tailscale, a Cloudflare tunnel, or built-in HTTPS with
  Caddy (`docker compose --profile https up -d`).
- **[Email](/docs/EMAIL)**: invites, "forgot password" links and notifications through SMTP or
  SendGrid.
- **[Webhooks and scripts](/docs/AUTOMATION)**: send events to chat, a build server or your own
  scripts.
- **Looks**: admins can give an organization or a project its own theme for light and dark mode
  (the organization's admin page, and **Look…** in Admin > Projects).

## Updating

```bash
git pull
docker compose up -d --build
```

The database is updated automatically when the new server starts. Take a backup first
(**Admin > Fixing > Back up now**).
