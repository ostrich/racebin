CREATE TABLE folders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  owner_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  name_key TEXT NOT NULL,
  created_at BIGINT NOT NULL
);

CREATE UNIQUE INDEX folders_owner_name_idx ON folders(owner_id, name_key);
CREATE INDEX folders_owner_idx ON folders(owner_id, name_key);

ALTER TABLE pastes
ADD COLUMN folder_id BIGINT REFERENCES folders(id) ON DELETE SET NULL;

CREATE INDEX pastes_folder_idx ON pastes(owner_id, folder_id, created_at DESC);
