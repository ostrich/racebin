use super::validation::authorize_owner;
use super::{Attachment, DomainError, DomainResult, NewAttachment, PasteService, Principal};
use crate::time::unix_timestamp;
use std::collections::HashSet;

impl PasteService {
    pub async fn delete_paste(
        &self,
        principal: &Principal,
        id: &str,
        expected_revision: Option<i64>,
    ) -> DomainResult<bool> {
        let _write_guard = self.storage.lock_writes().await;
        let current = match self.find_paste(id).await? {
            Some(value) => value,
            None => return Ok(false),
        };
        authorize_owner(principal, &current, "paste:delete")?;
        let result = sqlx::query("DELETE FROM pastes WHERE id=$1 AND ($2 IS NULL OR revision=$2)")
            .bind(id)
            .bind(expected_revision)
            .execute(self.storage.pool())
            .await
            .map_err(DomainError::internal)?;
        if result.rows_affected() == 1 {
            let directory = self.storage.data_dir.join("attachments").join(id);
            let _ = std::fs::remove_dir_all(directory);
            Ok(true)
        } else if expected_revision.is_some() {
            Err(DomainError::precondition("Paste revision changed"))
        } else {
            Ok(false)
        }
    }

    pub async fn add_attachments(
        &self,
        principal: &Principal,
        id: &str,
        inputs: &[NewAttachment],
        expected_revision: Option<i64>,
    ) -> DomainResult<Vec<Attachment>> {
        let _write_guard = self.storage.lock_writes().await;
        let paste = self
            .find_paste(id)
            .await?
            .ok_or_else(|| DomainError::not_found("Paste not found"))?;
        authorize_owner(principal, &paste, "paste:write")?;
        if inputs.is_empty() {
            return Err(DomainError::unprocessable(
                "invalid_attachment",
                "At least one attachment is required",
            ));
        }
        if paste.attachments.len().saturating_add(inputs.len())
            > crate::limits::MAX_ATTACHMENTS_PER_PASTE
        {
            return Err(DomainError::payload_too_large(
                "too_many_attachments",
                format!(
                    "A paste may contain at most {} attachments",
                    crate::limits::MAX_ATTACHMENTS_PER_PASTE
                ),
            ));
        }
        let mut names = paste
            .attachments
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<HashSet<_>>();
        for input in inputs {
            if input.filename.is_empty()
                || input.size_bytes < 0
                || input.storage_key.starts_with('.')
                || std::path::Path::new(&input.storage_key)
                    .components()
                    .count()
                    != 1
            {
                return Err(DomainError::unprocessable(
                    "invalid_attachment",
                    "Attachment metadata is invalid",
                ));
            }
            if !names.insert(&input.filename) {
                return Err(DomainError::conflict(
                    "attachment_exists",
                    format!("{} already exists", input.filename),
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
        for (offset, input) in inputs.iter().enumerate() {
            let sort_order = starting_sort_order + offset as i64;
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO attachments(paste_id,sort_order,filename,storage_key,size_bytes)
                 VALUES($1,$2,$3,$4,$5) RETURNING id",
            )
            .bind(&paste.id)
            .bind(sort_order)
            .bind(&input.filename)
            .bind(&input.storage_key)
            .bind(input.size_bytes)
            .fetch_one(&mut *tx)
            .await
            .map_err(DomainError::internal)?;
            attachments.push(Attachment {
                id,
                sort_order,
                filename: input.filename.clone(),
                storage_key: input.storage_key.clone(),
                size_bytes: input.size_bytes,
            });
        }
        let changed = sqlx::query(
            "UPDATE pastes SET updated_at=$2,modified_at=$2,revision=revision+1
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
        let _write_guard = self.storage.lock_writes().await;
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
                    "UPDATE pastes SET updated_at=$2,modified_at=$2,revision=revision+1
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
                let _ = std::fs::remove_file(path);
                Ok(Some(revision))
            }
            other => other,
        }
    }
}
