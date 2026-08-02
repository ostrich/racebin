use super::service::{
    folder_database_error, folder_name_key, folder_principal, validate_folder_name,
};
use super::{DomainError, DomainResult, Folder, FolderOverview, PasteService, Principal};
use crate::time::unix_timestamp;
use std::collections::HashSet;

impl PasteService {
    pub async fn list_folders(&self, principal: &Principal) -> DomainResult<FolderOverview> {
        let owner = folder_principal(principal, "paste:list")?;
        let items = sqlx::query_as(
            "SELECT f.id,f.owner_id,f.name,f.created_at,
                    (SELECT count(*) FROM pastes p WHERE p.folder_id=f.id) AS paste_count
             FROM folders f WHERE f.owner_id=$1 ORDER BY f.name_key,f.id",
        )
        .bind(owner)
        .fetch_all(self.storage.pool())
        .await
        .map_err(DomainError::internal)?;
        let (total_count, unfiled_count): (i64, i64) = sqlx::query_as(
            "SELECT count(*),coalesce(sum(CASE WHEN folder_id IS NULL THEN 1 ELSE 0 END),0)
             FROM pastes WHERE owner_id=$1",
        )
        .bind(owner)
        .fetch_one(self.storage.pool())
        .await
        .map_err(DomainError::internal)?;
        Ok(FolderOverview {
            items,
            total_count,
            unfiled_count,
        })
    }

    pub async fn create_folder(&self, principal: &Principal, name: &str) -> DomainResult<Folder> {
        let owner = folder_principal(principal, "paste:write")?;
        let name = validate_folder_name(name)?;
        let name_key = folder_name_key(name);
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO folders(owner_id,name,name_key,created_at) VALUES($1,$2,$3,$4) RETURNING id",
        )
        .bind(owner)
        .bind(name)
        .bind(name_key)
        .bind(unix_timestamp())
        .fetch_one(self.storage.pool())
        .await
        .map_err(folder_database_error)?;
        self.folder_by_id(owner, id)
            .await?
            .ok_or_else(|| DomainError::internal("Folder creation failed"))
    }

    pub async fn rename_folder(
        &self,
        principal: &Principal,
        id: i64,
        name: &str,
    ) -> DomainResult<Option<Folder>> {
        let owner = folder_principal(principal, "paste:write")?;
        let name = validate_folder_name(name)?;
        let changed =
            sqlx::query("UPDATE folders SET name=$3,name_key=$4 WHERE id=$1 AND owner_id=$2")
                .bind(id)
                .bind(owner)
                .bind(name)
                .bind(folder_name_key(name))
                .execute(self.storage.pool())
                .await
                .map_err(folder_database_error)?
                .rows_affected();
        if changed == 0 {
            Ok(None)
        } else {
            Ok(self.folder_by_id(owner, id).await?)
        }
    }

    pub async fn delete_folder(
        &self,
        principal: &Principal,
        id: i64,
    ) -> DomainResult<Option<Vec<(String, i64)>>> {
        let owner = folder_principal(principal, "paste:write")?;
        let mut transaction = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(DomainError::internal)?;
        let revisions = sqlx::query_as::<_, (String, i64)>(
            "UPDATE pastes SET folder_id=NULL,updated_at=$3,revision=revision+1
             WHERE folder_id=$1 AND owner_id=$2 RETURNING id,revision",
        )
        .bind(id)
        .bind(owner)
        .bind(unix_timestamp())
        .fetch_all(&mut *transaction)
        .await
        .map_err(DomainError::internal)?;
        let deleted = sqlx::query("DELETE FROM folders WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(owner)
            .execute(&mut *transaction)
            .await
            .map_err(DomainError::internal)?
            .rows_affected()
            == 1;
        transaction.commit().await.map_err(DomainError::internal)?;
        Ok(deleted.then_some(revisions))
    }

    pub async fn move_pastes(
        &self,
        principal: &Principal,
        paste_ids: &[String],
        folder_id: Option<i64>,
    ) -> DomainResult<Vec<(String, i64)>> {
        let owner = folder_principal(principal, "paste:write")?;
        if paste_ids.is_empty() || paste_ids.len() > crate::limits::MAX_BULK_PASTES {
            return Err(DomainError::validation("Select between 1 and 100 pastes"));
        }
        let unique = paste_ids.iter().collect::<HashSet<_>>();
        if unique.len() != paste_ids.len() {
            return Err(DomainError::validation("Paste IDs must be unique"));
        }
        self.validate_folder_owner(owner, folder_id).await?;
        let _guard = self.storage.lock_writes().await;
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(DomainError::internal)?;
        let mut revisions = Vec::with_capacity(paste_ids.len());
        for id in paste_ids {
            let revision = sqlx::query_scalar::<_, i64>(
                "UPDATE pastes SET folder_id=$3,updated_at=$4,revision=revision+1
                 WHERE id=$1 AND owner_id=$2 RETURNING revision",
            )
            .bind(id)
            .bind(owner)
            .bind(folder_id)
            .bind(unix_timestamp())
            .fetch_optional(&mut *tx)
            .await
            .map_err(DomainError::internal)?
            .ok_or_else(|| DomainError::not_found("One or more pastes were not found"))?;
            revisions.push((id.clone(), revision));
        }
        tx.commit().await.map_err(DomainError::internal)?;
        Ok(revisions)
    }

    pub(super) async fn validate_folder_owner(
        &self,
        owner: i64,
        folder_id: Option<i64>,
    ) -> DomainResult<()> {
        let Some(id) = folder_id else {
            return Ok(());
        };
        if self.folder_by_id(owner, id).await?.is_none() {
            return Err(DomainError::not_found("Folder not found"));
        }
        Ok(())
    }

    pub(super) async fn folder_by_id(&self, owner: i64, id: i64) -> DomainResult<Option<Folder>> {
        sqlx::query_as(
            "SELECT f.id,f.owner_id,f.name,f.created_at,
                    (SELECT count(*) FROM pastes p WHERE p.folder_id=f.id) AS paste_count
             FROM folders f WHERE f.id=$1 AND f.owner_id=$2",
        )
        .bind(id)
        .bind(owner)
        .fetch_optional(self.storage.pool())
        .await
        .map_err(DomainError::internal)
    }
}
