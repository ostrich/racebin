use rand::{distributions::Alphanumeric, Rng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::any::AnyRow;
use sqlx::{FromRow, Row};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::repository::Repository;

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

impl<'r> FromRow<'r, AnyRow> for ApiKey {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            name: row.try_get("name")?,
            prefix: row.try_get("prefix")?,
            scopes: row.try_get("scopes")?,
            created: row.try_get("created")?,
            last_used: row.try_get("last_used")?,
            enabled: row.try_get::<i64, _>("enabled")? != 0,
        })
    }
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

pub async fn create(
    repo: &Repository,
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
    let prefix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let token = format!("rbk_{prefix}_{secret}");
    let created = now();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO api_key(user_id,name,prefix,token_hash,scopes,created)
         VALUES($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(&prefix)
    .bind(token_hash(&token))
    .bind(&scopes)
    .bind(created)
    .fetch_one(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok((
        ApiKey {
            id,
            user_id,
            name: name.to_string(),
            prefix,
            scopes,
            created,
            last_used: None,
            enabled: true,
        },
        token,
    ))
}

pub async fn authenticate(repo: &Repository, token: &str) -> Result<Option<ApiKey>, String> {
    if !token.starts_with("rbk_") {
        return Ok(None);
    }
    let key = sqlx::query_as::<_, ApiKey>(
        "SELECT k.id,k.user_id,k.name,k.prefix,k.scopes,k.created,k.last_used,k.enabled
         FROM api_key k LEFT JOIN app_user u ON u.id=k.user_id
         WHERE k.token_hash=$1 AND k.enabled=1
           AND (k.user_id IS NULL OR u.enabled=1)",
    )
    .bind(token_hash(token))
    .fetch_optional(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    if let Some(key) = &key {
        sqlx::query(
            "UPDATE api_key SET last_used=$2 WHERE id=$1
             AND (last_used IS NULL OR last_used<$2-300)",
        )
        .bind(key.id)
        .bind(now())
        .execute(repo.pool())
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(key)
}

pub async fn list(repo: &Repository) -> Result<Vec<ApiKey>, String> {
    sqlx::query_as(
        "SELECT id,user_id,name,prefix,scopes,created,last_used,enabled
         FROM api_key ORDER BY created DESC",
    )
    .fetch_all(repo.pool())
    .await
    .map_err(|e| e.to_string())
}

pub async fn list_for_user(repo: &Repository, user_id: i64) -> Result<Vec<ApiKey>, String> {
    sqlx::query_as(
        "SELECT id,user_id,name,prefix,scopes,created,last_used,enabled
         FROM api_key WHERE user_id=$1 ORDER BY created DESC",
    )
    .bind(user_id)
    .fetch_all(repo.pool())
    .await
    .map_err(|e| e.to_string())
}

pub async fn set_enabled_for_user(
    repo: &Repository,
    id: i64,
    user_id: i64,
    enabled: bool,
) -> Result<bool, String> {
    sqlx::query("UPDATE api_key SET enabled=$3 WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|e| e.to_string())
}

pub async fn delete_for_user(repo: &Repository, id: i64, user_id: i64) -> Result<bool, String> {
    sqlx::query("DELETE FROM api_key WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|e| e.to_string())
}

pub async fn set_enabled(repo: &Repository, id: i64, enabled: bool) -> Result<bool, String> {
    sqlx::query("UPDATE api_key SET enabled=$2 WHERE id=$1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|e| e.to_string())
}

pub async fn delete(repo: &Repository, id: i64) -> Result<bool, String> {
    sqlx::query("DELETE FROM api_key WHERE id=$1")
        .bind(id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|e| e.to_string())
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
