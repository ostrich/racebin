CREATE TABLE IF NOT EXISTS app_user (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('user','admin')),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)),
  force_password_change INTEGER NOT NULL DEFAULT 0 CHECK(force_password_change IN (0,1)),
  created BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_session (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id BIGINT NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  csrf_token TEXT NOT NULL,
  created BIGINT NOT NULL,
  expires BIGINT NOT NULL,
  last_used BIGINT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_invite (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  token_hash TEXT NOT NULL UNIQUE,
  created_by BIGINT NOT NULL REFERENCES app_user(id),
  expires BIGINT NOT NULL,
  used INTEGER NOT NULL DEFAULT 0 CHECK(used IN (0,1)),
  revoked INTEGER NOT NULL DEFAULT 0 CHECK(revoked IN (0,1))
);

CREATE TABLE IF NOT EXISTS api_key (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id BIGINT REFERENCES app_user(id),
  name TEXT NOT NULL,
  prefix TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL UNIQUE,
  scopes TEXT NOT NULL,
  created BIGINT NOT NULL,
  last_used BIGINT,
  enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1))
);

CREATE TABLE IF NOT EXISTS pasta (
  id INTEGER PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  owner_user_id BIGINT REFERENCES app_user(id) ON DELETE SET NULL,
  title TEXT NOT NULL DEFAULT '',
  content TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL CHECK(kind IN ('text','url')),
  syntax TEXT NOT NULL DEFAULT 'none',
  access TEXT NOT NULL CHECK(access IN ('public','unlisted','owner')),
  created BIGINT NOT NULL,
  expiration BIGINT,
  last_read BIGINT,
  read_count BIGINT NOT NULL DEFAULT 0,
  burn_after_reads BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS pasta_owner_idx ON pasta(owner_user_id, created DESC);
CREATE INDEX IF NOT EXISTS pasta_public_idx ON pasta(access, created DESC);

CREATE TABLE IF NOT EXISTS pasta_file (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pasta_id BIGINT NOT NULL REFERENCES pasta(id) ON DELETE CASCADE,
  position BIGINT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('primary','attachment')),
  name TEXT NOT NULL,
  storage_name TEXT NOT NULL,
  size BIGINT NOT NULL,
  UNIQUE(pasta_id, position),
  UNIQUE(pasta_id, name)
);
