use crate::account::{self as accounts, api_keys};
use crate::repository::Repository;
use actix_web::HttpRequest;
use sqlx::{Any, Executor};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

mod model;
mod validation;

pub use model::{Page, Paste, PasteFile, PasteInput, PasteQuery};
use validation::{authorize_owner, can_read, validate_input, validate_url};

#[derive(Clone)]
pub struct Services {
    pub repo: Repository,
}

#[derive(Clone, Debug)]
pub enum Principal {
    Anonymous,
    User(accounts::SessionUser),
    Key(api_keys::ApiKey),
}

impl Principal {
    pub fn user_id(&self) -> Option<i64> {
        match self {
            Self::User(session) => Some(session.user.id),
            Self::Key(key) => key.user_id,
            Self::Anonymous => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Self::User(session) if session.user.is_admin())
    }

    pub fn can(&self, scope: &str) -> bool {
        self.is_admin() || matches!(self, Self::Key(key) if key.has_scope(scope))
    }
}

impl Services {
    pub fn new(repo: Repository) -> Self {
        Self { repo }
    }

    pub async fn principal(&self, req: &HttpRequest) -> Result<Principal, String> {
        if let Some(header) = req.headers().get("Authorization") {
            let header = header
                .to_str()
                .map_err(|_| "Invalid authorization header")?;
            let value = header
                .strip_prefix("Bearer ")
                .ok_or("Invalid authorization scheme")?;
            return api_keys::authenticate(&self.repo, value)
                .await?
                .map(Principal::Key)
                .ok_or_else(|| "Invalid bearer token".to_string());
        }
        match accounts::current(&self.repo, req).await {
            Some(session)
                if session.user.force_password_change
                    && !matches!(
                        (req.method().as_str(), req.path()),
                        ("GET", "/api/v2/session")
                            | ("DELETE", "/api/v2/session")
                            | ("PATCH", "/api/v2/account/password")
                    ) =>
            {
                Err("Password change required".to_string())
            }
            Some(session) => Ok(Principal::User(session)),
            None => Ok(Principal::Anonymous),
        }
    }

