use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, Transaction};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 2;

#[derive(Clone)]
pub struct Repository {
    pool: Pool<SqliteConnectionManager>,
    pub data_dir: PathBuf,
}

impl Repository {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self, String> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let manager =
            SqliteConnectionManager::file(data_dir.join("database.sqlite")).with_init(|conn| {
                conn.execute_batch(
                    "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;",
                )
            });
        let pool = Pool::builder()
            .max_size(16)
            .build(manager)
            .map_err(|e| e.to_string())?;
        Ok(Self { pool, data_dir })
    }

    pub fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, String> {
        self.pool.get().map_err(|e| e.to_string())
    }

    pub fn migrate(&self) -> Result<(), String> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let version: i64 = tx
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version >= SCHEMA_VERSION {
            return Ok(());
        }
        ensure_identity_schema(&tx)?;
        if table_exists(&tx, "pasta")? && column_exists(&tx, "pasta", "private")? {
            preflight_legacy(&tx)?;
            migrate_legacy_pastes(&tx)?;
        } else {
            create_paste_schema(&tx)?;
        }
        normalize_api_key_scopes(&tx)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        params![name],
        |row| row.get(0),
    )
    .map_err(|e| e.to_string())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(names.iter().any(|name| name == column))
}

fn ensure_identity_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS app_user (
          id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT NOT NULL UNIQUE,
          password_hash TEXT NOT NULL, role TEXT NOT NULL CHECK(role IN ('user','admin')),
          enabled INTEGER NOT NULL DEFAULT 1, force_password_change INTEGER NOT NULL DEFAULT 0,
          created INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_session (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          user_id INTEGER NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
          token_hash TEXT NOT NULL UNIQUE, csrf_token TEXT NOT NULL,
          created INTEGER NOT NULL, expires INTEGER NOT NULL, last_used INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_invite (
          id INTEGER PRIMARY KEY AUTOINCREMENT, token_hash TEXT NOT NULL UNIQUE,
          created_by INTEGER NOT NULL REFERENCES app_user(id), expires INTEGER NOT NULL,
          used INTEGER NOT NULL DEFAULT 0, revoked INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS api_key (
          id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER REFERENCES app_user(id),
          name TEXT NOT NULL, prefix TEXT NOT NULL UNIQUE, token_hash TEXT NOT NULL UNIQUE,
          scopes TEXT NOT NULL, created INTEGER NOT NULL, last_used INTEGER,
          enabled INTEGER NOT NULL DEFAULT 1
        );",
    )
    .map_err(|e| e.to_string())
}

fn create_paste_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS pasta (
          id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE,
          owner_user_id INTEGER REFERENCES app_user(id) ON DELETE SET NULL,
          title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '',
          kind TEXT NOT NULL CHECK(kind IN ('text','url')),
          syntax TEXT NOT NULL DEFAULT 'none',
          access TEXT NOT NULL CHECK(access IN ('public','unlisted','owner')),
          created INTEGER NOT NULL, expiration INTEGER, last_read INTEGER,
          read_count INTEGER NOT NULL DEFAULT 0, burn_after_reads INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS pasta_owner_idx ON pasta(owner_user_id, created DESC);
        CREATE INDEX IF NOT EXISTS pasta_public_idx ON pasta(access, created DESC);
        CREATE TABLE IF NOT EXISTS pasta_file (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          pasta_id INTEGER NOT NULL REFERENCES pasta(id) ON DELETE CASCADE,
          position INTEGER NOT NULL, role TEXT NOT NULL CHECK(role IN ('primary','attachment')),
          name TEXT NOT NULL, size INTEGER NOT NULL,
          UNIQUE(pasta_id, position)
        );",
    )
    .map_err(|e| e.to_string())
}

fn preflight_legacy(conn: &Connection) -> Result<(), String> {
    let unsupported: i64 = conn
        .query_row(
            "SELECT count(*) FROM pasta
             WHERE read_only != 0 OR encrypt_server != 0 OR encrypt_client != 0",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if unsupported != 0 {
        return Err(format!(
            "migration refused: {unsupported} encrypted or read-only pastes require manual cleanup"
        ));
    }
    Ok(())
}

