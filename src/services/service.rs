use crate::account::{self as accounts, api_keys};
use crate::repository::Repository;
use sqlx::{Any, Executor};
use std::collections::HashSet;
use uuid::Uuid;

use super::model::{
    Attachment, Folder, FolderOverview, Page, Paste, PasteInput, PasteQuery, PasteRead,
};
use super::rich_text::validate_document;
use super::validation::{authorize_owner, can_read, validate_input};
use crate::time::unix_timestamp;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct PasteService {
    pub storage: Repository,
}

#[derive(Clone, Debug)]
pub enum Principal {
    Anonymous,
    Session(accounts::SessionUser),
    ApiKey(api_keys::ApiKey),
}

impl Principal {
    pub fn user_id(&self) -> Option<i64> {
        match self {
            Self::Session(session) => Some(session.user.id),
            Self::ApiKey(key) => key.user_id,
            Self::Anonymous => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Session(session) if session.user.is_admin())
    }

    pub fn can(&self, scope: &str) -> bool {
        self.is_admin() || matches!(self, Self::ApiKey(key) if key.has_scope(scope))
    }
}

impl PasteService {
    pub fn new(storage: Repository) -> Self {
        Self { storage }
    }

    pub async fn list_pastes(
        &self,
        principal: &Principal,
        query: &PasteQuery,
        admin: bool,
    ) -> Result<Page<Paste>, String> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
        let offset = i64::from(page - 1).saturating_mul(i64::from(page_size));
        let user_id = principal.user_id();
        let search = format!("%{}%", query.search.as_deref().unwrap_or(""));
        let visibility = query.visibility.as_deref();
        let content_kind = query.content_kind.as_deref();
        let language = query.language.as_deref();
        let has_attachments = query.has_attachments.map(i64::from);
        let expiration = query
            .expiration
            .as_deref()
            .map(|value| i64::from(value == "scheduled"));
        let limited = query
            .read_limit
            .as_deref()
            .map(|value| i64::from(value == "limited"));
        let owner = if query.mine.unwrap_or(false) {
            user_id
        } else {
            query.owner_id
        };
        if let Some(folder_id) = query.folder_id {
            let owner = user_id.ok_or("Folder filters require an owner")?;
            if self.folder_by_id(owner, folder_id).await?.is_none() {
                return Err("Folder not found".into());
            }
        }
        let folder_id = query.folder_id;
        let unfiled = query.unfiled.map(i64::from);
        let text_size = if self.storage.kind() == crate::repository::DatabaseKind::Postgres {
            "CAST(octet_length(content) AS BIGINT) + \
             COALESCE(CAST(octet_length(document_json) AS BIGINT),0)"
        } else {
            "length(CAST(content AS BLOB)) + \
             COALESCE(length(CAST(document_json AS BLOB)),0)"
        };
        let total_size = format!(
            "CAST(({text_size} + COALESCE((SELECT sum(size_bytes) FROM attachments size_files \
             WHERE size_files.paste_id=pastes.id),0)) AS BIGINT)"
        );
        let order = match query.sort.as_deref().unwrap_or("created") {
            "title" => "lower(title)",
            "reads" => "read_count",
            "expires" => "expires_at",
            "size" => total_size.as_str(),
            _ => "created_at",
        };
        let direction = if query.direction.as_deref() == Some("asc") {
            "ASC"
        } else {
            "DESC"
        };
        let filter = format!(
            "consumed_at IS NULL
             AND (($1=1) OR (($2 IS NULL AND visibility='public') OR
              ($2 IS NOT NULL AND (visibility='public' OR owner_id=$2))))
             AND ($3 IS NULL OR visibility=$3)
             AND ($4 IS NULL OR owner_id=$4)
             AND (expires_at IS NULL OR expires_at>$5)
             AND (lower(title) LIKE lower($6) OR lower(content) LIKE lower($6)
                  OR lower(id) LIKE lower($6) OR lower(language) LIKE lower($6)
                  OR lower(content_kind) LIKE lower($6)
                  OR EXISTS(SELECT 1 FROM attachments search_files
                            WHERE search_files.paste_id=pastes.id
                              AND lower(search_files.filename) LIKE lower($6))
                  OR ($1=1 AND EXISTS(SELECT 1 FROM users search_owner
                                     WHERE search_owner.id=pastes.owner_id
                                       AND lower(search_owner.username) LIKE lower($6))))
             AND ($7 IS NULL OR content_kind=$7)
             AND ($8 IS NULL OR language=$8)
             AND ($9 IS NULL OR ($9=1 AND EXISTS(SELECT 1 FROM attachments filter_files
                                                 WHERE filter_files.paste_id=pastes.id))
                              OR ($9=0 AND NOT EXISTS(SELECT 1 FROM attachments filter_files
                                                     WHERE filter_files.paste_id=pastes.id)))
             AND ($10 IS NULL OR created_at>=$10)
             AND ($11 IS NULL OR created_at<=$11)
             AND ($12 IS NULL OR ($12=0 AND expires_at IS NULL)
                               OR ($12=1 AND expires_at IS NOT NULL))
             AND ($13 IS NULL OR read_count>=$13)
             AND ($14 IS NULL OR read_count<=$14)
             AND ($15 IS NULL OR ($15=0 AND read_limit IS NULL)
                               OR ($15=1 AND read_limit IS NOT NULL))
             AND ($16 IS NULL OR {total_size}>=$16)
             AND ($17 IS NULL OR {total_size}<=$17)
             AND ($18 IS NULL OR folder_id=$18)
             AND ($19 IS NULL OR ($19=1 AND folder_id IS NULL))"
        );
        let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM pastes WHERE {filter}"))
            .bind(i64::from(admin))
            .bind(user_id)
            .bind(visibility)
            .bind(owner)
            .bind(unix_timestamp())
            .bind(&search)
            .bind(content_kind)
            .bind(language)
            .bind(has_attachments)
            .bind(query.created_after)
            .bind(query.created_before)
            .bind(expiration)
            .bind(query.min_reads)
            .bind(query.max_reads)
            .bind(limited)
            .bind(query.min_size_bytes)
            .bind(query.max_size_bytes)
            .bind(folder_id)
            .bind(unfiled)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| e.to_string())?;
        let items = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,owner_id,folder_id,title,substr(content,1,500) AS content,NULL AS document_json,
                    content_kind,language,visibility,created_at,updated_at,revision,consumed_at,
                    expires_at,last_read_at,read_count,read_limit,
                    (SELECT count(*) FROM attachments summary_files
                     WHERE summary_files.paste_id=pastes.id) AS attachment_count,
                    {total_size} AS size_bytes
             FROM pastes WHERE {filter} ORDER BY {order} {direction},id ASC LIMIT $20 OFFSET $21"
        ))
        .bind(i64::from(admin))
        .bind(user_id)
        .bind(visibility)
        .bind(owner)
        .bind(unix_timestamp())
        .bind(search)
        .bind(content_kind)
        .bind(language)
        .bind(has_attachments)
        .bind(query.created_after)
        .bind(query.created_before)
        .bind(expiration)
        .bind(query.min_reads)
        .bind(query.max_reads)
        .bind(limited)
        .bind(query.min_size_bytes)
        .bind(query.max_size_bytes)
        .bind(folder_id)
        .bind(unfiled)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        Ok(Page {
            items: items
                .into_iter()
                .map(|paste| redact_folder(principal, paste, admin))
                .collect(),
            page,
            page_size,
            total_items: total,
        })
    }

    pub async fn get_paste(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<Option<Paste>, String> {
        Ok(self
            .find_paste(id)
            .await?
            .filter(|paste| can_read(principal, paste))
            .map(|paste| redact_folder(principal, paste, false)))
    }

    pub async fn get_source(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<Option<Paste>, String> {
        let Some(paste) = self.find_paste(id).await? else {
            return Ok(None);
        };
        authorize_owner(principal, &paste, "paste:read")?;
        Ok(Some(redact_folder(principal, paste, false)))
    }

    async fn find_paste(&self, id: &str) -> Result<Option<Paste>, String> {
        let mut paste = sqlx::query_as::<_, Paste>(
            "SELECT id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,created_at,
                    updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE id=$1 AND consumed_at IS NULL AND (expires_at IS NULL OR expires_at>$2)",
        )
        .bind(id)
        .bind(unix_timestamp())
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        if let Some(value) = &mut paste {
            value.attachments = self.load_attachments(&value.id).await?;
            value.attachment_count = value.attachments.len() as i64;
            value.size_bytes = paste_size(value);
        }
        Ok(paste)
    }

    pub async fn read_paste(
        &self,
        principal: &Principal,
        id: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Option<PasteRead>, String> {
        let _write_guard = self.storage.lock_writes().await;
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|e| e.to_string())?;
        let now = unix_timestamp();
        let key_hash = idempotency_key.map(hash_token);
        if let Some(key_hash) = key_hash.as_deref() {
            let replay: Option<i64> = sqlx::query_scalar(
                "SELECT 1 FROM paste_read_receipts WHERE paste_id=$1 AND key_hash=$2 AND expires_at>$3",
            )
            .bind(id)
            .bind(key_hash)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
            if replay.is_some() {
                let Some(mut paste) = load_paste_for_read(&mut tx, principal, id).await? else {
                    return Ok(None);
                };
                let grant_token = create_read_grant(&mut tx, &paste, now).await?;
                paste = redact_folder(principal, paste, false);
                tx.commit().await.map_err(|error| error.to_string())?;
                return Ok(Some(PasteRead {
                    paste,
                    grant_token,
                    replayed: true,
                }));
            }
        }
        let lock = if self.storage.kind() == crate::repository::DatabaseKind::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let mut paste = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,created_at,
                    updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE id=$1 AND consumed_at IS NULL AND (expires_at IS NULL OR expires_at>$2){lock}"
        ))
        .bind(id)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let Some(mut paste) = paste.take().filter(|paste| can_read(principal, paste)) else {
            return Ok(None);
        };
        let next_reads = paste.read_count + 1;
        let consumed = paste
            .read_limit
            .is_some_and(|read_limit| next_reads >= read_limit);
        paste.attachments = load_attachments_from(&mut *tx, &paste.id).await?;
        paste.attachment_count = paste.attachments.len() as i64;
        paste.size_bytes = paste_size(&paste);
        if consumed {
            sqlx::query("UPDATE pastes SET read_count=$2,last_read_at=$3,consumed_at=$3,updated_at=$3,revision=revision+1 WHERE id=$1")
                .bind(&paste.id)
                .bind(next_reads)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE pastes SET read_count=$2,last_read_at=$3,updated_at=$3,revision=revision+1 WHERE id=$1")
                .bind(&paste.id)
                .bind(next_reads)
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(key_hash) = key_hash.as_deref() {
            sqlx::query(
                "INSERT INTO paste_read_receipts(paste_id,key_hash,expires_at) VALUES($1,$2,$3)",
            )
            .bind(&paste.id)
            .bind(key_hash)
            .bind(now + 900)
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
        }
        let grant_token = create_read_grant(&mut tx, &paste, now).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        paste.read_count = next_reads;
        paste.last_read_at = Some(now);
        paste.updated_at = paste.last_read_at.unwrap_or(paste.updated_at);
        paste.revision += 1;
        paste.consumed_at = consumed.then_some(paste.updated_at);
        Ok(Some(PasteRead {
            paste: redact_folder(principal, paste, false),
            grant_token,
            replayed: false,
        }))
    }

    pub async fn valid_read_grant(&self, paste_id: &str, token: &str) -> Result<bool, String> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM paste_read_grants WHERE paste_id=$1 AND token_hash=$2 AND expires_at>$3",
        )
        .bind(paste_id)
        .bind(hash_token(token))
        .bind(unix_timestamp())
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        Ok(found.is_some())
    }

    pub async fn get_paste_with_grant(
        &self,
        id: &str,
        token: &str,
    ) -> Result<Option<Paste>, String> {
        if !self.valid_read_grant(id, token).await? {
            return Ok(None);
        }
        let mut paste = sqlx::query_as::<_, Paste>(
            "SELECT id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,
                    created_at,updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE id=$1",
        )
        .bind(id)
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        if let Some(paste) = &mut paste {
            paste.attachments = self.load_attachments(id).await?;
            paste.attachment_count = paste.attachments.len() as i64;
            paste.size_bytes = paste_size(paste);
            paste.folder_id = None;
            paste.owner_id = None;
        }
        Ok(paste)
    }

    pub async fn ensure_can_update(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<Paste, String> {
        let paste = self.find_paste(id).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        Ok(paste)
    }

    pub async fn ensure_can_delete(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<Paste, String> {
        let paste = self.find_paste(id).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:delete")?;
        Ok(paste)
    }

    pub async fn create_paste(
        &self,
        principal: &Principal,
        input: &PasteInput,
    ) -> Result<Paste, String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        if !principal.can("paste:write") && !matches!(principal, Principal::Session(_)) {
            return Err("Missing paste:write scope".into());
        }
        let now = unix_timestamp();
        validate_input(input, now)?;
        let content_kind = input.content_kind.as_deref().unwrap_or("text");
        let (content, document_json) = normalized_content(
            content_kind,
            input.content.as_deref().unwrap_or(""),
            input.document.as_ref(),
        )?;
        let id = Uuid::new_v4().simple().to_string()[..24].to_string();
        let folder_id = input.folder_id.flatten();
        self.validate_folder_owner(owner, folder_id).await?;
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,
                               created_at,updated_at,revision,expires_at,last_read_at,read_count,read_limit)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10,1,$11,NULL,0,$12)",
        )
        .bind(&id)
        .bind(owner)
        .bind(folder_id)
        .bind(input.title.as_deref().unwrap_or("").trim())
        .bind(content)
        .bind(document_json)
        .bind(content_kind)
        .bind(if content_kind == "text" {
            input.language.as_deref().unwrap_or("plaintext")
        } else {
            "plaintext"
        })
        .bind(input.visibility.as_deref().unwrap_or("unlisted"))
        .bind(now)
        .bind(input.expires_at.flatten())
        .bind(input.read_limit.flatten())
        .execute(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        self.get_paste(principal, &id)
            .await?
            .ok_or("Paste creation failed".into())
    }

    pub async fn create_paste_idempotent(
        &self,
        principal: &Principal,
        input: &PasteInput,
        idempotency_key: Option<&str>,
        request_hash: &str,
    ) -> Result<(Paste, bool), String> {
        let Some(key) = idempotency_key else {
            return self
                .create_paste(principal, input)
                .await
                .map(|paste| (paste, false));
        };
        let owner = principal.user_id().ok_or("Authentication required")?;
        let _guard = self.storage.lock_writes().await;
        let key_hash = hash_token(key);
        let now = unix_timestamp();
        sqlx::query(
            "DELETE FROM idempotency_records
             WHERE user_id=$1 AND operation='create_paste' AND key_hash=$2 AND expires_at<=$3",
        )
        .bind(owner)
        .bind(&key_hash)
        .bind(now)
        .execute(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        if let Some(paste) = self
            .existing_idempotent_create(owner, &key_hash, request_hash)
            .await?
        {
            return Ok((paste, true));
        }

        if !principal.can("paste:write") && !matches!(principal, Principal::Session(_)) {
            return Err("Missing paste:write scope".into());
        }
        validate_input(input, now)?;
        let content_kind = input.content_kind.as_deref().unwrap_or("text");
        let (content, document_json) = normalized_content(
            content_kind,
            input.content.as_deref().unwrap_or(""),
            input.document.as_ref(),
        )?;
        let folder_id = input.folder_id.flatten();
        self.validate_folder_owner(owner, folder_id).await?;
        let id = Uuid::new_v4().simple().to_string()[..24].to_string();
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,
                                created_at,updated_at,revision,expires_at,last_read_at,read_count,read_limit)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$10,1,$11,NULL,0,$12)",
        )
        .bind(&id)
        .bind(owner)
        .bind(folder_id)
        .bind(input.title.as_deref().unwrap_or("").trim())
        .bind(content)
        .bind(document_json)
        .bind(content_kind)
        .bind(if content_kind == "text" {
            input.language.as_deref().unwrap_or("plaintext")
        } else {
            "plaintext"
        })
        .bind(input.visibility.as_deref().unwrap_or("unlisted"))
        .bind(now)
        .bind(input.expires_at.flatten())
        .bind(input.read_limit.flatten())
        .execute(&mut *tx)
        .await
        .map_err(|error| error.to_string())?;
        let idempotency_insert = sqlx::query(
            "INSERT INTO idempotency_records(user_id,operation,key_hash,request_hash,paste_id,created_at,expires_at)
             VALUES($1,'create_paste',$2,$3,$4,$5,$6)",
        )
        .bind(owner)
        .bind(&key_hash)
        .bind(request_hash)
        .bind(&id)
        .bind(now)
        .bind(now + 86400)
        .execute(&mut *tx)
        .await;
        if let Err(error) = idempotency_insert {
            tx.rollback()
                .await
                .map_err(|rollback| rollback.to_string())?;
            if let Some(paste) = self
                .existing_idempotent_create(owner, &key_hash, request_hash)
                .await?
            {
                return Ok((paste, true));
            }
            return Err(error.to_string());
        }
        tx.commit().await.map_err(|error| error.to_string())?;
        let paste = self.find_paste(&id).await?.ok_or("Paste creation failed")?;
        Ok((paste, false))
    }

    async fn existing_idempotent_create(
        &self,
        owner: i64,
        key_hash: &str,
        request_hash: &str,
    ) -> Result<Option<Paste>, String> {
        let existing = sqlx::query(
            "SELECT request_hash,paste_id FROM idempotency_records
             WHERE user_id=$1 AND operation='create_paste' AND key_hash=$2 AND expires_at>$3",
        )
        .bind(owner)
        .bind(key_hash)
        .bind(unix_timestamp())
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        let Some(row) = existing else {
            return Ok(None);
        };
        use sqlx::Row;
        let stored_hash: String = row
            .try_get("request_hash")
            .map_err(|error| error.to_string())?;
        if stored_hash != request_hash {
            return Err("Idempotency key was already used with a different request".into());
        }
        let paste_id: Option<String> =
            row.try_get("paste_id").map_err(|error| error.to_string())?;
        let paste_id = paste_id.ok_or("Idempotency resource no longer exists")?;
        self.find_paste(&paste_id)
            .await?
            .filter(|paste| paste.owner_id == Some(owner))
            .map(Some)
            .ok_or("Idempotency resource no longer exists".into())
    }

    pub async fn clear_create_idempotency(
        &self,
        principal: &Principal,
        key: &str,
    ) -> Result<(), String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        sqlx::query(
            "DELETE FROM idempotency_records
             WHERE user_id=$1 AND operation='create_paste' AND key_hash=$2",
        )
        .bind(owner)
        .bind(hash_token(key))
        .execute(self.storage.pool())
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    pub async fn update_paste(
        &self,
        principal: &Principal,
        id: &str,
        input: &PasteInput,
        expected_revision: Option<i64>,
    ) -> Result<Option<Paste>, String> {
        let now = unix_timestamp();
        validate_input(input, now)?;
        let current = match self.find_paste(id).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        authorize_owner(principal, &current, "paste:write")?;
        let folder_id = if let Some(folder) = input.folder_id {
            if current.owner_id != principal.user_id() {
                return Err("Folder organization is private to the paste owner".into());
            }
            self.validate_folder_owner(current.owner_id.unwrap_or_default(), folder)
                .await?;
            folder
        } else {
            current.folder_id
        };
        let content_kind = input
            .content_kind
            .as_deref()
            .unwrap_or(&current.content_kind);
        let requested_content = input.content.as_deref().unwrap_or(&current.content);
        let requested_document = if content_kind == "rich_text" {
            input.document.as_ref().or(current.document.as_ref())
        } else {
            input.document.as_ref()
        };
        let (content, document_json) =
            normalized_content(content_kind, requested_content, requested_document)?;
        let language = if content_kind == "text" {
            input.language.as_deref().unwrap_or(&current.language)
        } else {
            "plaintext"
        };
        let result = sqlx::query(
            "UPDATE pastes SET title=coalesce($2,title),content=$3,document_json=$4,
             content_kind=$5,language=$6,visibility=coalesce($7,visibility),
             expires_at=CASE WHEN $8=1 THEN $9 ELSE expires_at END,
             read_limit=CASE WHEN $10=1 THEN $11 ELSE read_limit END,
             folder_id=$12,updated_at=$13,revision=revision+1
             WHERE id=$1 AND ($14 IS NULL OR revision=$14)",
        )
        .bind(id)
        .bind(input.title.as_deref().map(str::trim))
        .bind(content)
        .bind(document_json)
        .bind(content_kind)
        .bind(language)
        .bind(input.visibility.as_deref())
        .bind(i64::from(input.expires_at.is_some()))
        .bind(input.expires_at.flatten())
        .bind(i64::from(input.read_limit.is_some()))
        .bind(input.read_limit.flatten())
        .bind(folder_id)
        .bind(now)
        .bind(expected_revision)
        .execute(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        if result.rows_affected() == 0 {
            return Err("Paste revision changed".into());
        }
        Ok(self
            .find_paste(id)
            .await?
            .map(|paste| redact_folder(principal, paste, false)))
    }

    pub async fn list_folders(&self, principal: &Principal) -> Result<FolderOverview, String> {
        let owner = folder_principal(principal, "paste:list")?;
        let items = sqlx::query_as(
            "SELECT f.id,f.owner_id,f.name,f.created_at,
                    (SELECT count(*) FROM pastes p WHERE p.folder_id=f.id) AS paste_count
             FROM folders f WHERE f.owner_id=$1 ORDER BY f.name_key,f.id",
        )
        .bind(owner)
        .fetch_all(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        let (total_count, unfiled_count): (i64, i64) = sqlx::query_as(
            "SELECT count(*),coalesce(sum(CASE WHEN folder_id IS NULL THEN 1 ELSE 0 END),0)
             FROM pastes WHERE owner_id=$1",
        )
        .bind(owner)
        .fetch_one(self.storage.pool())
        .await
        .map_err(|error| error.to_string())?;
        Ok(FolderOverview {
            items,
            total_count,
            unfiled_count,
        })
    }

    pub async fn create_folder(&self, principal: &Principal, name: &str) -> Result<Folder, String> {
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
        .map_err(|error| folder_database_error(error.to_string()))?;
        self.folder_by_id(owner, id)
            .await?
            .ok_or("Folder creation failed".into())
    }

    pub async fn rename_folder(
        &self,
        principal: &Principal,
        id: i64,
        name: &str,
    ) -> Result<Option<Folder>, String> {
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
                .map_err(|error| folder_database_error(error.to_string()))?
                .rows_affected();
        if changed == 0 {
            Ok(None)
        } else {
            self.folder_by_id(owner, id).await
        }
    }

    pub async fn delete_folder(&self, principal: &Principal, id: i64) -> Result<bool, String> {
        let owner = folder_principal(principal, "paste:write")?;
        let mut transaction = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query(
            "UPDATE pastes SET folder_id=NULL,updated_at=$3,revision=revision+1
             WHERE folder_id=$1 AND owner_id=$2",
        )
        .bind(id)
        .bind(owner)
        .bind(unix_timestamp())
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
        let deleted = sqlx::query("DELETE FROM folders WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(owner)
            .execute(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?
            .rows_affected()
            == 1;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        Ok(deleted)
    }

    pub async fn move_pastes(
        &self,
        principal: &Principal,
        paste_ids: &[String],
        folder_id: Option<i64>,
    ) -> Result<(), String> {
        let owner = folder_principal(principal, "paste:write")?;
        if paste_ids.is_empty() || paste_ids.len() > 100 {
            return Err("Select between 1 and 100 pastes".into());
        }
        let unique = paste_ids.iter().collect::<HashSet<_>>();
        if unique.len() != paste_ids.len() {
            return Err("Paste IDs must be unique".into());
        }
        self.validate_folder_owner(owner, folder_id).await?;
        let _guard = self.storage.lock_writes().await;
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|e| e.to_string())?;
        for id in paste_ids {
            let changed = sqlx::query(
                "UPDATE pastes SET folder_id=$3,updated_at=$4,revision=revision+1
                 WHERE id=$1 AND owner_id=$2",
            )
            .bind(id)
            .bind(owner)
            .bind(folder_id)
            .bind(unix_timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .rows_affected();
            if changed != 1 {
                return Err("One or more pastes were not found".into());
            }
        }
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn validate_folder_owner(
        &self,
        owner: i64,
        folder_id: Option<i64>,
    ) -> Result<(), String> {
        let Some(id) = folder_id else {
            return Ok(());
        };
        if self.folder_by_id(owner, id).await?.is_none() {
            return Err("Folder not found".into());
        }
        Ok(())
    }

    async fn folder_by_id(&self, owner: i64, id: i64) -> Result<Option<Folder>, String> {
        sqlx::query_as(
            "SELECT f.id,f.owner_id,f.name,f.created_at,
                    (SELECT count(*) FROM pastes p WHERE p.folder_id=f.id) AS paste_count
             FROM folders f WHERE f.id=$1 AND f.owner_id=$2",
        )
        .bind(id)
        .bind(owner)
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|error| error.to_string())
    }

    pub async fn delete_paste(
        &self,
        principal: &Principal,
        id: &str,
        expected_revision: Option<i64>,
    ) -> Result<bool, String> {
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
            std::fs::rename(&directory, &staged).map_err(|e| e.to_string())?;
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
                    Err("Paste revision changed".into())
                } else {
                    Ok(false)
                }
            }
            Err(error) => {
                if had_directory {
                    let _ = std::fs::rename(staged, directory);
                }
                Err(error.to_string())
            }
        }
    }

    pub async fn add_attachments(
        &self,
        principal: &Principal,
        id: &str,
        inputs: &[(String, String, i64)],
        expected_revision: Option<i64>,
    ) -> Result<Vec<Attachment>, String> {
        let _write_guard = self.storage.lock_writes().await;
        let paste = self.find_paste(id).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let mut names = paste
            .attachments
            .iter()
            .map(|attachment| attachment.filename.as_str())
            .collect::<HashSet<_>>();
        for (name, _, _) in inputs {
            if !names.insert(name) {
                return Err(format!("{name} already exists"));
            }
        }
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|e| e.to_string())?;
        let starting_sort_order: i64 = sqlx::query_scalar(
            "SELECT coalesce(max(sort_order)+1,0) FROM attachments WHERE paste_id=$1",
        )
        .bind(&paste.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
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
            .map_err(|e| e.to_string())?;
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
        .map_err(|error| error.to_string())?
        .rows_affected();
        if changed == 0 {
            return Err("Paste revision changed".into());
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(attachments)
    }

    pub async fn delete_attachment(
        &self,
        principal: &Principal,
        id: &str,
        attachment_id: i64,
        expected_revision: Option<i64>,
    ) -> Result<bool, String> {
        let paste = self.find_paste(id).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let attachment = paste
            .attachments
            .into_iter()
            .find(|attachment| attachment.id == attachment_id);
        let Some(attachment) = attachment else {
            return Ok(false);
        };
        if attachment.storage_key.starts_with('.')
            || std::path::Path::new(&attachment.storage_key)
                .components()
                .count()
                != 1
        {
            return Err("Unsafe attachment metadata".to_string());
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
            std::fs::rename(&path, &staged).map_err(|e| e.to_string())?;
        }
        let mut transaction = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|error| error.to_string())?;
        let result = async {
            let affected = sqlx::query("DELETE FROM attachments WHERE id=$1")
                .bind(attachment_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?
                .rows_affected();
            if affected == 1 {
                let changed = sqlx::query(
                    "UPDATE pastes SET updated_at=$2,revision=revision+1
                     WHERE id=$1 AND ($3 IS NULL OR revision=$3)",
                )
                .bind(&paste.id)
                .bind(unix_timestamp())
                .bind(expected_revision)
                .execute(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?
                .rows_affected();
                if changed == 0 {
                    return Err("Paste revision changed".into());
                }
            }
            transaction
                .commit()
                .await
                .map_err(|error| error.to_string())?;
            Ok::<u64, String>(affected)
        }
        .await;
        match result {
            Ok(1) => {
                if existed {
                    let _ = std::fs::remove_file(staged);
                }
                Ok(true)
            }
            Ok(_) => {
                if existed {
                    let _ = std::fs::rename(staged, path);
                }
                Ok(false)
            }
            Err(error) => {
                if existed {
                    let _ = std::fs::rename(staged, path);
                }
                Err(error)
            }
        }
    }

    async fn load_attachments(&self, paste_id: &str) -> Result<Vec<Attachment>, String> {
        load_attachments_from(self.storage.pool(), paste_id).await
    }
}

fn folder_principal(principal: &Principal, scope: &str) -> Result<i64, String> {
    let owner = principal
        .user_id()
        .ok_or("Folders require a user-owned credential")?;
    if matches!(principal, Principal::ApiKey(_)) && !principal.can(scope) {
        return Err(format!("Missing {scope} scope"));
    }
    Ok(owner)
}

fn validate_folder_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err("Folder name must contain 1 to 100 printable characters".into());
    }
    if matches!(
        name.to_ascii_lowercase().as_str(),
        "all pastes" | "uncategorized"
    ) {
        return Err("Folder name is reserved".into());
    }
    Ok(name)
}

