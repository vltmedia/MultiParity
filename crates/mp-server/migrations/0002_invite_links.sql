-- Keep the invite link so admins can copy it again until it is used, revoked or expires.
-- Cleared when the invite is accepted or revoked.
ALTER TABLE invites ADD COLUMN token TEXT;