fn migrate_legacy_pastes(tx: &Transaction<'_>) -> Result<(), String> {
    tx.execute_batch("ALTER TABLE pasta RENAME TO pasta_legacy;")
        .map_err(|e| e.to_string())?;
    create_paste_schema(tx)?;
    tx.execute(
        "INSERT INTO pasta
         (id,slug,owner_user_id,title,content,kind,syntax,access,created,expiration,last_read,read_count,burn_after_reads)
         SELECT id, CAST(id AS TEXT), owner_user_id, title, content,
                CASE WHEN pasta_type='url' THEN 'url' ELSE 'text' END,
                extension,
                CASE WHEN private=0 THEN 'public'
                     WHEN owner_user_id IS NULL THEN 'unlisted' ELSE 'owner' END,
                created, NULLIF(expiration,0), last_read, read_count, burn_after_reads
         FROM pasta_legacy",
        [],
    )
    .map_err(|e| e.to_string())?;
    {
        let mut statement = tx
            .prepare("SELECT id FROM pasta")
            .map_err(|e| e.to_string())?;
        let ids = statement
            .query_map([], |row| row.get::<_, u64>(0))
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        drop(statement);
        for id in ids {
            tx.execute(
                "UPDATE pasta SET slug=?2 WHERE id=?1",
                params![id, crate::util::animalnumbers::to_animal_names(id)],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let mut stmt = tx
        .prepare(
            "SELECT id,file_name,file_size,attachments FROM pasta_legacy
             WHERE (file_name IS NOT NULL AND file_name != '') OR attachments IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for (pasta_id, primary, size, attachments) in rows {
        let mut position = 0;
        if let Some(name) = primary.filter(|name| !name.is_empty()) {
            tx.execute(
                "INSERT INTO pasta_file(pasta_id,position,role,name,size) VALUES(?1,?2,'primary',?3,?4)",
                params![pasta_id, position, name, size.unwrap_or(0)],
            )
            .map_err(|e| e.to_string())?;
            position += 1;
        }
        if let Some(json) = attachments {
            let values: serde_json::Value =
                serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
            if let Some(files) = values.as_array() {
                for file in files {
                    let name = file.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let size = file.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    if !name.is_empty() {
                        tx.execute(
                            "INSERT INTO pasta_file(pasta_id,position,role,name,size) VALUES(?1,?2,'attachment',?3,?4)",
                            params![pasta_id, position, name, size],
                        )
                        .map_err(|e| e.to_string())?;
                        position += 1;
                    }
                }
            }
        }
    }
    tx.execute_batch("DROP TABLE pasta_legacy;")
        .map_err(|e| e.to_string())
}

fn normalize_api_key_scopes(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "UPDATE api_key SET scopes =
         replace(scopes, 'admin',
           'paste:admin,user:admin,invite:admin,key:admin')
         WHERE instr(',' || scopes || ',', ',admin,') > 0",
        [],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_repository(encrypted: bool) -> Repository {
        let path = std::env::temp_dir().join(format!("racebin-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let repository = Repository::open(&path).unwrap();
        let conn = repository.conn().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE app_user (
              id INTEGER PRIMARY KEY, username TEXT UNIQUE, password_hash TEXT,
              role TEXT, enabled INTEGER, force_password_change INTEGER, created INTEGER
            );
            INSERT INTO app_user VALUES(1,'owner','hash','admin',1,0,1);
            CREATE TABLE pasta (
              id INTEGER PRIMARY KEY, owner_user_id INTEGER, title TEXT, content TEXT,
              file_name TEXT, file_size INTEGER, extension TEXT, read_only INTEGER,
              private INTEGER, editable INTEGER, encrypt_server INTEGER,
              encrypt_client INTEGER, encrypted_key TEXT, created INTEGER,
              expiration INTEGER, last_read INTEGER, read_count INTEGER,
              burn_after_reads INTEGER, attachments TEXT, pasta_type TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pasta VALUES(
              42,1,'A title','content','notes.txt',7,'js',0,1,1,?1,0,NULL,
              100,0,100,3,0,'[{\"name\":\"extra.txt\",\"size\":4}]','text'
            )",
            params![encrypted as i32],
        )
        .unwrap();
        drop(conn);
        repository
    }

    #[test]
    fn migrates_supported_legacy_data_to_final_schema() {
        let repository = legacy_repository(false);
        repository.migrate().unwrap();
        let conn = repository.conn().unwrap();
        let row: (String, String, Option<i64>, i64) = conn
            .query_row(
                "SELECT slug,access,expiration,read_count FROM pasta WHERE id=42",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(row.0, crate::util::animalnumbers::to_animal_names(42));
        assert_eq!(row.1, "owner");
        assert_eq!(row.2, None);
        assert_eq!(row.3, 3);
        let files: i64 = conn
            .query_row(
                "SELECT count(*) FROM pasta_file WHERE pasta_id=42",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(files, 2);
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn preflight_leaves_unsupported_database_untouched() {
        let repository = legacy_repository(true);
        assert!(repository
            .migrate()
            .unwrap_err()
            .contains("migration refused"));
        let conn = repository.conn().unwrap();
        assert!(column_exists(&conn, "pasta", "encrypt_server").unwrap());
        assert!(!table_exists(&conn, "pasta_file").unwrap());
        assert_eq!(
            conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
