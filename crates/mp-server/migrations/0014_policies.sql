-- Policies: named sets of access rules an organization makes and gives to people, e.g. "Audio
-- team" or "Northwind reviewers". A rule gives a level on the whole organization, a client, a
-- production or one project: 0 no access, 1 read, 2 write.
--
-- How someone's access to a project is worked out (mp_access below):
--   * owners and admins of the project's organization: everything (3);
--   * otherwise the rules that cover the project, from the organization's default policy (for
--     members, not guests) and the policies given to them, plus any access given on the project
--     itself (project_access): the most specific rules win (project, then production, then
--     client, then organization), and among those the highest level.
CREATE TABLE policies (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    -- The policy every member (not guest) has without being given it. One per organization.
    is_default  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX policies_name ON policies (org_id, lower(name));
CREATE UNIQUE INDEX policies_one_default ON policies (org_id) WHERE is_default;

CREATE TABLE policy_rules (
    policy_id  UUID NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('org', 'client', 'production', 'project')),
    scope_id   UUID NOT NULL,
    level      SMALLINT NOT NULL CHECK (level BETWEEN 0 AND 2),
    PRIMARY KEY (policy_id, scope_kind, scope_id)
);
CREATE INDEX policy_rules_scope ON policy_rules (scope_kind, scope_id);

CREATE TABLE user_policies (
    user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    policy_id UUID NOT NULL REFERENCES policies(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, policy_id)
);
CREATE INDEX user_policies_policy ON user_policies (policy_id);

-- Invites can give policies on joining.
ALTER TABLE invites ADD COLUMN policies UUID[] NOT NULL DEFAULT '{}';

-- Every organization keeps today's access: members can change every project.
INSERT INTO policies (org_id, name, description, is_default)
SELECT id, 'All members', 'Every member of the organization has this without being given it. Guests do not.', TRUE FROM orgs;
INSERT INTO policy_rules (policy_id, scope_kind, scope_id, level)
SELECT id, 'org', org_id, 2 FROM policies WHERE is_default;

-- A user's level on a project: 0 none, 1 read, 2 write, 3 admin. Server admins are handled by
-- the server before asking.
CREATE FUNCTION mp_access(u UUID, pid UUID) RETURNS SMALLINT LANGUAGE sql STABLE AS $$
  WITH p AS (SELECT id, org_id, client_id, production_id FROM projects WHERE id = pid),
  m AS (SELECT mm.role FROM memberships mm JOIN p ON mm.org_id = p.org_id
        WHERE mm.user_id = u AND (mm.expires_at IS NULL OR mm.expires_at > now())),
  rules AS (
    SELECT CASE r.scope_kind WHEN 'org' THEN 0 WHEN 'client' THEN 1 WHEN 'production' THEN 2 ELSE 3 END AS spec, r.level
    FROM policy_rules r JOIN policies pol ON pol.id = r.policy_id JOIN p ON pol.org_id = p.org_id
    WHERE ((pol.is_default AND EXISTS (SELECT 1 FROM m WHERE m.role = 'member'))
           OR EXISTS (SELECT 1 FROM user_policies up WHERE up.policy_id = pol.id AND up.user_id = u))
      AND ((r.scope_kind = 'org' AND r.scope_id = p.org_id)
        OR (r.scope_kind = 'client' AND r.scope_id = p.client_id)
        OR (r.scope_kind = 'production' AND r.scope_id = p.production_id)
        OR (r.scope_kind = 'project' AND r.scope_id = p.id))
    UNION ALL
    SELECT 3, (CASE WHEN pa.can_write THEN 2 ELSE 1 END)::SMALLINT FROM project_access pa WHERE pa.project_id = pid AND pa.user_id = u
  )
  SELECT (CASE
    WHEN NOT EXISTS (SELECT 1 FROM m) THEN 0
    WHEN EXISTS (SELECT 1 FROM m WHERE m.role IN ('owner', 'admin')) THEN 3
    ELSE COALESCE((SELECT max(level) FROM rules WHERE spec = (SELECT max(spec) FROM rules)), 0)
  END)::SMALLINT
$$;
