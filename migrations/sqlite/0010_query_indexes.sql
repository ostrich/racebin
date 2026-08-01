DROP INDEX folders_owner_idx;

CREATE INDEX sessions_expiry_idx ON sessions(expires_at);
CREATE INDEX sessions_user_expiry_idx ON sessions(user_id, expires_at);
CREATE INDEX api_keys_user_idx ON api_keys(user_id);
CREATE INDEX invitations_expiry_idx ON invitations(expires_at);
CREATE INDEX password_reset_tokens_expiry_idx ON password_reset_tokens(expires_at);
CREATE INDEX auth_attempts_expiry_idx ON auth_attempts(occurred_at);
CREATE INDEX pastes_folder_only_idx ON pastes(folder_id);
