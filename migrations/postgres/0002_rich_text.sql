ALTER TABLE pastes ADD COLUMN document_json TEXT;
ALTER TABLE pastes DROP CONSTRAINT pastes_content_kind_check;
ALTER TABLE pastes ADD CONSTRAINT pastes_content_kind_check
  CHECK(content_kind IN ('text','rich_text','redirect'));
ALTER TABLE pastes ADD CONSTRAINT pastes_rich_text_document_check
  CHECK((content_kind='rich_text') = (document_json IS NOT NULL));
