use super::validation::authorize_owner;
use super::{Attachment, DomainError, DomainResult, PasteService, Principal};
use crate::time::unix_timestamp;
use std::collections::HashSet;
use uuid::Uuid;

impl PasteService {
    pub async fn delete_paste(
        &self,
        principal: &Principal,
        id: &str,
        expected_revision: Option<i64>,
    ) -> DomainResult<bool> {
        let current = match self.find_paste(id).await? {
            Some(value) => value,
            None => return Ok(false),
        };
        authorize_owner(principal, &current, "paste:delete")?;
        let directory = self.storage.data_dir.join("attachments").join(id);
        let staged = self
            .storage
            .data_dir
            .join("attachments")
            .join(format!(".delete-{}", Uuid::new_v4()));
        let had_directory = directory.exists();
        if had_directory {
            std::fs::rename(&directory, &staged).map_err(DomainError::internal)?;
        }
        match sqlx::query("DELETE FROM pastes WHERE id=$1 AND ($2 IS NULL OR revision=$2)")
            .bind(id)
            .bind(expected_revision)
            .execute(self.storage.pool())
            .await
        {
            Ok(result) if result.rows_affected() == 1 => {
                if had_directory {
                    let _ = std::fs::remove_dir_all(staged);
                }
                Ok(true)
            }
            Ok(_) => {
                if had_directory {
                    let _ = std::fs::rename(staged, directory);
                }
                if expected_revision.is_some() {
                    Err(DomainError::precondition("Paste revision changed"))
                } else {
                    Ok(false)
                }
            }
            Err(error) => {
                if had_directory {
                    let _ = std::fs::rename(staged, directory);
                }
                Err(DomainError::internal(error))
            }
        }
    }

    pub async fn add_attachments(
        &self,
        principal: &Principal,
        id: &str,
        inputs: &[(String, String, i64)],
        expected_revision: Option<i64>,
    ) -> DomainResult<Vec<Attachment>> {
        let _write_guard = self.storage.lock_writes().await;
        let paste = self
            .find_paste(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Paste not found"))?;
        authorize_owner(principal, &paste, "paste:write")?;
        let mut names = paste
            .attachments
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<HashSet<_>>();
        for (name, _, _) in inputs {
            if !names.insert(name) {
                return Err(DomainError::conflict(
                    "attachment_exists",
                    format!("{name} already exists"),
                ));
            }
        }
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(DomainError::internal)?;
        let starting_sort_order: i64 = sqlx::query_scalar(
            "SELECT coalesce(max(sort_order)+1,0) FROM attachments WHERE paste_id=$1",
        )
        .bind(&paste.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(DomainError::internal)?;
        let mut attachments = Vec::with_capacity(inputs.len());
        for (offset, (filename, storage_key, size_bytes)) in inputs.iter().enumerate() {
            let sort_order = starting_sort_order + offset as i64;
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO attachments(paste_id,sort_order,filename,storage_key,size_bytes)
                 VALUES($1,$2,$3,$4,$5) RETURNING id",
            )
            .bind(&paste.id)
            .bind(sort_order)
            .bind(filename)
            .bind(storage_key)
            .bind(size_bytes)
            .fetch_one(&mut *tx)
            .await
            .map_err(DomainError::internal)?;
            attachments.push(Attachment {
                id,
                sort_order,
                filename: filename.clone(),
                storage_key: storage_key.clone(),
                size_bytes: *size_bytes,
            });
        }
        let changed = sqlx::query(
            "UPDATE pastes SET updated_at=$2,revision=revision+1
             WHERE id=$1 AND ($3 IS NULL OR revision=$3)",
        )
        .bind(&paste.id)
        .bind(unix_timestamp())
        .bind(expected_revision)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::internal)?
        .rows_affected();
        if changed == 0 {
            return Err(DomainError::precondition("Paste revision changed"));
        }
        tx.commit().await.map_err(DomainError::internal)?;
        Ok(attachments)
    }

    pub async fn delete_attachment(
        &self,
        principal: &Principal,
        id: &str,
        attachment_id: i64,
        expected_revision: Option<i64>,
    ) -> DomainResult<Option<i64>> {
        let paste = self
            .find_paste(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Paste not found"))?;
        authorize_owner(principal, &paste, "paste:write")?;
        let attachment = paste
            .attachments
            .into_iter()
            .find(|attachment| attachment.id == attachment_id);
        let Some(attachment) = attachment else {
            return Ok(None);
        };
        if attachment.storage_key.starts_with('.')
            || std::path::Path::new(&attachment.storage_key)
                .components()
                .count()
                != 1
        {
            return Err(DomainError::internal("Unsafe attachment metadata"));
        }
        let path = self
            .storage
            .data_dir
            .join("attachments")
            .join(&paste.id)
            .join(&attachment.storage_key);
        let staged = path.with_file_name(format!(".delete-{}", Uuid::new_v4()));
        let existed = path.exists();
        if existed {
            std::fs::rename(&path, &staged).map_err(DomainError::internal)?;
        }
        let mut transaction = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(DomainError::internal)?;
        let result = async {
            let affected = sqlx::query("DELETE FROM attachments WHERE id=$1")
                .bind(attachment_id)
                .execute(&mut *transaction)
                .await
                .map_err(DomainError::internal)?
                .rows_affected();
            if affected == 1 {
                let revision = sqlx::query_scalar::<_, i64>(
                    "UPDATE pastes SET updated_at=$2,revision=revision+1
                     WHERE id=$1 AND ($3 IS NULL OR revision=$3)
                     RETURNING revision",
                )
                .bind(&paste.id)
                .bind(unix_timestamp())
                .bind(expected_revision)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(DomainError::internal)?
                .ok_or_else(|| DomainError::precondition("Paste revision changed"))?;
                transaction.commit().await.map_err(DomainError::internal)?;
                return Ok::<Option<i64>, DomainError>(Some(revision));
            }
            transaction.commit().await.map_err(DomainError::internal)?;
            Ok::<Option<i64>, DomainError>(None)
        }
        .await;
        match result {
            Ok(Some(revision)) => {
                if existed {
                    let _ = std::fs::remove_file(staged);
                }
                Ok(Some(revision))
            }
            Ok(None) => {
                if existed {
                    let _ = std::fs::rename(staged, path);
                }
                Ok(None)
            }
            Err(error) => {
                if existed {
                    let _ = std::fs::rename(staged, path);
                }
                Err(error)
            }
        }
    }
}
