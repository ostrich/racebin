ALTER TABLE pastes ADD COLUMN modified_at BIGINT;

-- Preserve an unambiguous existing modification when the most recent resource
-- update was not merely a recorded read. Older edits followed by a read cannot
-- be recovered reliably and remain unknown.
UPDATE pastes
SET modified_at = updated_at
WHERE updated_at > created_at
  AND (last_read_at IS NULL OR updated_at <> last_read_at);
