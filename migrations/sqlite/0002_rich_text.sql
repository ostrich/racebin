CREATE TABLE pastes_rich_text (
  id TEXT PRIMARY KEY,
  owner_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  document_json TEXT,
  content_kind TEXT NOT NULL CHECK(content_kind IN ('text','rich_text','redirect')),
  language TEXT NOT NULL DEFAULT 'plaintext',
  visibility TEXT NOT NULL CHECK(visibility IN ('public','unlisted','private')),
  created_at BIGINT NOT NULL,
  expires_at BIGINT,
  last_read_at BIGINT,
  read_count BIGINT NOT NULL DEFAULT 0,
  read_limit BIGINT CHECK(read_limit IS NULL OR read_limit > 0),
  CHECK((content_kind='rich_text') = (document_json IS NOT NULL))
);

INSERT INTO pastes_rich_text(
  id,owner_id,title,content,content_kind,language,visibility,created_at,
  expires_at,last_read_at,read_count,read_limit
)
SELECT id,owner_id,title,content,content_kind,language,visibility,created_at,
       expires_at,last_read_at,read_count,read_limit
FROM pastes;

CREATE TABLE attachments_rich_text (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  paste_id TEXT NOT NULL REFERENCES pastes_rich_text(id) ON DELETE CASCADE,
  sort_order BIGINT NOT NULL,
  filename TEXT NOT NULL,
  storage_key TEXT NOT NULL,
  size_bytes BIGINT NOT NULL,
  UNIQUE(paste_id, sort_order),
  UNIQUE(paste_id, filename)
);
INSERT INTO attachments_rich_text
SELECT id,paste_id,sort_order,filename,storage_key,size_bytes FROM attachments;

DROP TABLE attachments;
DROP TABLE pastes;
ALTER TABLE pastes_rich_text RENAME TO pastes;
ALTER TABLE attachments_rich_text RENAME TO attachments;
CREATE INDEX pastes_owner_idx ON pastes(owner_id, created_at DESC);
CREATE INDEX pastes_public_idx ON pastes(visibility, created_at DESC);
