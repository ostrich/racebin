use crate::account::{self as accounts, api_keys};
use crate::repository::Repository;
use sqlx::{Any, Executor};
use std::collections::HashSet;
use uuid::Uuid;

use super::model::{Attachment, Page, Paste, PasteInput, PasteQuery};
use super::rich_text::validate_document;
use super::validation::{authorize_owner, can_read, validate_input, validate_url};
use crate::time::unix_timestamp;

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
        let owner = if query.mine.unwrap_or(false) {
            user_id
        } else {
            query.owner_id
        };
        let filter = "(($1=1) OR (($2 IS NULL AND visibility='public') OR
              ($2 IS NOT NULL AND (visibility='public' OR owner_id=$2))))
             AND ($3 IS NULL OR visibility=$3)
             AND ($4 IS NULL OR owner_id=$4)
             AND (expires_at IS NULL OR expires_at>$5)
             AND (lower(title) LIKE lower($6) OR lower(content) LIKE lower($6)
                  OR lower(id) LIKE lower($6))";
        let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM pastes WHERE {filter}"))
            .bind(i64::from(admin))
            .bind(user_id)
            .bind(visibility)
            .bind(owner)
            .bind(unix_timestamp())
            .bind(&search)
            .fetch_one(self.storage.pool())
            .await
            .map_err(|e| e.to_string())?;
        let items = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,owner_id,title,substr(content,1,500) AS content,NULL AS document_json,
                    content_kind,language,visibility,created_at,expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE {filter} ORDER BY created_at DESC LIMIT $7 OFFSET $8"
        ))
        .bind(i64::from(admin))
        .bind(user_id)
        .bind(visibility)
        .bind(owner)
        .bind(unix_timestamp())
        .bind(search)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        Ok(Page {
            items,
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
            .filter(|paste| can_read(principal, paste)))
    }

    async fn find_paste(&self, id: &str) -> Result<Option<Paste>, String> {
        let mut paste = sqlx::query_as::<_, Paste>(
            "SELECT id,owner_id,title,content,document_json,content_kind,language,visibility,created_at,
                    expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE id=$1 AND (expires_at IS NULL OR expires_at>$2)",
        )
        .bind(id)
        .bind(unix_timestamp())
        .fetch_optional(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        if let Some(value) = &mut paste {
            value.attachments = self.load_attachments(&value.id).await?;
        }
        Ok(paste)
    }

    pub async fn consume_paste(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<Option<Paste>, String> {
        let _write_guard = self.storage.lock_writes().await;
        let mut tx = self
            .storage
            .pool()
            .begin()
            .await
            .map_err(|e| e.to_string())?;
        let lock = if self.storage.kind() == crate::repository::DatabaseKind::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let mut paste = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,owner_id,title,content,document_json,content_kind,language,visibility,created_at,
                    expires_at,last_read_at,read_count,read_limit
             FROM pastes WHERE id=$1 AND (expires_at IS NULL OR expires_at>$2){lock}"
        ))
        .bind(id)
        .bind(unix_timestamp())
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
        if consumed {
            sqlx::query("DELETE FROM pastes WHERE id=$1")
                .bind(&paste.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE pastes SET read_count=$2,last_read_at=$3 WHERE id=$1")
                .bind(&paste.id)
                .bind(next_reads)
                .bind(unix_timestamp())
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        if consumed {
            let _ =
                std::fs::remove_dir_all(self.storage.data_dir.join("attachments").join(&paste.id));
        }
        paste.read_count = next_reads;
        Ok(Some(paste))
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

    pub async fn create_paste(
        &self,
        principal: &Principal,
        input: &PasteInput,
    ) -> Result<Paste, String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        if !principal.can("paste:write") && !matches!(principal, Principal::Session(_)) {
            return Err("Missing paste:write scope".into());
        }
        validate_input(input, true)?;
        let content_kind = input.content_kind.as_deref().unwrap_or("text");
        let (content, document_json) = normalized_content(
            content_kind,
            input.content.as_deref().unwrap_or(""),
            input.document.as_ref(),
        )?;
        let now = unix_timestamp();
        let id = Uuid::new_v4().simple().to_string()[..24].to_string();
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,title,content,document_json,content_kind,language,visibility,
                               created_at,expires_at,last_read_at,read_count,read_limit)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$9,0,$11)",
        )
        .bind(&id)
        .bind(owner)
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

    pub async fn update_paste(
        &self,
        principal: &Principal,
        id: &str,
        input: &PasteInput,
    ) -> Result<Option<Paste>, String> {
        validate_input(input, false)?;
        let current = match self.find_paste(id).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        authorize_owner(principal, &current, "paste:write")?;
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
        validate_url(content_kind, &content)?;
        sqlx::query(
            "UPDATE pastes SET title=coalesce($2,title),content=$3,document_json=$4,
             content_kind=$5,language=$6,visibility=coalesce($7,visibility),
             expires_at=CASE WHEN $8=1 THEN $9 ELSE expires_at END,
             read_limit=CASE WHEN $10=1 THEN $11 ELSE read_limit END WHERE id=$1",
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
        .execute(self.storage.pool())
        .await
        .map_err(|e| e.to_string())?;
        self.find_paste(id).await
    }

    pub async fn delete_paste(&self, principal: &Principal, id: &str) -> Result<bool, String> {
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
        match sqlx::query("DELETE FROM pastes WHERE id=$1")
            .bind(id)
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
                Ok(false)
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
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(attachments)
    }

    pub async fn delete_attachment(
        &self,
        principal: &Principal,
        id: &str,
        attachment_id: i64,
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
        let result = sqlx::query("DELETE FROM attachments WHERE id=$1")
            .bind(attachment_id)
            .execute(self.storage.pool())
            .await
            .map(|result| result.rows_affected())
            .map_err(|e| e.to_string());
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

#[cfg(test)]
mod tests {
    use super::{Attachment, Paste, PasteService, Principal};
    use crate::account::api_keys::ApiKey;
    use crate::account::{SessionUser, User};
    use crate::repository::Repository;
    use crate::services::validation::{can_read, validate_url};

    #[test]
    fn redirect_pastes_accept_only_http_destinations() {
        assert!(validate_url("redirect", "https://example.com/path").is_ok());
        assert!(validate_url("redirect", "javascript:alert(1)").is_err());
        assert!(validate_url("redirect", "not a url").is_err());
        assert!(validate_url("text", "not a url").is_ok());
    }

    fn owner_paste() -> Paste {
        Paste {
            id: "example".to_string(),
            owner_id: Some(7),
            title: String::new(),
            content: "secret".to_string(),
            document: None,
            content_kind: "text".to_string(),
            language: "plaintext".to_string(),
            visibility: "private".to_string(),
            created_at: 0,
            expires_at: None,
            last_read_at: None,
            read_count: 0,
            read_limit: None,
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
            .consume_paste(&principal, "limited")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(consumed.content, "secret");
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM pastes WHERE id=$1")
            .bind("limited")
            .fetch_one(repository.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
        let _ = std::fs::remove_dir_all(path);
    }
}
