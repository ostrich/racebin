use crate::repository::Repository;
use crate::util::{accounts, api_keys};
use actix_web::HttpRequest;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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

#[derive(Clone, Debug, Serialize)]
pub struct Paste {
    pub id: i64,
    pub slug: String,
    pub owner_user_id: Option<i64>,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub syntax: String,
    pub access: String,
    pub created: i64,
    pub expiration: Option<i64>,
    pub last_read: Option<i64>,
    pub read_count: i64,
    pub burn_after_reads: i64,
    pub files: Vec<PasteFile>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PasteFile {
    pub id: i64,
    pub position: i64,
    pub role: String,
    pub name: String,
    #[serde(skip)]
    pub storage_name: String,
    pub size: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub kind: Option<String>,
    pub syntax: Option<String>,
    pub access: Option<String>,
    pub expiration: Option<Option<i64>>,
    pub burn_after_reads: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub access: Option<String>,
    pub owner_user_id: Option<i64>,
    pub mine: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}

impl Services {
    pub fn new(repo: Repository) -> Self {
        Self { repo }
    }

    pub fn principal(&self, req: &HttpRequest) -> Result<Principal, String> {
        if let Some(header) = req.headers().get("Authorization") {
            let header = header
                .to_str()
                .map_err(|_| "Invalid authorization header")?;
            let value = header
                .strip_prefix("Bearer ")
                .ok_or("Invalid authorization scheme")?;
            return api_keys::authenticate(value)?
                .map(Principal::Key)
                .ok_or_else(|| "Invalid bearer token".to_string());
        }
        match accounts::current(req) {
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

    pub fn list_pastes(
        &self,
        principal: &Principal,
        query: &PasteQuery,
        admin: bool,
    ) -> Result<Page<Paste>, String> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(30).clamp(1, 100);
        let offset = u64::from(page - 1).saturating_mul(u64::from(page_size));
        let user_id = principal.user_id();
        let search = format!("%{}%", query.search.as_deref().unwrap_or(""));
        let access = query.access.as_deref();
        let owner = if query.mine.unwrap_or(false) {
            user_id
        } else {
            query.owner_user_id
        };
        let conn = self.repo.conn()?;
        let visibility = if admin {
            "(:user_id IS NULL OR :user_id IS NOT NULL)"
        } else {
            "((:user_id IS NULL AND access='public') OR
              (:user_id IS NOT NULL AND (access='public' OR owner_user_id=:user_id)))"
        };
        let filter = format!(
            "{visibility} AND (:access IS NULL OR access=:access)
             AND (:owner IS NULL OR owner_user_id=:owner)
             AND (expiration IS NULL OR expiration>:now)
             AND (title LIKE :search OR content LIKE :search OR slug LIKE :search)"
        );
        let total = conn
            .query_row(
                &format!("SELECT count(*) FROM pasta WHERE {filter}"),
                rusqlite::named_params! {
                    ":user_id": user_id, ":access": access, ":owner": owner,
                    ":search": search, ":now": now()
                },
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT id,slug,owner_user_id,title,substr(content,1,500),kind,syntax,access,created,
                        expiration,last_read,read_count,burn_after_reads
                 FROM pasta WHERE {filter} ORDER BY created DESC LIMIT :limit OFFSET :offset"
            ))
            .map_err(|e| e.to_string())?;
        let items = stmt
            .query_map(
                rusqlite::named_params! {
                    ":user_id": user_id, ":access": access, ":owner": owner, ":search": search,
                    ":limit": page_size, ":offset": offset, ":now": now()
                },
                paste_row,
            )
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        Ok(Page {
            items,
            page,
            page_size,
            total,
        })
    }

    pub fn get_paste(&self, principal: &Principal, slug: &str) -> Result<Option<Paste>, String> {
        Ok(self
            .find_paste(slug)?
            .filter(|paste| can_read(principal, paste)))
    }

