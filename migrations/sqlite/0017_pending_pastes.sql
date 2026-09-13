ALTER TABLE pastes ADD COLUMN creation_state TEXT NOT NULL DEFAULT 'complete'
  CHECK(creation_state IN ('pending','complete'));

UPDATE idempotency_records
SET state='completed', owner_token=NULL
WHERE operation='create_paste' AND state='pending'
  AND expected_attachment_count=(
    SELECT count(*) FROM attachments WHERE paste_id=idempotency_records.paste_id
  );

UPDATE pastes SET creation_state='pending'
WHERE id IN (
  SELECT paste_id FROM idempotency_records
  WHERE operation='create_paste' AND state='pending' AND paste_id IS NOT NULL
);

CREATE INDEX pastes_creation_state_idx ON pastes(creation_state, created_at);