fn folder_database_error(error: String) -> String {
    if error.to_ascii_lowercase().contains("unique") {
        "A folder with that name already exists".into()
    } else {
        error
    }
}

fn folder_name_key(name: &str) -> String {
    name.to_lowercase()
}

fn redact_folder(principal: &Principal, mut paste: Paste, administrative: bool) -> Paste {
    if administrative || principal.user_id() != paste.owner_id {
        paste.folder_id = None;
    }
    paste
}

fn normalized_content(
    content_kind: &str,
    content: &str,
    document: Option<&serde_json::Value>,
) -> Result<(String, Option<String>), String> {
    if content_kind == "rich_text" {
        let document = document.ok_or("Rich-text pastes require a document")?;
        let content = validate_document(document)
            .map_err(|error| format!("Rich-text document is invalid: {error}"))?;
        let document_json = serde_json::to_string(document).map_err(|error| error.to_string())?;
        Ok((content, Some(document_json)))
    } else {
        if document.is_some() {
            return Err("Only rich-text pastes accept a document".into());
        }
        Ok((content.to_string(), None))
    }
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

async fn load_paste_for_read(
    transaction: &mut sqlx::Transaction<'_, Any>,
    principal: &Principal,
    id: &str,
) -> Result<Option<Paste>, String> {
    let mut paste = sqlx::query_as::<_, Paste>(
        "SELECT id,owner_id,folder_id,title,content,document_json,content_kind,language,visibility,
                created_at,updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit
         FROM pastes WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(|error| error.to_string())?;
    let Some(mut paste) = paste.take().filter(|paste| can_read(principal, paste)) else {
        return Ok(None);
    };
    paste.attachments = load_attachments_from(&mut **transaction, id).await?;
    paste.attachment_count = paste.attachments.len() as i64;
    paste.size_bytes = paste_size(&paste);
    Ok(Some(paste))
}

async fn create_read_grant(
    transaction: &mut sqlx::Transaction<'_, Any>,
    paste: &Paste,
    now: i64,
) -> Result<Option<String>, String> {
    if paste.read_limit.is_none() || paste.attachments.is_empty() {
        return Ok(None);
    }
    let token = format!("rbg_{}", Uuid::new_v4().simple());
    sqlx::query("INSERT INTO paste_read_grants(token_hash,paste_id,expires_at) VALUES($1,$2,$3)")
        .bind(hash_token(&token))
        .bind(&paste.id)
        .bind(now + 900)
        .execute(&mut **transaction)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Some(token))
}

async fn load_attachments_from<'e, E>(
    executor: E,
    paste_id: &str,
) -> Result<Vec<Attachment>, String>
where
    E: Executor<'e, Database = Any>,
{
    sqlx::query_as::<_, Attachment>(
        "SELECT id,sort_order,filename,storage_key,size_bytes FROM attachments
         WHERE paste_id=$1 ORDER BY sort_order",
    )
    .bind(paste_id)
    .fetch_all(executor)
    .await
    .map_err(|e| e.to_string())
}