    pub fn csrf_valid(&self, req: &HttpRequest, principal: &Principal) -> bool {
        match principal {
            Principal::Key(_) => true,
            Principal::User(session) => req
                .headers()
                .get("X-CSRF-Token")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == session.csrf_token),
            Principal::Anonymous => false,
        }
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
        let access = query.access.as_deref();
        let owner = if query.mine.unwrap_or(false) {
            user_id
        } else {
            query.owner_user_id
        };
        let filter = "(($1=1) OR (($2 IS NULL AND access='public') OR
              ($2 IS NOT NULL AND (access='public' OR owner_user_id=$2))))
             AND ($3 IS NULL OR access=$3)
             AND ($4 IS NULL OR owner_user_id=$4)
             AND (expiration IS NULL OR expiration>$5)
             AND (lower(title) LIKE lower($6) OR lower(content) LIKE lower($6)
                  OR lower(slug) LIKE lower($6))";
        let total: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM pasta WHERE {filter}"))
            .bind(i64::from(admin))
            .bind(user_id)
            .bind(access)
            .bind(owner)
            .bind(now())
            .bind(&search)
            .fetch_one(self.repo.pool())
            .await
            .map_err(|e| e.to_string())?;
        let items = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,slug,owner_user_id,title,substr(content,1,500) AS content,
                    kind,syntax,access,created,expiration,last_read,read_count,burn_after_reads
             FROM pasta WHERE {filter} ORDER BY created DESC LIMIT $7 OFFSET $8"
        ))
        .bind(i64::from(admin))
        .bind(user_id)
        .bind(access)
        .bind(owner)
        .bind(now())
        .bind(search)
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(self.repo.pool())
        .await
        .map_err(|e| e.to_string())?;
        Ok(Page {
            items,
            page,
            page_size,
            total,
        })
    }

    pub async fn get_paste(
        &self,
        principal: &Principal,
        slug: &str,
    ) -> Result<Option<Paste>, String> {
        Ok(self
            .find_paste(slug)
            .await?
            .filter(|paste| can_read(principal, paste)))
    }

    async fn find_paste(&self, slug: &str) -> Result<Option<Paste>, String> {
        let mut paste = sqlx::query_as::<_, Paste>(
            "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,
                    expiration,last_read,read_count,burn_after_reads
             FROM pasta WHERE slug=$1 AND (expiration IS NULL OR expiration>$2)",
        )
        .bind(slug)
        .bind(now())
        .fetch_optional(self.repo.pool())
        .await
        .map_err(|e| e.to_string())?;
        if let Some(value) = &mut paste {
            value.files = self.files(value.id).await?;
        }
        Ok(paste)
    }

    pub async fn read_paste(
        &self,
        principal: &Principal,
        slug: &str,
    ) -> Result<Option<Paste>, String> {
        let _write_guard = self.repo.lock_writes().await;
        let mut tx = self.repo.pool().begin().await.map_err(|e| e.to_string())?;
        let lock = if self.repo.kind() == crate::repository::DatabaseKind::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let mut paste = sqlx::query_as::<_, Paste>(&format!(
            "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,
                    expiration,last_read,read_count,burn_after_reads
             FROM pasta WHERE slug=$1 AND (expiration IS NULL OR expiration>$2){lock}"
        ))
        .bind(slug)
        .bind(now())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let Some(mut paste) = paste.take().filter(|paste| can_read(principal, paste)) else {
            return Ok(None);
        };
        let next_reads = paste.read_count + 1;
        let burned = paste.burn_after_reads > 0 && next_reads >= paste.burn_after_reads;
        paste.files = files_from(&mut *tx, paste.id).await?;
        if burned {
            sqlx::query("DELETE FROM pasta WHERE id=$1")
                .bind(paste.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE pasta SET read_count=$2,last_read=$3 WHERE id=$1")
                .bind(paste.id)
                .bind(next_reads)
                .bind(now())
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        if burned {
            let _ =
                std::fs::remove_dir_all(self.repo.data_dir.join("attachments").join(&paste.slug));
        }
        paste.read_count = next_reads;
        Ok(Some(paste))
    }

    pub async fn ensure_can_update(
        &self,
        principal: &Principal,
        slug: &str,
    ) -> Result<Paste, String> {
        let paste = self.find_paste(slug).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        Ok(paste)
    }

    pub async fn create_paste(
        &self,
        principal: &Principal,
        input: &PasteInput,
    ) -> Result<Paste, String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        if !principal.can("paste:write") && !matches!(principal, Principal::User(_)) {
            return Err("Missing paste:write scope".into());
        }
        validate_input(input, true)?;
        let now = now();
        let slug = Uuid::new_v4().simple().to_string()[..24].to_string();
        sqlx::query(
            "INSERT INTO pasta(slug,owner_user_id,title,content,kind,syntax,access,created,
                               expiration,last_read,read_count,burn_after_reads)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$8,0,$10)",
        )
        .bind(&slug)
        .bind(owner)
        .bind(input.title.as_deref().unwrap_or("").trim())
        .bind(input.content.as_deref().unwrap_or(""))
        .bind(input.kind.as_deref().unwrap_or("text"))
        .bind(input.syntax.as_deref().unwrap_or("none"))
        .bind(input.access.as_deref().unwrap_or("unlisted"))
        .bind(now)
        .bind(input.expiration.flatten())
        .bind(input.burn_after_reads.unwrap_or(0))
        .execute(self.repo.pool())
        .await
        .map_err(|e| e.to_string())?;
        self.get_paste(principal, &slug)
            .await?
            .ok_or("Paste creation failed".into())
    }

    pub async fn update_paste(
        &self,
        principal: &Principal,
        slug: &str,
        input: &PasteInput,
    ) -> Result<Option<Paste>, String> {
        validate_input(input, false)?;
        let current = match self.find_paste(slug).await? {
            Some(value) => value,
            None => return Ok(None),
        };
        authorize_owner(principal, &current, "paste:write")?;
        validate_url(
            input.kind.as_deref().unwrap_or(&current.kind),
            input.content.as_deref().unwrap_or(&current.content),
        )?;
        sqlx::query(
            "UPDATE pasta SET title=coalesce($2,title),content=coalesce($3,content),
             kind=coalesce($4,kind),syntax=coalesce($5,syntax),access=coalesce($6,access),
             expiration=CASE WHEN $7=1 THEN $8 ELSE expiration END,
             burn_after_reads=coalesce($9,burn_after_reads) WHERE slug=$1",
        )
        .bind(slug)
        .bind(input.title.as_deref().map(str::trim))
        .bind(input.content.as_deref())
        .bind(input.kind.as_deref())
        .bind(input.syntax.as_deref())
        .bind(input.access.as_deref())
        .bind(i64::from(input.expiration.is_some()))
        .bind(input.expiration.flatten())
        .bind(input.burn_after_reads)
        .execute(self.repo.pool())
        .await
        .map_err(|e| e.to_string())?;
        self.find_paste(slug).await
    }

    pub async fn delete_paste(&self, principal: &Principal, slug: &str) -> Result<bool, String> {
        let current = match self.find_paste(slug).await? {
            Some(value) => value,
            None => return Ok(false),
        };
        authorize_owner(principal, &current, "paste:delete")?;
        let directory = self.repo.data_dir.join("attachments").join(slug);
        let staged = self
            .repo
            .data_dir
            .join("attachments")
            .join(format!(".delete-{}", Uuid::new_v4()));
        let had_directory = directory.exists();
        if had_directory {
            std::fs::rename(&directory, &staged).map_err(|e| e.to_string())?;
        }
        match sqlx::query("DELETE FROM pasta WHERE slug=$1")
            .bind(slug)
            .execute(self.repo.pool())
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

    pub async fn add_files(
        &self,
        principal: &Principal,
        slug: &str,
        inputs: &[(String, String, i64)],
    ) -> Result<Vec<PasteFile>, String> {
        let _write_guard = self.repo.lock_writes().await;
        let paste = self.find_paste(slug).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let mut names = paste
            .files
            .iter()
            .map(|file| file.name.as_str())
            .collect::<HashSet<_>>();
        for (name, _, _) in inputs {
            if !names.insert(name) {
                return Err(format!("{name} already exists"));
            }
        }
        let mut tx = self.repo.pool().begin().await.map_err(|e| e.to_string())?;
        let starting_position: i64 = sqlx::query_scalar(
            "SELECT coalesce(max(position)+1,0) FROM pasta_file WHERE pasta_id=$1",
        )
        .bind(paste.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let mut has_primary = paste.files.iter().any(|file| file.role == "primary");
        let mut files = Vec::with_capacity(inputs.len());
        for (offset, (name, storage_name, size)) in inputs.iter().enumerate() {
            let position = starting_position + offset as i64;
            let role = if has_primary {
                "attachment"
            } else {
                has_primary = true;
                "primary"
            };
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO pasta_file(pasta_id,position,role,name,storage_name,size)
                 VALUES($1,$2,$3,$4,$5,$6) RETURNING id",
            )
            .bind(paste.id)
            .bind(position)
            .bind(role)
            .bind(name)
            .bind(storage_name)
            .bind(size)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            files.push(PasteFile {
                id,
                position,
                role: role.to_string(),
                name: name.clone(),
                storage_name: storage_name.clone(),
                size: *size,
            });
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(files)
    }

    pub async fn delete_file(
        &self,
        principal: &Principal,
        slug: &str,
        file_id: i64,
    ) -> Result<bool, String> {
        let paste = self.find_paste(slug).await?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let file = paste.files.into_iter().find(|file| file.id == file_id);
        let Some(file) = file else {
            return Ok(false);
        };
        if file.storage_name.starts_with('.')
            || std::path::Path::new(&file.storage_name)
                .components()
                .count()
                != 1
        {
            return Err("Unsafe attachment metadata".to_string());
        }
        let path = self
            .repo
            .data_dir
            .join("attachments")
            .join(&paste.slug)
            .join(&file.storage_name);
        let staged = path.with_file_name(format!(".delete-{}", Uuid::new_v4()));
        let existed = path.exists();
        if existed {
            std::fs::rename(&path, &staged).map_err(|e| e.to_string())?;
        }
        let result = sqlx::query("DELETE FROM pasta_file WHERE id=$1")
            .bind(file_id)
            .execute(self.repo.pool())
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

    async fn files(&self, paste_id: i64) -> Result<Vec<PasteFile>, String> {
        files_from(self.repo.pool(), paste_id).await
    }
}

async fn files_from<'e, E>(executor: E, paste_id: i64) -> Result<Vec<PasteFile>, String>
where
    E: Executor<'e, Database = Any>,
{
    sqlx::query_as::<_, PasteFile>(
        "SELECT id,position,role,name,storage_name,size FROM pasta_file
         WHERE pasta_id=$1 ORDER BY position",
    )
    .bind(paste_id)
    .fetch_all(executor)
    .await
    .map_err(|e| e.to_string())
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{Paste, PasteFile, Principal, Services};
    use crate::account::api_keys::ApiKey;
    use crate::account::{SessionUser, User};
    use crate::repository::Repository;
    use crate::services::validation::{can_read, validate_url};

    #[test]
    fn url_pastes_accept_only_http_destinations() {
        assert!(validate_url("url", "https://example.com/path").is_ok());
        assert!(validate_url("url", "javascript:alert(1)").is_err());
        assert!(validate_url("url", "not a url").is_err());
        assert!(validate_url("text", "not a url").is_ok());
    }

    fn owner_paste() -> Paste {
        Paste {
            id: 1,
            slug: "example".to_string(),
            owner_user_id: Some(7),
            title: String::new(),
            content: "secret".to_string(),
            kind: "text".to_string(),
            syntax: "none".to_string(),
            access: "owner".to_string(),
            created: 0,
            expiration: None,
            last_read: None,
            read_count: 0,
            burn_after_reads: 0,
            files: Vec::<PasteFile>::new(),
        }
    }

    fn key(scopes: &str) -> Principal {
        Principal::Key(ApiKey {
            id: 1,
            user_id: Some(7),
            name: "test".to_string(),
            prefix: "prefix".to_string(),
            scopes: scopes.to_string(),
            created: 0,
            last_used: None,
            enabled: true,
        })
    }

    #[test]
    fn owner_only_reads_require_read_scope_for_api_keys() {
        let paste = owner_paste();
        assert!(!can_read(&key("paste:write"), &paste));
        assert!(!can_read(&key("paste:delete"), &paste));
        assert!(can_read(&key("paste:read"), &paste));
        assert!(can_read(&key("paste:admin"), &paste));
    }

    #[actix_web::test]
    async fn burn_after_read_is_committed_with_the_consuming_read() {
        let path = std::env::temp_dir().join(format!("racebin-burn-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            path.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &path).await.unwrap();
        repository.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,created)
             VALUES(7,'owner','unused','user',0)",
        )
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,
             created,read_count,burn_after_reads)
             VALUES(1,'burn',7,'','secret','text','none','owner',0,0,1)",
        )
        .execute(repository.pool())
        .await
        .unwrap();
        let principal = Principal::User(SessionUser {
            user: User {
                id: 7,
                username: "owner".to_string(),
                role: "user".to_string(),
                enabled: true,
                force_password_change: false,
            },
            csrf_token: "csrf".to_string(),
        });
        let services = Services::new(repository.clone());
        let consumed = services
            .read_paste(&principal, "burn")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(consumed.content, "secret");
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM pasta WHERE slug=$1")
            .bind("burn")
            .fetch_one(repository.pool())
            .await
            .unwrap();
        assert_eq!(remaining, 0);
        let _ = std::fs::remove_dir_all(path);
    }
}
