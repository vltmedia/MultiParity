# MultiParity

Self-hosted version control for game and art teams: a friendlier Perforce. Big binary files,
claims instead of merge conflicts, branches that stay simple, and apps that artists can use
without learning version control. Unlimited users; deploy it with Docker Compose.

| Word | What it means | Perforce |
|---|---|---|
| Claim / Release | Reserve a file so nobody else can publish it / give it back | Check out / unlock |
| Publish | Send your edits to the server as one numbered change | Submit |
| Get latest | Bring your folder up to date | Sync |
| Checkpoint | A private save point on your computer | (Git commit) |
| Save draft | Unfinished work saved on the server, only for you | Shelve |
| Discard / Rewind | Throw away edits / go back to a checkpoint | Revert |
| Branch, Update from Main, Merge into Main | Parallel work and bringing it back | Streams |

The full product spec is [docs/SPEC.md](docs/SPEC.md); the HTTP API is [docs/API.md](docs/API.md).

## What is in this repository

| Path | What it is |
|---|---|
| `crates/mp-core` | Chunking (FastCDC), BLAKE3 hashing, zstd, manifests, file kinds, ignore patterns, text merge |
| `crates/mp-server` | The server: HTTP API, accounts, organizations, projects, branches, claims, tags, drafts, admin, live events |
| `crates/mp-client` | Client library: accounts, connected folders, scan, publish, get latest, checkpoints, drafts, branches, selective sync |
| `crates/mp-daemon` | Background service on 127.0.0.1:18519: local API for the apps, auto-claim on first edit, automatic checkpoints |
| `crates/mp-cli` | `mp`, the command line (Perforce words work as aliases) |
| `web/` | The web UI (React), served by the server |
| `desktop/` | The desktop app for Windows, macOS and Linux (Tauri), with the daemon built in |
| `design/concepts/` | The UI concept rounds |
| `scripts/dev/` | Build-host loop and the end-to-end test |

## Run a server

```bash
cp .env.example .env
```

Edit `.env`: set `MP_ADMIN_EMAIL`, `MP_ADMIN_PASSWORD` (10+ characters), `POSTGRES_PASSWORD` and
`MP_PUBLIC_URL` (the address people will use). Then:

```bash
docker compose up -d
```

Open `MP_PUBLIC_URL` and sign in as the admin. The first organization is created for you; add
projects and invite people from Admin.

**Storage points.** `MP_STORAGE=local` keeps file data in a Docker volume. `MP_STORAGE=mount`
uses a host folder or network share (set `MP_BLOB_DIR`); the server refuses to start if the mount
is missing, rather than filling the container's disk. `MP_STORAGE=s3` uses any S3-compatible
service (AWS, MinIO, R2, Backblaze, Wasabi); `docker compose --profile s3 up -d` also starts a
bundled MinIO.

**More than one storage location.** `MP_STORAGE_LOCATIONS` adds named locations next to the main
one (see `.env.example`), for example a NAS for active projects and an S3 bucket for finished ones.
Each project keeps its data in one location, chosen when it is created (`MP_DEFAULT_STORAGE` sets
the default; `deploy/docker-compose.override.example.yml` shows an extra drive); a server admin
can move it later from Admin > Projects. The move copies in the background while people keep working, and
the old copy stays until Reclaim space removes it. On each computer, a synced folder can be moved
anywhere (the desktop app's Move folder…, or `mp move-folder <new place>`): on the same drive
it is renamed instantly, to another drive it is copied with its timestamps, and nothing is
downloaded again.

**Working remotely.** [docs/REMOTE.md](docs/REMOTE.md) covers reaching the server from outside the
office: Tailscale, a Cloudflare tunnel, or built-in HTTPS with Caddy
(`docker compose --profile https up -d`, with `MP_DOMAIN` and `MP_TLS` set). Sign-in protection
(waits after repeated wrong passwords) is always on.

## Use it

**Desktop app**: sign in under Accounts (as many servers and accounts as you like), then turn on
the projects you want on this computer and tick the folders to bring. A folder that already has
the project's files is matched, not re-downloaded, and nothing in it is deleted.

**Command line**:

```bash
mp login https://mp.studio.example --label work
```

```bash
mp connect Emberfall ~/Work/Emberfall --only Art --only Audio
```

```bash
mp status
```

```bash
mp publish -m "Hero retopo"
```

`mp --help` lists everything: `get-latest`, `claim`, `release`, `checkpoint`, `rewind`,
`discard`, `versions`, `draft save|list|restore`, `branch new|switch|list`, `update`, `merge`, `ignore`,
`folders`, `tag`, `admin`, `daemon`. Add `--json` to any command for scripts.

## Develop

The Rust code builds with stable Rust (1.80+). The web UI needs Node 22 and pnpm 9.

```bash
cargo build --workspace
```

```bash
cd web && pnpm install && pnpm dev
```

The web dev server proxies `/api` to `http://localhost:18500`; open it with `?mock=1` to use the
built-in mock data without a server.

**End-to-end test.** `scripts/dev/e2e.sh` starts a real server on Postgres and drives two people
through 20 scenarios with the `mp` CLI. It covers publishing with delta uploads, claims, stale
edits, checkpoints, drafts, branch auto-merge and binary clashes, adopting a populated folder,
selective sync, ignore rules, tags, suspension, the admin tools and the daemon. Set
`E2E_STORAGE=s3` or `mount` to run it on the other storage points.

`scripts/dev/baiji.sh` runs any of this on a Linux host with only Docker installed:

```bash
scripts/dev/baiji.sh cargo test --workspace
```

**Desktop app.** Needs the Tauri prerequisites for your platform
(https://tauri.app/start/prerequisites/). Then:

```bash
cd desktop && pnpm install && pnpm tauri build
```

## Licence

Server, apps and command line: AGPL-3.0, see [LICENSE](LICENSE). `mp-core`, `mp-client` and
`mp-daemon` (the pieces other tools embed): Apache-2.0, see the `LICENSE` file in each of those
crates, so tools that are not open source can build on them.
