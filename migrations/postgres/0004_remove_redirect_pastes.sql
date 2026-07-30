UPDATE pastes
SET content_kind = 'text',
    language = 'plaintext'
WHERE content_kind = 'redirect';

ALTER TABLE pastes DROP CONSTRAINT pastes_content_kind_check;
ALTER TABLE pastes
  ADD CONSTRAINT pastes_content_kind_check
  CHECK(content_kind IN ('text','rich_text'));