fn paste_size(paste: &Paste) -> i64 {
    let document_size = paste
        .document
        .as_ref()
        .and_then(|document| serde_json::to_vec(document).ok())
        .map_or(0, |document| document.len());
    let attachment_size: i64 = paste
        .attachments
        .iter()
        .map(|attachment| attachment.size_bytes.max(0))
        .sum();
    paste.content.len() as i64 + document_size as i64 + attachment_size
}

#[cfg(test)]
mod tests {
    use super::{Attachment, Paste, PasteService, Principal};
    use crate::account::api_keys::ApiKey;
    use crate::account::{SessionUser, User};
    use crate::repository::Repository;
    use crate::services::validation::can_read;

    fn owner_paste() -> Paste {
        Paste {
            id: "example".to_string(),
            owner_id: Some(7),
            folder_id: None,
            title: String::new(),
            content: "secret".to_string(),
            document: None,
            content_kind: "text".to_string(),
            language: "plaintext".to_string(),
            visibility: "private".to_string(),
            created_at: 0,
            updated_at: 0,
            revision: 1,
            consumed_at: None,
            expires_at: None,
            last_read_at: None,
            read_count: 0,
            read_limit: None,
            attachment_count: 0,
            size_bytes: 0,
            attachments: Vec::<Attachment>::new(),
        }
    }

