use crate::repository::Repository;
use crate::util::{accounts, api_keys};
use actix_web::HttpRequest;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
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
    pub size: i64,
}

#[derive(Debug, Deserialize)]
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
        if let Some(value) = req
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        {
            return Ok(api_keys::authenticate(value)?
                .map(Principal::Key)
                .unwrap_or(Principal::Anonymous));
        }
        Ok(accounts::current(req)
            .map(Principal::User)
            .unwrap_or(Principal::Anonymous))
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
        let offset = (page - 1) * page_size;
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
                "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,
                        expiration,last_read,read_count,burn_after_reads
                 FROM pasta WHERE {filter} ORDER BY created DESC LIMIT :limit OFFSET :offset"
            ))
            .map_err(|e| e.to_string())?;
        let mut items = stmt
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
        for paste in &mut items {
            paste.files = self.files(paste.id)?;
        }
        Ok(Page {
            items,
            page,
            page_size,
            total,
        })
    }

    pub fn get_paste(&self, principal: &Principal, slug: &str) -> Result<Option<Paste>, String> {
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
        if let Some(value) = &paste {
            if value.access == "owner"
                && principal.user_id() != value.owner_user_id
                && !principal.can("paste:admin")
            {
                return Ok(None);
            }
        }
        if let Some(value) = &mut paste {
            value.files = self.files(value.id)?;
        }
        Ok(paste)
    }

    pub fn read_paste(&self, principal: &Principal, slug: &str) -> Result<Option<Paste>, String> {
        let mut paste = match self.get_paste(principal, slug)? {
            Some(paste) => paste,
            None => return Ok(None),
        };
        let next_reads = paste.read_count + 1;
        if paste.burn_after_reads > 0 && next_reads >= paste.burn_after_reads {
            self.repo
                .conn()?
                .execute("DELETE FROM pasta WHERE id=?1", params![paste.id])
                .map_err(|e| e.to_string())?;
        } else {
            self.repo
                .conn()?
                .execute(
                    "UPDATE pasta SET read_count=?2,last_read=?3 WHERE id=?1",
                    params![paste.id, next_reads, now()],
                )
                .map_err(|e| e.to_string())?;
        }
        paste.read_count = next_reads;
        Ok(Some(paste))
    }

    pub fn create_paste(&self, principal: &Principal, input: &PasteInput) -> Result<Paste, String> {
        let owner = principal.user_id().ok_or("Authentication required")?;
        if !principal.can("paste:write") && !matches!(principal, Principal::User(_)) {
            return Err("Missing paste:write scope".into());
        }
        validate_input(input, true)?;
        let now = now();
        let slug = Uuid::new_v4().simple().to_string()[..12].to_string();
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
        let current = match self.get_paste(principal, slug)? {
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
        self.get_paste(principal, slug)
    }

    pub fn delete_paste(&self, principal: &Principal, slug: &str) -> Result<bool, String> {
        let current = match self.get_paste(principal, slug)? {
            Some(value) => value,
            None => return Ok(false),
        };
        authorize_owner(principal, &current, "paste:delete")?;
        let deleted = self
            .repo
            .conn()?
            .execute("DELETE FROM pasta WHERE slug=?1", params![slug])
            .map(|count| count == 1)
            .map_err(|e| e.to_string())?;
        if deleted {
            let _ = std::fs::remove_dir_all(self.repo.data_dir.join("attachments").join(slug));
        }
        Ok(deleted)
    }

    pub fn add_file(
        &self,
        principal: &Principal,
        slug: &str,
        name: &str,
        size: i64,
    ) -> Result<PasteFile, String> {
        let paste = self.get_paste(principal, slug)?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let conn = self.repo.conn()?;
        let position: i64 = conn
            .query_row(
                "SELECT coalesce(max(position)+1,0) FROM pasta_file WHERE pasta_id=?1",
                params![paste.id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let role = if paste.files.iter().any(|file| file.role == "primary") {
            "attachment"
        } else {
            "primary"
        };
        conn.execute(
            "INSERT INTO pasta_file(pasta_id,position,role,name,size) VALUES(?1,?2,?3,?4,?5)",
            params![paste.id, position, role, name, size],
        )
        .map_err(|e| e.to_string())?;
        Ok(PasteFile {
            id: conn.last_insert_rowid(),
            position,
            role: role.to_string(),
            name: name.to_string(),
            size,
        })
    }

    pub fn delete_file(
        &self,
        principal: &Principal,
        slug: &str,
        file_id: i64,
    ) -> Result<Option<String>, String> {
        let paste = self.get_paste(principal, slug)?.ok_or("Paste not found")?;
        authorize_owner(principal, &paste, "paste:write")?;
        let file = paste.files.into_iter().find(|file| file.id == file_id);
        if file.is_some() {
            self.repo
                .conn()?
                .execute("DELETE FROM pasta_file WHERE id=?1", params![file_id])
                .map_err(|e| e.to_string())?;
        }
        Ok(file.map(|file| file.name))
    }

    fn files(&self, paste_id: i64) -> Result<Vec<PasteFile>, String> {
        let conn = self.repo.conn()?;
        let mut stmt = conn
            .prepare("SELECT id,position,role,name,size FROM pasta_file WHERE pasta_id=?1 ORDER BY position")
            .map_err(|e| e.to_string())?;
        let files = stmt
            .query_map(params![paste_id], |row| {
                Ok(PasteFile {
                    id: row.get(0)?,
                    position: row.get(1)?,
                    role: row.get(2)?,
                    name: row.get(3)?,
                    size: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        Ok(files)
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
    if creating && input.content.as_deref().unwrap_or("").is_empty() {
        return Err("Content is required".into());
    }
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
    use super::validate_url;

    #[test]
    fn url_pastes_accept_only_http_destinations() {
        assert!(validate_url("url", "https://example.com/path").is_ok());
        assert!(validate_url("url", "javascript:alert(1)").is_err());
        assert!(validate_url("url", "not a url").is_err());
        assert!(validate_url("text", "not a url").is_ok());
    }
}
