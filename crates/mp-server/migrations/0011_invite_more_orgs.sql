-- An invite can add the person to more organizations than the one it was made in, each with its
-- own role: [{"org_id": "...", "role": "member"}, ...]. Accepting joins all of them.
ALTER TABLE invites ADD COLUMN also JSONB NOT NULL DEFAULT '[]';
