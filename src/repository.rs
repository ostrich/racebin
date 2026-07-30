use sqlx::any::{install_default_drivers, AnyPoolOptions};
use sqlx::{AnyPool, Row};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

static INSTALL_DRIVERS: Once = Once::new();
static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/sqlite");
static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("migrations/postgres");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseKind {
    Sqlite,
    Postgres,
}

#[derive(Clone)]
pub struct Repository {
    pool: AnyPool,
    kind: DatabaseKind,
    pub data_dir: PathBuf,
}

impl Repository {
    pub async fn open(database_url: &str, data_dir: impl AsRef<Path>) -> Result<Self, String> {
        INSTALL_DRIVERS.call_once(install_default_drivers);
        let kind = database_kind(database_url)?;
        let pool = AnyPoolOptions::new()
            .max_connections(if kind == DatabaseKind::Sqlite { 16 } else { 32 })
            .after_connect(move |connection, _| {
                Box::pin(async move {
                    if kind == DatabaseKind::Sqlite {
                        sqlx::query("PRAGMA foreign_keys=ON")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("PRAGMA journal_mode=WAL")
                            .execute(&mut *connection)
                            .await?;
                        sqlx::query("PRAGMA busy_timeout=5000")
                            .execute(&mut *connection)
                            .await?;
                    }
                    Ok(())
                })
            })
            .connect(database_url)
            .await
            .map_err(|error| format!("database connection failed: {error}"))?;
        let repository = Self {
            pool,
            kind,
            data_dir: data_dir.as_ref().to_path_buf(),
        };
        Ok(repository)
    }

    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    pub fn kind(&self) -> DatabaseKind {
        self.kind
    }

    pub async fn migrate(&self) -> Result<(), String> {
        if self.kind == DatabaseKind::Sqlite {
            SQLITE_MIGRATOR
                .run(&self.pool)
                .await
                .map_err(|error| format!("SQLite migration failed: {error}"))?;
        } else {
            POSTGRES_MIGRATOR
                .run(&self.pool)
                .await
                .map_err(|error| format!("PostgreSQL migration failed: {error}"))?;
        }
        sqlx::query(
            "UPDATE api_key SET scopes =
             replace(scopes, 'admin', 'paste:admin,user:admin,invite:admin,key:admin')
             WHERE ',' || scopes || ',' LIKE '%,admin,%'",
        )
        .execute(&self.pool)
        .await
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub async fn purge_expired(&self, now: i64) -> Result<usize, String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        let slugs: Vec<String> = sqlx::query_scalar(
            "SELECT slug FROM pasta WHERE expiration IS NOT NULL AND expiration<=$1",
        )
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM pasta WHERE expiration IS NOT NULL AND expiration<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM user_session WHERE expires<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM user_invite WHERE expires<=$1-2592000")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        for slug in &slugs {
            let _ = fs::remove_dir_all(self.data_dir.join("attachments").join(slug));
        }
        let valid: HashSet<String> = sqlx::query_scalar("SELECT slug FROM pasta")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let attachment_root = self.data_dir.join("attachments");
        if let Ok(entries) = fs::read_dir(attachment_root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().is_dir() && !valid.contains(&name) {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
        Ok(slugs.len())
    }
}

pub fn database_kind(url: &str) -> Result<DatabaseKind, String> {
    if url.starts_with("sqlite:") {
        Ok(DatabaseKind::Sqlite)
    } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
        Ok(DatabaseKind::Postgres)
    } else {
        Err("database URL must use sqlite, postgres, or postgresql".to_string())
    }
}

