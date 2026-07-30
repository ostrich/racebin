CREATE TABLE IF NOT EXISTS users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('user','admin')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  password_change_required INTEGER NOT NULL DEFAULT 0 CHECK(password_change_required IN (0,1)),
  created_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  csrf_token TEXT NOT NULL,
  created_at BIGINT NOT NULL,
  expires_at BIGINT NOT NULL,
  last_used_at BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS invitations (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  token_hash TEXT NOT NULL UNIQUE,
  created_by_user_id BIGINT NOT NULL REFERENCES users(id),
  expires_at BIGINT NOT NULL,
  redeemed INTEGER NOT NULL DEFAULT 0 CHECK(redeemed IN (0,1)),
  revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
);

CREATE TABLE IF NOT EXISTS api_keys (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_prefix TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL UNIQUE,
  created_at BIGINT NOT NULL,
  last_used_at BIGINT,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1))
);

CREATE TABLE IF NOT EXISTS api_key_scopes (
  api_key_id BIGINT NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
  scope TEXT NOT NULL CHECK(scope IN (
    'paste:read','paste:write','paste:delete','paste:list','paste:manage',
    'user:manage','invitation:manage','api_key:manage'
  )),
  PRIMARY KEY(api_key_id, scope)
);

CREATE TABLE IF NOT EXISTS pastes (
  id TEXT PRIMARY KEY,
  owner_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  content_kind TEXT NOT NULL CHECK(content_kind IN ('text','redirect')),
  language TEXT NOT NULL DEFAULT 'plaintext',
  visibility TEXT NOT NULL CHECK(visibility IN ('public','unlisted','private')),
  created_at BIGINT NOT NULL,
  expires_at BIGINT,
  last_read_at BIGINT,
  read_count BIGINT NOT NULL DEFAULT 0,
  read_limit BIGINT CHECK(read_limit IS NULL OR read_limit > 0)
);

CREATE INDEX IF NOT EXISTS pastes_owner_idx ON pastes(owner_id, created_at DESC);
CREATE INDEX IF NOT EXISTS pastes_public_idx ON pastes(visibility, created_at DESC);

CREATE TABLE IF NOT EXISTS attachments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  paste_id TEXT NOT NULL REFERENCES pastes(id) ON DELETE CASCADE,
  sort_order BIGINT NOT NULL,
  filename TEXT NOT NULL,
  storage_key TEXT NOT NULL,
  size_bytes BIGINT NOT NULL,
  UNIQUE(paste_id, sort_order),
  UNIQUE(paste_id, filename)
);
