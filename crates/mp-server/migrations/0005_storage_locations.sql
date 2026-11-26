-- Per-project storage locations. "main" is the location MP_STORAGE describes; others come from
-- MP_STORAGE_LOCATIONS. A project's data lives in one location; while it is being moved it also
-- has a previous one, which reads fall back to until the move finishes.

ALTER TABLE projects ADD COLUMN storage TEXT NOT NULL DEFAULT 'main';
ALTER TABLE projects ADD COLUMN storage_previous TEXT;

-- The same chunk can now be stored in more than one location.
ALTER TABLE chunks ADD COLUMN location TEXT NOT NULL DEFAULT 'main';
ALTER TABLE chunks DROP CONSTRAINT chunks_pkey;
ALTER TABLE chunks ADD PRIMARY KEY (location, hash);
CREATE INDEX chunks_hash ON chunks (hash);

CREATE TABLE storage_moves (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    from_location TEXT NOT NULL,
    to_location   TEXT NOT NULL,
    -- running, done, failed, interrupted (the server stopped during it)
    status        TEXT NOT NULL DEFAULT 'running',
    total_chunks  BIGINT NOT NULL DEFAULT 0,
    copied_chunks BIGINT NOT NULL DEFAULT 0,
    copied_bytes  BIGINT NOT NULL DEFAULT 0,
    error         TEXT,
    started_by    UUID REFERENCES users(id) ON DELETE SET NULL,
    started_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at   TIMESTAMPTZ
);
CREATE INDEX storage_moves_project ON storage_moves (project_id, started_at DESC);
