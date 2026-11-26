# Backups

The server backs itself up every night: the whole database and every storage location's file
data, copied to a backup place you choose. Admin > Fixing shows the last backups, and has a
**Back up now** button.

## What is copied

- **The database**, table by table from one consistent snapshot, compressed. A few MB for most
  studios.
- **File data**, part by part. A part is copied once and shared by every backup that needs it,
  so each night only copies what was published, saved as a draft or checkpointed that day.

Backups are kept for 7 days (one per day) and 4 weeks (one per week). File data no kept backup
needs is removed from the backup place automatically.

## Setting it up

In `.env`:

    MP_BACKUP_TARGET=local:/data/backups

That writes to the `backups` Docker volume, which is on the same disk as everything else: it
protects against mistakes, not against the disk failing. For real protection, back up to a
different disk or off site:

| Where | `MP_BACKUP_TARGET` | Also needed |
|---|---|---|
| A second drive on the server | `local:/data/backups` | `MP_BACKUP_DIR=/mnt/backup-drive/multiparity` (the host folder for the volume) |
| A NAS share mounted on the server | `mount:/data/nas-backups` | a volume line in `docker-compose.override.yml` |
| Backblaze B2, Cloudflare R2, AWS S3, MinIO | `s3:bucket=studio-backups;endpoint=https://s3.us-west-004.backblazeb2.com;region=us-west-004;prefix=multiparity` | `MP_STORAGE_BACKUP_ACCESS_KEY_ID`, `MP_STORAGE_BACKUP_SECRET_ACCESS_KEY` |

`mount:` refuses to start if the path is missing, so an unplugged drive is reported instead of
filling the internal disk. Other settings: `MP_BACKUP_AT` (UTC, default `03:00`),
`MP_BACKUP_KEEP_DAILY` (7), `MP_BACKUP_KEEP_WEEKLY` (4). A backup missed while the server was
off runs within a minute of it starting again.

From a shell on the server:

    docker compose exec server mp-server backup-now     # one backup now
    docker compose exec server mp-server backups        # list them

## Restoring

Restoring fills an **empty** database and the storage locations from a backup. It refuses a
database that already has people in it, unless you pass `--force` (which replaces everything).

1. Stop the server, and keep the old database aside rather than deleting it:

       docker compose stop server
       docker compose exec db psql -U multiparity -d postgres -c "ALTER DATABASE multiparity RENAME TO multiparity_old"
       docker compose exec db psql -U multiparity -d postgres -c "CREATE DATABASE multiparity"

2. Restore the newest backup (or `--snapshot <id>` from `mp-server backups`):

       docker compose run --rm server mp-server restore

   It prints what it restored. File data is copied back only where a part is missing, so
   restoring onto storage that survived is quick. Parts of a storage location this server is no
   longer configured with are listed; add the location to `MP_STORAGE_LOCATIONS` and run the
   restore again with `--force`.

3. Start the server and check:

       docker compose up -d
       # Admin > Fixing > Storage check: everything should be found.

When everything is right, drop the old database:
`docker compose exec db psql -U multiparity -d postgres -c "DROP DATABASE multiparity_old"`.

`--db-only` restores just the database (for example to look at an old state in a scratch
database). A backup restores into the same or a newer server version.

## How it was tested

The end-to-end test (`scripts/dev/e2e.sh`, step "Backups") backs up a server, checks that a
second backup copies no file data again, restores into a brand-new database and brand-new
storage folders, starts a second server on the restored copy, signs in, compares the history,
runs the storage check and downloads a file to compare byte for byte.

It was also tried on a real studio server: a backup of a live database restored into a scratch
database and scratch storage folders, with every table's row count and every file part
matching, and the scratch copy dropped afterwards.
