-- Thumbnails and previews of published files, by content: a file published twice, or in two
-- projects, is drawn once. `status`: pending (waiting for the worker), ready, none (nothing to
-- show, e.g. a .blend saved without a preview), failed (could not be read; `error` says why).
CREATE TABLE thumbnails (
    file_hash     TEXT PRIMARY KEY,
    project_id    UUID REFERENCES projects(id) ON DELETE CASCADE,
    manifest_hash TEXT NOT NULL,
    path          TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'ready', 'none', 'failed')),
    width         INTEGER,
    height        INTEGER,
    small         BYTEA,
    large         BYTEA,
    error         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    done_at       TIMESTAMPTZ
);
CREATE INDEX thumbnails_pending ON thumbnails(created_at) WHERE status = 'pending';
