-- Grouping projects: Organization > Client > Production > Project. Both levels are optional:
-- a project can sit straight in its organization, under a client, or in a production (which
-- may itself have a client or not). Each project keeps its own history and access.
CREATE TABLE clients (
    id         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id     UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX clients_name ON clients (org_id, lower(name));

CREATE TABLE productions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    client_id       UUID REFERENCES clients(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    -- Listed first in the production, and where opening the production lands.
    main_project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX productions_name ON productions (org_id, COALESCE(client_id, '00000000-0000-0000-0000-000000000000'::uuid), lower(name));

-- client_id is always the project's client, also when it comes through its production (kept in
-- step by the server), so listings need no extra join to know it.
ALTER TABLE projects ADD COLUMN client_id UUID REFERENCES clients(id) ON DELETE SET NULL;
ALTER TABLE projects ADD COLUMN production_id UUID REFERENCES productions(id) ON DELETE SET NULL;
CREATE INDEX projects_client ON projects (client_id);
CREATE INDEX projects_production ON projects (production_id);
