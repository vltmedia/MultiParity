-- Backup runs (scheduled and "Back up now"). The backups themselves live on the backup target.
CREATE TABLE backup_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    started_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at     TIMESTAMPTZ,
    -- running, done, failed
    status          TEXT NOT NULL DEFAULT 'running',
    error           TEXT,
    snapshot        TEXT,
    db_bytes        BIGINT,
    new_chunks      BIGINT,
    new_chunk_bytes BIGINT,
    total_chunks    BIGINT,
    started_by      UUID REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX backup_runs_started ON backup_runs (started_at DESC);
