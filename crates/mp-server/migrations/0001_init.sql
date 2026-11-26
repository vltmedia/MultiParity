-- MultiParity schema v1.

CREATE TABLE users (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email            TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    password_hash    TEXT,
    is_server_admin  BOOLEAN NOT NULL DEFAULT FALSE,
    status           TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended')),
    suspended_reason TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at     TIMESTAMPTZ
);

CREATE TABLE sessions (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash   TEXT NOT NULL UNIQUE,
    kind         TEXT NOT NULL DEFAULT 'session' CHECK (kind IN ('session', 'token')),
    label        TEXT,
    device       TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX sessions_user ON sessions(user_id);

CREATE TABLE orgs (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE memberships (
    org_id     UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'guest')),
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (org_id, user_id)
);
CREATE INDEX memberships_user ON memberships(user_id);

CREATE TABLE invites (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    email       TEXT,
    role        TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'guest')),
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at TIMESTAMPTZ,
    accepted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    revoked_at  TIMESTAMPTZ
);

CREATE TABLE projects (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL REFERENCES orgs(id),
    slug        TEXT NOT NULL,
    name        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ,
    next_change INTEGER NOT NULL DEFAULT 1,
    UNIQUE (org_id, slug)
);

CREATE TABLE project_access (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    can_write  BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (project_id, user_id)
);

CREATE TABLE branches (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id          UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    parent_id           UUID REFERENCES branches(id),
    shares_claims       BOOLEAN NOT NULL DEFAULT TRUE,
    protected           BOOLEAN NOT NULL DEFAULT FALSE,
    archived            BOOLEAN NOT NULL DEFAULT FALSE,
    head_change         INTEGER NOT NULL DEFAULT 0,
    last_merged_change  INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by          UUID REFERENCES users(id) ON DELETE SET NULL,
    UNIQUE (project_id, name)
);

CREATE TABLE changes (
    id          BIGSERIAL PRIMARY KEY,
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    number      INTEGER NOT NULL,
    branch_id   UUID NOT NULL REFERENCES branches(id),
    author_id   UUID REFERENCES users(id) ON DELETE SET NULL,
    message     TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('publish', 'merge', 'update')),
    merged_from INTEGER[],
    file_count  INTEGER NOT NULL DEFAULT 0,
    size        BIGINT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, number)
);
CREATE INDEX changes_branch ON changes(branch_id, number DESC);

CREATE TABLE change_files (
    project_id    UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    change_number INTEGER NOT NULL,
    path          TEXT NOT NULL,
    op            TEXT NOT NULL CHECK (op IN ('add', 'edit', 'delete')),
    size          BIGINT NOT NULL DEFAULT 0,
    file_hash     TEXT,
    manifest_hash TEXT,
    PRIMARY KEY (project_id, change_number, path)
);
CREATE INDEX change_files_path ON change_files(project_id, path);

-- The current state of each branch. A NULL file_hash is a deletion (tombstone). The base_*
-- columns hold the parent branch's version as of the last update from the parent; on the root
-- branch they are unused.
CREATE TABLE tree_entries (
    branch_id          UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    path               TEXT NOT NULL,
    file_hash          TEXT,
    manifest_hash      TEXT,
    size               BIGINT NOT NULL DEFAULT 0,
    change_number      INTEGER NOT NULL,
    base_file_hash     TEXT,
    base_manifest_hash TEXT,
    base_size          BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (branch_id, path)
);
CREATE INDEX tree_entries_change ON tree_entries(branch_id, change_number);

CREATE TABLE manifests (
    hash       TEXT PRIMARY KEY,
    size       BIGINT NOT NULL,
    file_hash  TEXT NOT NULL,
    chunks     JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE chunks (
    hash        TEXT PRIMARY KEY,
    size        BIGINT NOT NULL,
    stored_size BIGINT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE claims (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scope_id   UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    branch_id  UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, scope_id, path)
);
CREATE INDEX claims_user ON claims(user_id);

CREATE TABLE tags (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id     UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    color      TEXT NOT NULL DEFAULT '#7a3cff',
    group_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX tags_unique ON tags(org_id, COALESCE(project_id, '00000000-0000-0000-0000-000000000000'::uuid), lower(name));

CREATE TABLE path_tags (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    tag_id     UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, path, tag_id)
);

CREATE TABLE drafts (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    branch_id  UUID NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    entries    JSONB NOT NULL,
    file_count INTEGER NOT NULL,
    size       BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE events (
    id         BIGSERIAL PRIMARY KEY,
    org_id     UUID,
    project_id UUID,
    actor_id   UUID,
    kind       TEXT NOT NULL,
    data       JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX events_project ON events(project_id, id DESC);
CREATE INDEX events_org ON events(org_id, id DESC);
