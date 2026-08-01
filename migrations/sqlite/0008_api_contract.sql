ALTER TABLE pastes ADD COLUMN updated_at BIGINT NOT NULL DEFAULT 0;
ALTER TABLE pastes ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
ALTER TABLE pastes ADD COLUMN consumed_at BIGINT;

UPDATE pastes SET updated_at = created_at WHERE updated_at = 0;

CREATE TABLE idempotency_records (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  operation TEXT NOT NULL,
  key_hash TEXT NOT NULL,
  request_hash TEXT NOT NULL,
  paste_id TEXT REFERENCES pastes(id) ON DELETE SET NULL,
  created_at BIGINT NOT NULL,
  expires_at BIGINT NOT NULL,
  PRIMARY KEY(user_id, operation, key_hash)
);

CREATE INDEX idempotency_records_expiry_idx ON idempotency_records(expires_at);

CREATE TABLE paste_read_receipts (
  paste_id TEXT NOT NULL REFERENCES pastes(id) ON DELETE CASCADE,
  key_hash TEXT NOT NULL,
  expires_at BIGINT NOT NULL,
  PRIMARY KEY(paste_id, key_hash)
);

CREATE INDEX paste_read_receipts_expiry_idx ON paste_read_receipts(expires_at);

CREATE TABLE paste_read_grants (
  token_hash TEXT PRIMARY KEY,
  paste_id TEXT NOT NULL REFERENCES pastes(id) ON DELETE CASCADE,
  expires_at BIGINT NOT NULL
);

CREATE INDEX paste_read_grants_expiry_idx ON paste_read_grants(expires_at);
