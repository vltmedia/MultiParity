-- Checkpoints kept on the server: private save points of one person's unpublished work, so big
-- projects need no checkpoint data on laptops and checkpoints survive a dead disk.

-- NULL: automatic (on for projects above MP_SERVER_CHECKPOINTS_FROM_GB); TRUE/FALSE: forced.
ALTER TABLE projects ADD COLUMN checkpoints_on_server BOOLEAN;

CREATE TABLE checkpoints (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    branch_id   UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The id the person's computer gave it; what they see and type.
    client_id   TEXT NOT NULL,
    device      TEXT,
    note        TEXT,
    auto        BOOLEAN NOT NULL DEFAULT FALSE,
    system      BOOLEAN NOT NULL DEFAULT FALSE,
    entries     JSONB NOT NULL,
    file_count  INTEGER NOT NULL,
    -- Size of the files as they were.
    size        BIGINT NOT NULL,
    -- Data uploaded for this checkpoint that the server did not already have.
    stored      BIGINT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, user_id, client_id)
);
CREATE INDEX checkpoints_owner ON checkpoints (project_id, user_id, created_at DESC);
CREATE INDEX checkpoints_user ON checkpoints (user_id);
