UPDATE pastes SET content_kind='markdown', language='plaintext' WHERE content_kind='rich_text';
ALTER TABLE pastes DROP CONSTRAINT pastes_rich_text_document_check;
ALTER TABLE pastes DROP CONSTRAINT pastes_content_kind_check;
ALTER TABLE pastes ADD CONSTRAINT pastes_content_kind_check CHECK(content_kind IN ('text','markdown'));
ALTER TABLE pastes DROP COLUMN document_json;