    fn key(scopes: &str) -> Principal {
        Principal::ApiKey(ApiKey {
            id: 1,
            user_id: Some(7),
            name: "test".to_string(),
            token_prefix: "prefix".to_string(),
            scopes: scopes.split(',').map(str::to_string).collect(),
            created_at: 0,
            last_used_at: None,
            enabled: true,
        })
    }

    #[test]
    fn private_paste_reads_require_read_scope_for_api_keys() {
        let paste = owner_paste();
        assert!(!can_read(&key("paste:write"), &paste));
        assert!(!can_read(&key("paste:delete"), &paste));
        assert!(can_read(&key("paste:read"), &paste));
        assert!(can_read(&key("paste:manage"), &paste));
    }

    #[actix_web::test]
    async fn read_limit_is_committed_with_the_consuming_read() {
        let path = std::env::temp_dir().join(format!("racebin-limited-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &path).await.unwrap();
        repository.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO users(id,username,password_hash,role,created_at)
             VALUES(7,'owner','unused','user',0)",
        )
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,title,content,content_kind,language,visibility,
             created_at,read_count,read_limit)
             VALUES('limited',7,'','secret','text','plaintext','private',0,0,1)",
        )
        .execute(repository.pool())
        .await
        .unwrap();
        let principal = Principal::Session(SessionUser {
            user: User {
                id: 7,
                username: "private".to_string(),
                role: "user".to_string(),
                enabled: true,
                password_change_required: false,
            },
            csrf_token: "csrf".to_string(),
        });
        let services = PasteService::new(repository.clone());
        let consumed = services
            .read_paste(&principal, "limited", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(consumed.paste.content, "secret");
        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM pastes WHERE id=$1 AND consumed_at IS NULL")
                .bind("limited")
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert_eq!(remaining, 0);
        let tombstone: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pastes WHERE id=$1 AND consumed_at IS NOT NULL",
        )
        .bind("limited")
        .fetch_one(repository.pool())
        .await
        .unwrap();
        assert_eq!(tombstone, 1);
        let _ = std::fs::remove_dir_all(path);
    }
}
