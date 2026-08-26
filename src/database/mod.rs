use sqlx::any::{install_default_drivers, AnyPoolOptions};
use sqlx::AnyPool;
use std::collections::HashSet;
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
pub struct Database {
    pool: AnyPool,
    kind: DatabaseKind,
    write_lock: Arc<Mutex<()>>,
    pub data_dir: PathBuf,
}

impl Database {
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
        self.prepare_markdown_migration().await?;
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

    async fn prepare_markdown_migration(&self) -> Result<(), String> {
        let has_document = match self.kind {
            DatabaseKind::Sqlite => sqlx::query("SELECT name FROM pragma_table_info('pastes') WHERE name='document_json'")
                .fetch_optional(&self.pool).await.map_err(|error| error.to_string())?.is_some(),
            DatabaseKind::Postgres => sqlx::query("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='pastes' AND column_name='document_json'")
                .fetch_optional(&self.pool).await.map_err(|error| error.to_string())?.is_some(),
        };
        if !has_document {
            return Ok(());
        }
        let rows =
            sqlx::query("SELECT id,document_json FROM pastes WHERE content_kind='rich_text'")
                .fetch_all(&self.pool)
                .await
                .map_err(|error| error.to_string())?;
        if rows.is_empty() {
            return Ok(());
        }
        use sqlx::Row;
        let has_revision = match self.kind {
            DatabaseKind::Sqlite => sqlx::query("SELECT name FROM pragma_table_info('pastes') WHERE name='revision'").fetch_optional(&self.pool).await.map_err(|e| e.to_string())?.is_some(),
            DatabaseKind::Postgres => sqlx::query("SELECT column_name FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='pastes' AND column_name='revision'").fetch_optional(&self.pool).await.map_err(|e| e.to_string())?.is_some(),
        };
        let mut converted = Vec::new();
        for row in rows {
            let id: String = row.try_get("id").map_err(|e| e.to_string())?;
            let encoded: String = row
                .try_get("document_json")
                .map_err(|_| format!("Rich-text paste {id} has no document"))?;
            let document = serde_json::from_str(&encoded)
                .map_err(|error| format!("Rich-text paste {id} is invalid: {error}"))?;
            let markdown = crate::pastes::document_to_markdown(&document)
                .map_err(|error| format!("Cannot migrate paste {id}: {error}"))?;
            crate::pastes::render_markdown(&markdown)
                .map_err(|error| format!("Cannot migrate paste {id}: {error}"))?;
            converted.push((id, markdown));
        }
        let mut transaction = self.pool.begin().await.map_err(|e| e.to_string())?;
        for (id, markdown) in converted {
            let statement = if has_revision {
                "UPDATE pastes SET revision=revision+CASE WHEN content<>$1 THEN 1 ELSE 0 END,content=$1 WHERE id=$2"
            } else {
                "UPDATE pastes SET content=$1 WHERE id=$2"
            };
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .bind(markdown)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().await.map_err(|e| e.to_string())
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
            let _ =
                tokio::fs::remove_dir_all(self.data_dir.join("attachments").join(paste_id)).await;
        }
        let valid: HashSet<String> = sqlx::query_scalar("SELECT id FROM pastes")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .collect();
        let attachment_root = self.data_dir.join("attachments");
        if let Ok(mut entries) = tokio::fs::read_dir(attachment_root).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == ".staging" {
                    if let Ok(mut staged) = tokio::fs::read_dir(entry.path()).await {
                        while let Ok(Some(file)) = staged.next_entry().await {
                            let stale = file
                                .metadata()
                                .await
                                .and_then(|metadata| metadata.modified())
                                .and_then(|modified| {
                                    modified.elapsed().map_err(std::io::Error::other)
                                })
                                .is_ok_and(|age| age.as_secs() >= 3600);
                            if stale {
                                let _ = tokio::fs::remove_file(file.path()).await;
                            }
                        }
                    }
                    continue;
                }
                if entry.file_type().await.is_ok_and(|kind| kind.is_dir()) && !valid.contains(&name)
                {
                    let _ = tokio::fs::remove_dir_all(entry.path()).await;
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

#[cfg(test)]
mod tests;
