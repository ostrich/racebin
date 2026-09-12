ALTER TABLE idempotency_records ADD COLUMN state TEXT NOT NULL DEFAULT 'completed'
  CHECK(state IN ('pending','completed'));
ALTER TABLE idempotency_records ADD COLUMN owner_token TEXT;
ALTER TABLE idempotency_records ADD COLUMN expected_attachment_count BIGINT NOT NULL DEFAULT 0;