    fn find_paste(&self, slug: &str) -> Result<Option<Paste>, String> {
        let conn = self.repo.conn()?;
        let mut paste = conn
            .query_row(
                "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,
                        expiration,last_read,read_count,burn_after_reads
                 FROM pasta WHERE slug=?1 AND (expiration IS NULL OR expiration>?2)",
                params![slug, now()],
                paste_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(value) = &mut paste {
            value.files = self.files(value.id)?;
        }
        Ok(paste)
    }

    pub fn read_paste(&self, principal: &Principal, slug: &str) -> Result<Option<Paste>, String> {
        let mut conn = self.repo.conn()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let mut paste = tx
            .query_row(
                "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,
                        expiration,last_read,read_count,burn_after_reads
                 FROM pasta WHERE slug=?1 AND (expiration IS NULL OR expiration>?2)",
                params![slug, now()],
                paste_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some(mut paste) = paste.take().filter(|paste| can_read(principal, paste)) else {
            return Ok(None);
        };
        let next_reads = paste.read_count + 1;
        let burned = paste.burn_after_reads > 0 && next_reads >= paste.burn_after_reads;
        paste.files = files_from(&tx, paste.id)?;
        if burned {
            tx.execute("DELETE FROM pasta WHERE id=?1", params![paste.id])
                .map_err(|e| e.to_string())?;
        } else {
            tx.execute(
                "UPDATE pasta SET read_count=?2,last_read=?3 WHERE id=?1",
                params![paste.id, next_reads, now()],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        if burned {
            let _ =
                std::fs::remove_dir_all(self.repo.data_dir.join("attachments").join(&paste.slug));
        }
        paste.read_count = next_reads;
        Ok(Some(paste))
    }

    pub fn ensure_can_update(&self, principal: &Principal, slug: &str) -> Result<Paste, String> {
        let paste = self.find_paste(slug)?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        Ok(paste)
    }

    pub fn create_paste(&self, principal: &Principal, input: &PasteInput) -> Result<Paste, String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        if !principal.can("paste:write") && !matches!(principal, Principal::User(_)) {
            return Err("Missing paste:write scope".into());
        }
        validate_input(input, true)?;
        let now = now();
        let slug = Uuid::new_v4().simple().to_string()[..24].to_string();
        let conn = self.repo.conn()?;
        conn.execute(
            "INSERT INTO pasta(slug,owner_user_id,title,content,kind,syntax,access,created,
                               expiration,last_read,read_count,burn_after_reads)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?8,0,?10)",
            params![
                slug,
                owner,
                input.title.as_deref().unwrap_or("").trim(),
                input.content.as_deref().unwrap_or(""),
                input.kind.as_deref().unwrap_or("text"),
                input.syntax.as_deref().unwrap_or("none"),
                input.access.as_deref().unwrap_or("unlisted"),
                now,
                input.expiration.flatten(),
                input.burn_after_reads.unwrap_or(0)
            ],
        )
        .map_err(|e| e.to_string())?;
        self.get_paste(principal, &slug)?
            .ok_or("Paste creation failed".into())
    }

    pub fn update_paste(
        &self,
        principal: &Principal,
        slug: &str,
        input: &PasteInput,
    ) -> Result<Option<Paste>, String> {
        validate_input(input, false)?;
        let current = match self.find_paste(slug)? {
            Some(value) => value,
            None => return Ok(None),
        };
        authorize_owner(principal, &current, "paste:write")?;
        validate_url(
            input.kind.as_deref().unwrap_or(&current.kind),
            input.content.as_deref().unwrap_or(&current.content),
        )?;
        let conn = self.repo.conn()?;
        conn.execute(
            "UPDATE pasta SET title=coalesce(?2,title),content=coalesce(?3,content),
             kind=coalesce(?4,kind),syntax=coalesce(?5,syntax),access=coalesce(?6,access),
             expiration=CASE WHEN ?7 THEN ?8 ELSE expiration END,
             burn_after_reads=coalesce(?9,burn_after_reads)
             WHERE slug=?1",
            params![
                slug,
                input.title.as_deref().map(str::trim),
                input.content,
                input.kind,
                input.syntax,
                input.access,
                input.expiration.is_some(),
                input.expiration.flatten(),
                input.burn_after_reads
            ],
        )
        .map_err(|e| e.to_string())?;
        self.find_paste(slug)
    }

    pub fn delete_paste(&self, principal: &Principal, slug: &str) -> Result<bool, String> {
        let current = match self.find_paste(slug)? {
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
        match self
            .repo
            .conn()?
            .execute("DELETE FROM pasta WHERE slug=?1", params![slug])
        {
            Ok(1) => {
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

    pub fn add_files(
        &self,
        principal: &Principal,
        slug: &str,
        inputs: &[(String, String, i64)],
    ) -> Result<Vec<PasteFile>, String> {
        let paste = self.find_paste(slug)?.ok_or("Paste not found")?;
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
        let mut conn = self.repo.conn()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let starting_position: i64 = tx
            .query_row(
                "SELECT coalesce(max(position)+1,0) FROM pasta_file WHERE pasta_id=?1",
                params![paste.id],
                |row| row.get(0),
            )
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
            tx.execute(
                "INSERT INTO pasta_file(pasta_id,position,role,name,storage_name,size)
                 VALUES(?1,?2,?3,?4,?5,?6)",
                params![paste.id, position, role, name, storage_name, size],
            )
            .map_err(|e| e.to_string())?;
            files.push(PasteFile {
                id: tx.last_insert_rowid(),
                position,
                role: role.to_string(),
                name: name.clone(),
                storage_name: storage_name.clone(),
                size: *size,
            });
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(files)
    }

    pub fn delete_file(
        &self,
        principal: &Principal,
        slug: &str,
        file_id: i64,
    ) -> Result<bool, String> {
        let paste = self.find_paste(slug)?.ok_or("Paste not found")?;
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
        let result = self
            .repo
            .conn()?
            .execute("DELETE FROM pasta_file WHERE id=?1", params![file_id])
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

    fn files(&self, paste_id: i64) -> Result<Vec<PasteFile>, String> {
        let conn = self.repo.conn()?;
        files_from(&conn, paste_id)
    }
}

fn files_from(conn: &rusqlite::Connection, paste_id: i64) -> Result<Vec<PasteFile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id,position,role,name,storage_name,size FROM pasta_file
             WHERE pasta_id=?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    let files = stmt
        .query_map(params![paste_id], |row| {
            Ok(PasteFile {
                id: row.get(0)?,
                position: row.get(1)?,
                role: row.get(2)?,
                name: row.get(3)?,
                storage_name: row.get(4)?,
                size: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    Ok(files)
}

fn can_read(principal: &Principal, paste: &Paste) -> bool {
    if paste.access != "owner" {
        return true;
    }
    principal.can("paste:admin")
        || match principal {
            Principal::User(session) => Some(session.user.id) == paste.owner_user_id,
            Principal::Key(key) => {
                key.user_id == paste.owner_user_id && key.has_scope("paste:read")
            }
            Principal::Anonymous => false,
        }
}

fn paste_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Paste> {
    Ok(Paste {
        id: row.get(0)?,
        slug: row.get(1)?,
        owner_user_id: row.get(2)?,
        title: row.get(3)?,
        content: row.get(4)?,
        kind: row.get(5)?,
        syntax: row.get(6)?,
        access: row.get(7)?,
        created: row.get(8)?,
        expiration: row.get(9)?,
        last_read: row.get(10)?,
        read_count: row.get(11)?,
        burn_after_reads: row.get(12)?,
        files: Vec::new(),
    })
}

fn authorize_owner(principal: &Principal, paste: &Paste, scope: &str) -> Result<(), String> {
    if principal.can("paste:admin")
        || (principal.user_id() == paste.owner_user_id
            && (matches!(principal, Principal::User(_)) || principal.can(scope)))
    {
        Ok(())
    } else {
        Err("You do not own this paste".into())
    }
}

fn validate_input(input: &PasteInput, creating: bool) -> Result<(), String> {
    if input
        .title
        .as_deref()
        .is_some_and(|v| v.chars().count() > 200)
    {
        return Err("Title exceeds 200 characters".into());
    }
    if input
        .kind
        .as_deref()
        .is_some_and(|v| !matches!(v, "text" | "url"))
    {
        return Err("Kind must be text or url".into());
    }
    if input
        .access
        .as_deref()
        .is_some_and(|v| !matches!(v, "public" | "unlisted" | "owner"))
    {
        return Err("Access must be public, unlisted, or owner".into());
    }
    if input.burn_after_reads.is_some_and(|v| v < 0) {
        return Err("Burn count cannot be negative".into());
    }
    if creating {
        validate_url(
            input.kind.as_deref().unwrap_or("text"),
            input.content.as_deref().unwrap_or(""),
        )?;
    }
    Ok(())
}

fn validate_url(kind: &str, content: &str) -> Result<(), String> {
    if kind != "url" {
        return Ok(());
    }
    let parsed = url::Url::parse(content).map_err(|_| "URL content is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("URL pastes support only http and https".into());
    }
    Ok(())
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{can_read, validate_url, Paste, PasteFile, Principal, Services};
    use crate::repository::Repository;
    use crate::util::accounts::{SessionUser, User};
    use crate::util::api_keys::ApiKey;
    use rusqlite::params;

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

    #[test]
    fn burn_after_read_is_committed_with_the_consuming_read() {
        let path = std::env::temp_dir().join(format!("racebin-burn-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let repository = Repository::open(&path).unwrap();
        repository.migrate().unwrap();
        let conn = repository.conn().unwrap();
        conn.execute(
            "INSERT INTO app_user(id,username,password_hash,role,created)
             VALUES(7,'owner','unused','user',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,
             created,read_count,burn_after_reads)
             VALUES(1,'burn',7,'','secret','text','none','owner',0,0,1)",
            [],
        )
        .unwrap();
        drop(conn);
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
        let consumed = services.read_paste(&principal, "burn").unwrap().unwrap();
        assert_eq!(consumed.content, "secret");
        let remaining: i64 = repository
            .conn()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM pasta WHERE slug=?1",
                params!["burn"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
        let _ = std::fs::remove_dir_all(path);
    }
}
