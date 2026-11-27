-- Exports: a branch's files at one change, written as plain files or one .zip to an export
-- target (this server's exports folder, or a folder, NAS or S3 bucket set in MP_EXPORT_TARGETS).
CREATE TABLE exports (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    branch_id     UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    change_number INT NOT NULL,
    target        TEXT NOT NULL,           -- "server" or a name from MP_EXPORT_TARGETS
    format        TEXT NOT NULL CHECK (format IN ('zip', 'files')),
    name          TEXT NOT NULL,           -- the folder or .zip written, relative to the target
    archive_after BOOLEAN NOT NULL DEFAULT FALSE,
    status        TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed')),
    files_total   INT NOT NULL DEFAULT 0,
    files_done    INT NOT NULL DEFAULT 0,
    bytes_total   BIGINT NOT NULL DEFAULT 0,
    bytes_done    BIGINT NOT NULL DEFAULT 0,
    error         TEXT,
    -- A one-time link for downloading a .zip on this server: the hash of its token, and expiry.
    link_hash     TEXT,
    link_expires  TIMESTAMPTZ,
    created_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at   TIMESTAMPTZ
);
CREATE INDEX exports_project ON exports (project_id, created_at DESC);
