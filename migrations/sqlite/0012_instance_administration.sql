ALTER TABLE users ADD COLUMN is_owner BIGINT NOT NULL DEFAULT 0 CHECK(is_owner IN (0,1));
ALTER TABLE sessions ADD COLUMN reauthenticated_at BIGINT;

UPDATE users
SET is_owner=1
WHERE id=(
  SELECT id FROM users
  WHERE role='admin' AND enabled=1
  ORDER BY id
  LIMIT 1
);

CREATE UNIQUE INDEX users_single_owner_idx ON users(is_owner) WHERE is_owner=1;

CREATE TABLE instance_settings (
  id BIGINT PRIMARY KEY CHECK(id=1),
  site_name TEXT NOT NULL,
  home_mode TEXT NOT NULL CHECK(home_mode IN ('standard','plain')),
  public_explore_enabled BIGINT NOT NULL CHECK(public_explore_enabled IN (0,1)),
  invitations_enabled BIGINT NOT NULL CHECK(invitations_enabled IN (0,1)),
  attachments_enabled BIGINT NOT NULL CHECK(attachments_enabled IN (0,1)),
  qr_codes_enabled BIGINT NOT NULL CHECK(qr_codes_enabled IN (0,1)),
  default_format TEXT NOT NULL CHECK(default_format IN ('text','markdown')),
  default_language TEXT NOT NULL,
  default_visibility TEXT NOT NULL CHECK(default_visibility IN ('public','unlisted','private')),
  default_expiration_seconds BIGINT CHECK(default_expiration_seconds IS NULL OR default_expiration_seconds > 0),
  updated_at BIGINT NOT NULL,
  updated_by_user_id BIGINT REFERENCES users(id) ON DELETE SET NULL
);

CREATE TABLE audit_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  actor_user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
  actor_username TEXT NOT NULL,
  actor_api_key_id BIGINT REFERENCES api_keys(id) ON DELETE SET NULL,
  action TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id TEXT,
  target_label TEXT,
  details TEXT NOT NULL DEFAULT '{}',
  created_at BIGINT NOT NULL
);

CREATE INDEX audit_events_created_idx ON audit_events(created_at DESC,id DESC);
CREATE INDEX audit_events_actor_idx ON audit_events(actor_user_id,created_at DESC);
CREATE INDEX audit_events_target_idx ON audit_events(target_type,target_id,created_at DESC);
