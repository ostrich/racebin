ALTER TABLE invitations
ADD COLUMN redeemed_by_user_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
