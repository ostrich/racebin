ALTER TABLE invitations ADD COLUMN comment TEXT;
ALTER TABLE invitations ADD COLUMN created_at BIGINT;
ALTER TABLE invitations ADD COLUMN redeemed_at BIGINT;

UPDATE invitations SET created_at=expires_at-86400 WHERE created_at IS NULL;

CREATE INDEX invitations_status_idx
ON invitations(redeemed,revoked,expires_at,id DESC);