pub async fn copy_database(
    source_url: &str,
    destination_url: &str,
    data_dir: impl AsRef<Path>,
) -> Result<(), String> {
    let data_dir = data_dir.as_ref();
    let source = Repository::open(source_url, data_dir).await?;
    let destination = Repository::open(destination_url, data_dir).await?;
    if source.kind == destination.kind && source_url == destination_url {
        return Err("source and destination databases must differ".to_string());
    }
    source.migrate().await?;
    destination.migrate().await?;
    let occupied: i64 = sqlx::query_scalar(
        "SELECT
          (SELECT count(*) FROM app_user) +
          (SELECT count(*) FROM user_session) +
          (SELECT count(*) FROM user_invite) +
          (SELECT count(*) FROM api_key) +
          (SELECT count(*) FROM pasta) +
          (SELECT count(*) FROM pasta_file)",
    )
    .fetch_one(destination.pool())
    .await
    .map_err(|e| e.to_string())?;
    if occupied != 0 {
        return Err("destination database is not empty".to_string());
    }

    let mut source_tx = source.pool.begin().await.map_err(|e| e.to_string())?;
    let users = sqlx::query(
        "SELECT id,username,password_hash,role,enabled,force_password_change,created FROM app_user",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let sessions = sqlx::query(
        "SELECT id,user_id,token_hash,csrf_token,created,expires,last_used FROM user_session",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let invites =
        sqlx::query("SELECT id,token_hash,created_by,expires,used,revoked FROM user_invite")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|e| e.to_string())?;
    let keys = sqlx::query(
        "SELECT id,user_id,name,prefix,token_hash,scopes,created,last_used,enabled FROM api_key",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let pastes = sqlx::query(
        "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,expiration,
                last_read,read_count,burn_after_reads FROM pasta",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let files =
        sqlx::query("SELECT id,pasta_id,position,role,name,storage_name,size FROM pasta_file")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|e| e.to_string())?;

    for row in &files {
        let pasta_id: i64 = row.try_get("pasta_id").map_err(|e| e.to_string())?;
        let storage_name: String = row.try_get("storage_name").map_err(|e| e.to_string())?;
        let slug = pastes
            .iter()
            .find_map(|paste| {
                (paste.try_get::<i64, _>("id").ok() == Some(pasta_id))
                    .then(|| paste.try_get::<String, _>("slug").ok())
                    .flatten()
            })
            .ok_or_else(|| format!("file {storage_name:?} references missing paste {pasta_id}"))?;
        if !data_dir
            .join("attachments")
            .join(&slug)
            .join(&storage_name)
            .is_file()
        {
            return Err(format!(
                "attachment {storage_name:?} for paste {slug:?} is missing from data-dir"
            ));
        }
    }

    let counts = [
        users.len(),
        sessions.len(),
        invites.len(),
        keys.len(),
        pastes.len(),
        files.len(),
    ];
    let mut tx = destination.pool.begin().await.map_err(|e| e.to_string())?;
    for row in users {
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("username").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("password_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("role").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("force_password_change")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("created").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in sessions {
        sqlx::query(
            "INSERT INTO user_session(id,user_id,token_hash,csrf_token,created,expires,last_used)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("csrf_token")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("last_used")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in invites {
        sqlx::query(
            "INSERT INTO user_invite(id,token_hash,created_by,expires,used,revoked)
             VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created_by")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("used").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("revoked")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in keys {
        sqlx::query(
            "INSERT INTO api_key(id,user_id,name,prefix,token_hash,scopes,created,last_used,enabled)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("user_id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("name").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("prefix").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("token_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("scopes").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("created").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("last_used").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in pastes {
        sqlx::query(
            "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,created,
                               expiration,last_read,read_count,burn_after_reads)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("slug")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("owner_user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("title")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("content")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("kind")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("syntax")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("access")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("expiration")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("last_read")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("read_count")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("burn_after_reads")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in files {
        sqlx::query(
            "INSERT INTO pasta_file(id,pasta_id,position,role,name,storage_name,size)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("pasta_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("position")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("role")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("name")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("storage_name")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("size").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    if destination.kind == DatabaseKind::Postgres {
        for table in [
            "app_user",
            "user_session",
            "user_invite",
            "api_key",
            "pasta",
            "pasta_file",
        ] {
            sqlx::query(&format!(
                "SELECT setval(pg_get_serial_sequence('{table}','id'),
                               coalesce((SELECT max(id) FROM {table}),1),
                               EXISTS(SELECT 1 FROM {table}))"
            ))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    for (table, expected) in [
        ("app_user", counts[0]),
        ("user_session", counts[1]),
        ("user_invite", counts[2]),
        ("api_key", counts[3]),
        ("pasta", counts[4]),
        ("pasta_file", counts[5]),
    ] {
        let actual: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if actual != expected as i64 {
            return Err(format!(
                "copy verification failed for {table}: expected {expected}, got {actual}"
            ));
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    source_tx.rollback().await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn run_cli_if_requested() -> Result<bool, String> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) != Some("database")
        || arguments.get(2).map(String::as_str) != Some("copy")
    {
        return Ok(false);
    }
    let option = |name: &str| {
        arguments
            .iter()
            .position(|value| value == name)
            .and_then(|index| arguments.get(index + 1))
            .cloned()
    };
    let source = option("--from").ok_or("database copy requires --from URL")?;
    let destination = option("--to").ok_or("database copy requires --to URL")?;
    let data_dir = option("--data-dir")
        .or_else(|| std::env::var("RACEBIN_DATA_DIR").ok())
        .unwrap_or_else(|| "racebin_data".to_string());
    copy_database(&source, &destination, &data_dir).await?;
    println!("database copy completed and verified");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{copy_database, Repository};

    #[actix_web::test]
    async fn sqlite_schema_is_repeatable() {
        let data_dir =
            std::env::temp_dir().join(format!("racebin-schema-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &data_dir).await.unwrap();
        repository.migrate().await.unwrap();
        repository.migrate().await.unwrap();
        let tables: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master
             WHERE type='table' AND name IN ('app_user','pasta','pasta_file','api_key')",
        )
        .fetch_one(repository.pool())
        .await
        .unwrap();
        assert_eq!(tables, 4);
        drop(repository);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[actix_web::test]
    async fn copies_sqlite_into_postgres_when_test_database_is_configured() {
        let Ok(postgres_url) = std::env::var("RACEBIN_TEST_POSTGRES_URL") else {
            return;
        };
        let data_dir = std::env::temp_dir().join(format!("racebin-copy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let sqlite_url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let source = Repository::open(&sqlite_url, &data_dir).await.unwrap();
        source.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES(42,'copy-test','hash','admin',1,0,123)",
        )
        .execute(source.pool())
        .await
        .unwrap();
        drop(source);

        copy_database(&sqlite_url, &postgres_url, &data_dir)
            .await
            .unwrap();
        let destination = Repository::open(&postgres_url, &data_dir).await.unwrap();
        let user: (i64, String, i64) =
            sqlx::query_as("SELECT id,username,enabled FROM app_user WHERE id=42")
                .fetch_one(destination.pool())
                .await
                .unwrap();
        assert_eq!(user, (42, "copy-test".to_string(), 1));
        drop(destination);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
