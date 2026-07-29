use rand::{distributions::Alphanumeric, Rng};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::args::ARGS;

pub const VALID_SCOPES: &[&str] = &[
    "paste:read",
    "paste:write",
    "paste:delete",
    "paste:list",
    "paste:admin",
    "user:admin",
    "invite:admin",
    "key:admin",
];

#[derive(Clone, Debug, Serialize)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub prefix: String,
    pub scopes: String,
    pub created: i64,
    pub last_used: Option<i64>,
    pub enabled: bool,
}

impl ApiKey {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.split(',').any(|item| item == scope)
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn connection() -> rusqlite::Result<Connection> {
    let conn = Connection::open(format!("{}/database.sqlite", ARGS.data_dir))?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS api_key (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER REFERENCES app_user(id),
            name TEXT NOT NULL,
            prefix TEXT NOT NULL UNIQUE,
            token_hash TEXT NOT NULL UNIQUE,
            scopes TEXT NOT NULL,
            created INTEGER NOT NULL,
            last_used INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1
        );
        ",
    )?;
    let _ = conn.execute("ALTER TABLE api_key ADD COLUMN user_id INTEGER", []);
    Ok(conn)
}

pub fn normalize_scopes(scopes: &[String]) -> Result<String, &'static str> {
    let mut normalized = Vec::new();
    for scope in scopes {
        let scope = scope.trim();
        if !VALID_SCOPES.contains(&scope) {
            return Err("Unknown API key scope");
        }
        if !normalized.contains(&scope) {
            normalized.push(scope);
        }
    }
    if normalized.is_empty() {
        return Err("Select at least one API key scope");
    }
    normalized.sort_unstable();
    Ok(normalized.join(","))
}

pub fn create(
    user_id: Option<i64>,
    name: &str,
    scopes: &[String],
) -> Result<(ApiKey, String), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err("Key name must contain 1 to 100 characters".to_string());
    }
    let scopes = normalize_scopes(scopes).map_err(str::to_string)?;
    let secret: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    let prefix = secret.chars().take(10).collect::<String>();
    let token = format!("mbk_{}_{}", prefix, secret);
    let created = now();
    let conn = connection().map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO api_key (user_id, name, prefix, token_hash, scopes, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![user_id, name, prefix, token_hash(&token), scopes, created],
    )
    .map_err(|error| error.to_string())?;
    let key = ApiKey {
        id: conn.last_insert_rowid(),
        user_id,
        name: name.to_string(),
        prefix,
        scopes,
        created,
        last_used: None,
        enabled: true,
    };
    Ok((key, token))
}

pub fn authenticate(token: &str) -> Result<Option<ApiKey>, String> {
    if !token.starts_with("mbk_") {
        return Ok(None);
    }
    let conn = connection().map_err(|error| error.to_string())?;
    let hash = token_hash(token);
    let key = conn
        .query_row(
            "SELECT k.id, k.user_id, k.name, k.prefix, k.scopes, k.created, k.last_used, k.enabled
             FROM api_key k LEFT JOIN app_user u ON u.id = k.user_id
             WHERE k.token_hash = ?1 AND k.enabled = 1
               AND (k.user_id IS NULL OR u.enabled = 1)",
            params![hash],
            |row| {
                Ok(ApiKey {
                    id: row.get(0)?,
                    user_id: row.get(1)?,
                    name: row.get(2)?,
                    prefix: row.get(3)?,
                    scopes: row.get(4)?,
                    created: row.get(5)?,
                    last_used: row.get(6)?,
                    enabled: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(key) = &key {
        conn.execute(
            "UPDATE api_key SET last_used = ?2 WHERE id = ?1",
            params![key.id, now()],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(key)
}

pub fn list() -> Result<Vec<ApiKey>, String> {
    let conn = connection().map_err(|error| error.to_string())?;
    let mut statement = conn
        .prepare(
            "SELECT id, user_id, name, prefix, scopes, created, last_used, enabled
             FROM api_key ORDER BY created DESC",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ApiKey {
                id: row.get(0)?,
                user_id: row.get(1)?,
                name: row.get(2)?,
                prefix: row.get(3)?,
                scopes: row.get(4)?,
                created: row.get(5)?,
                last_used: row.get(6)?,
                enabled: row.get(7)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

pub fn list_for_user(user_id: i64) -> Result<Vec<ApiKey>, String> {
    Ok(list()?
        .into_iter()
        .filter(|key| key.user_id == Some(user_id))
        .collect())
}

pub fn set_enabled_for_user(id: i64, user_id: i64, enabled: bool) -> Result<bool, String> {
    connection()
        .map_err(|error| error.to_string())?
        .execute(
            "UPDATE api_key SET enabled = ?3 WHERE id = ?1 AND user_id = ?2",
            params![id, user_id, enabled as i32],
        )
        .map(|changed| changed == 1)
        .map_err(|error| error.to_string())
}

pub fn delete_for_user(id: i64, user_id: i64) -> Result<bool, String> {
    connection()
        .map_err(|error| error.to_string())?
        .execute(
            "DELETE FROM api_key WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )
        .map(|changed| changed == 1)
        .map_err(|error| error.to_string())
}

pub fn set_enabled(id: i64, enabled: bool) -> Result<bool, String> {
    let conn = connection().map_err(|error| error.to_string())?;
    conn.execute(
        "UPDATE api_key SET enabled = ?2 WHERE id = ?1",
        params![id, enabled as i32],
    )
    .map(|changed| changed == 1)
    .map_err(|error| error.to_string())
}

pub fn delete(id: i64) -> Result<bool, String> {
    let conn = connection().map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM api_key WHERE id = ?1", params![id])
        .map(|changed| changed == 1)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_scopes;

    #[test]
    fn validates_and_normalizes_scopes() {
        let scopes = vec![
            "paste:write".to_string(),
            "paste:read".to_string(),
            "paste:write".to_string(),
        ];
        assert_eq!(normalize_scopes(&scopes).unwrap(), "paste:read,paste:write");
        assert!(normalize_scopes(&["unknown".to_string()]).is_err());
        assert!(normalize_scopes(&[]).is_err());
    }
}
