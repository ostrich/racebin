use sqlx::any::{install_default_drivers, AnyPoolOptions};
use sqlx::AnyPool;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once};
use tokio::sync::{Mutex, MutexGuard};

mod copy;
pub use copy::copy_database;

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
    write_lock: Arc<Mutex<()>>,
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
            write_lock: Arc::new(Mutex::new(())),
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

    pub async fn lock_writes(&self) -> MutexGuard<'_, ()> {
        self.write_lock.lock().await
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
        Ok(())
    }

    pub async fn purge_expired(&self, now: i64) -> Result<usize, String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM paste_read_grants WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM paste_read_receipts WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM idempotency_records WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM auth_attempts WHERE occurred_at<=$1-900")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        let paste_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM pastes
             WHERE (expires_at IS NOT NULL AND expires_at<=$1)
                OR (consumed_at IS NOT NULL AND consumed_at<=$1-900)",
        )
        .bind(now)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query(
            "DELETE FROM pastes
             WHERE (expires_at IS NOT NULL AND expires_at<=$1)
                OR (consumed_at IS NOT NULL AND consumed_at<=$1-900)",
        )
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM sessions WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM password_reset_tokens WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM invitations WHERE expires_at<=$1-2592000")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())?;
        for paste_id in &paste_ids {
            let _ = fs::remove_dir_all(self.data_dir.join("attachments").join(paste_id));
        }
        let valid: HashSet<String> = sqlx::query_scalar("SELECT id FROM pastes")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let attachment_root = self.data_dir.join("attachments");
        if let Ok(entries) = fs::read_dir(attachment_root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == ".staging" {
                    if let Ok(staged) = fs::read_dir(entry.path()) {
                        for file in staged.flatten() {
                            let stale = file
                                .metadata()
                                .and_then(|metadata| metadata.modified())
                                .and_then(|modified| {
                                    modified.elapsed().map_err(std::io::Error::other)
                                })
                                .is_ok_and(|age| age.as_secs() >= 3600);
                            if stale {
                                let _ = fs::remove_file(file.path());
                            }
                        }
                    }
                    continue;
                }
                if entry.path().is_dir() && !valid.contains(&name) {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
        Ok(paste_ids.len())
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

include!("repository/tests.rs");
