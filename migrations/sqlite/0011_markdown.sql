CREATE TABLE pastes_markdown (
  id TEXT PRIMARY KEY,
  owner_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  content_kind TEXT NOT NULL CHECK(content_kind IN ('text','markdown')),
  language TEXT NOT NULL DEFAULT 'plaintext',
  visibility TEXT NOT NULL CHECK(visibility IN ('public','unlisted','private')),
  created_at BIGINT NOT NULL,
  expires_at BIGINT,
  last_read_at BIGINT,
  read_count BIGINT NOT NULL DEFAULT 0,
  read_limit BIGINT CHECK(read_limit IS NULL OR read_limit > 0),
  folder_id BIGINT REFERENCES folders(id) ON DELETE SET NULL,
  updated_at BIGINT NOT NULL DEFAULT 0,
  revision BIGINT NOT NULL DEFAULT 1,
  consumed_at BIGINT
);
INSERT INTO pastes_markdown(id,owner_id,title,content,content_kind,language,visibility,created_at,expires_at,last_read_at,read_count,read_limit,folder_id,updated_at,revision,consumed_at)
SELECT id,owner_id,title,content,CASE WHEN content_kind='rich_text' THEN 'markdown' ELSE content_kind END,language,visibility,created_at,expires_at,last_read_at,read_count,read_limit,folder_id,updated_at,revision,consumed_at FROM pastes;
CREATE TABLE attachments_markdown (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  paste_id TEXT NOT NULL REFERENCES pastes_markdown(id) ON DELETE CASCADE,
  sort_order BIGINT NOT NULL, filename TEXT NOT NULL, storage_key TEXT NOT NULL, size_bytes BIGINT NOT NULL,
  UNIQUE(paste_id,sort_order), UNIQUE(paste_id,filename)
);
INSERT INTO attachments_markdown SELECT id,paste_id,sort_order,filename,storage_key,size_bytes FROM attachments;
CREATE TABLE idempotency_records_markdown (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE, operation TEXT NOT NULL,
  key_hash TEXT NOT NULL, request_hash TEXT NOT NULL,
  paste_id TEXT REFERENCES pastes_markdown(id) ON DELETE SET NULL,
  created_at BIGINT NOT NULL, expires_at BIGINT NOT NULL,
  PRIMARY KEY(user_id,operation,key_hash)
);
INSERT INTO idempotency_records_markdown SELECT user_id,operation,key_hash,request_hash,paste_id,created_at,expires_at FROM idempotency_records;
CREATE TABLE paste_read_receipts_markdown (
  paste_id TEXT NOT NULL REFERENCES pastes_markdown(id) ON DELETE CASCADE,
  key_hash TEXT NOT NULL, expires_at BIGINT NOT NULL, PRIMARY KEY(paste_id,key_hash)
);
INSERT INTO paste_read_receipts_markdown SELECT paste_id,key_hash,expires_at FROM paste_read_receipts;
CREATE TABLE paste_read_grants_markdown (
  token_hash TEXT PRIMARY KEY, paste_id TEXT NOT NULL REFERENCES pastes_markdown(id) ON DELETE CASCADE,
  expires_at BIGINT NOT NULL
);
INSERT INTO paste_read_grants_markdown SELECT token_hash,paste_id,expires_at FROM paste_read_grants;
DROP TABLE attachments;
DROP TABLE idempotency_records;
DROP TABLE paste_read_receipts;
DROP TABLE paste_read_grants;
DROP TABLE pastes;
ALTER TABLE pastes_markdown RENAME TO pastes;
ALTER TABLE attachments_markdown RENAME TO attachments;
ALTER TABLE idempotency_records_markdown RENAME TO idempotency_records;
ALTER TABLE paste_read_receipts_markdown RENAME TO paste_read_receipts;
ALTER TABLE paste_read_grants_markdown RENAME TO paste_read_grants;
CREATE INDEX pastes_owner_idx ON pastes(owner_id,created_at DESC);
CREATE INDEX pastes_public_idx ON pastes(visibility,created_at DESC);
CREATE INDEX pastes_folder_idx ON pastes(owner_id,folder_id,created_at DESC);
CREATE INDEX pastes_folder_only_idx ON pastes(folder_id);
CREATE INDEX idempotency_records_expiry_idx ON idempotency_records(expires_at);
CREATE INDEX paste_read_receipts_expiry_idx ON paste_read_receipts(expires_at);
CREATE INDEX paste_read_grants_expiry_idx ON paste_read_grants(expires_at);
