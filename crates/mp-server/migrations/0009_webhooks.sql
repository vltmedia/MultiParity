-- Webhooks: the server posts matching events to a URL, signed with the hook's secret.
CREATE TABLE webhooks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    -- Event kinds, exact ("change.published") or by prefix ("change.*"); empty means all.
    events      TEXT[] NOT NULL DEFAULT '{}',
    -- Only events of this project (null: every project of the organization).
    project_id  UUID REFERENCES projects(id) ON DELETE CASCADE,
    secret      TEXT NOT NULL,
    active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_at     TIMESTAMPTZ,
    last_status INTEGER,
    last_error  TEXT,
    failures    INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX webhooks_org ON webhooks(org_id) WHERE active;
