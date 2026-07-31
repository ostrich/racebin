use rand::{distributions::Alphanumeric, Rng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{Any, Row};

use crate::repository::Repository;
use crate::time::unix_timestamp;

pub const VALID_SCOPES: &[&str] = &[
    "paste:read",
    "paste:write",
    "paste:delete",
    "paste:list",
    "paste:manage",
    "user:manage",
    "invitation:manage",
    "api_key:manage",
];

#[derive(Clone, Debug, Serialize)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub enabled: bool,
}

impl ApiKey {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|candidate| candidate == scope)
    }
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub fn normalize_scopes(scopes: &[String]) -> Result<Vec<String>, &'static str> {
    let mut normalized = Vec::new();
    for scope in scopes {
        let scope = scope.trim();
        if !VALID_SCOPES.contains(&scope) {
            return Err("Unknown API key scope");
        }
        if !normalized.iter().any(|candidate| candidate == scope) {
            normalized.push(scope.to_string());
        }
    }
    if normalized.is_empty() {
        return Err("Select at least one API key scope");
    }
    normalized.sort_unstable();
    Ok(normalized)
}

async fn scopes_for(
    executor: impl sqlx::Executor<'_, Database = Any>,
    api_key_id: i64,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar("SELECT scope FROM api_key_scopes WHERE api_key_id=$1 ORDER BY scope")
        .bind(api_key_id)
        .fetch_all(executor)
        .await
        .map_err(|error| error.to_string())
}

async fn from_row(repo: &Repository, row: sqlx::any::AnyRow) -> Result<ApiKey, String> {
    let id = row.try_get("id").map_err(|error| error.to_string())?;
    Ok(ApiKey {
        id,
        user_id: row.try_get("user_id").map_err(|error| error.to_string())?,
        name: row.try_get("name").map_err(|error| error.to_string())?,
        token_prefix: row
            .try_get("token_prefix")
            .map_err(|error| error.to_string())?,
        scopes: scopes_for(repo.pool(), id).await?,
        created_at: row
            .try_get("created_at")
            .map_err(|error| error.to_string())?,
        last_used_at: row
            .try_get("last_used_at")
            .map_err(|error| error.to_string())?,
        enabled: row
            .try_get::<i64, _>("enabled")
            .map_err(|error| error.to_string())?
            != 0,
    })
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
    let token_prefix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let token = format!("rbk_{token_prefix}_{secret}");
    let created_at = unix_timestamp();
    let mut transaction = repo
        .pool()
        .begin()
        .await
        .map_err(|error| error.to_string())?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO api_keys(user_id,name,token_prefix,token_hash,created_at)
         VALUES($1,$2,$3,$4,$5) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(&token_prefix)
    .bind(token_hash(&token))
    .bind(created_at)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| error.to_string())?;
    for scope in &scopes {
        sqlx::query("INSERT INTO api_key_scopes(api_key_id,scope) VALUES($1,$2)")
            .bind(id)
            .bind(scope)
            .execute(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;
    }
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;
    Ok((
        ApiKey {
            id,
            user_id,
            name: name.to_string(),
            token_prefix,
            scopes,
            created_at,
            last_used_at: None,
            enabled: true,
        },
        token,
    ))
}

pub async fn authenticate(repo: &Repository, token: &str) -> Result<Option<ApiKey>, String> {
    if !token.starts_with("rbk_") {
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT k.id,k.user_id,k.name,k.token_prefix,k.created_at,k.last_used_at,k.enabled
         FROM api_keys k LEFT JOIN users u ON u.id=k.user_id
         WHERE k.token_hash=$1 AND k.enabled=1
           AND (k.user_id IS NULL OR u.enabled=1)",
    )
    .bind(token_hash(token))
    .fetch_optional(repo.pool())
    .await
    .map_err(|error| error.to_string())?;
    let Some(row) = row else {
        return Ok(None);
    };
    let key = from_row(repo, row).await?;
    sqlx::query(
        "UPDATE api_keys SET last_used_at=$2 WHERE id=$1
         AND (last_used_at IS NULL OR last_used_at<$2-300)",
    )
    .bind(key.id)
    .bind(unix_timestamp())
    .execute(repo.pool())
    .await
    .map_err(|error| error.to_string())?;
    Ok(Some(key))
}

async fn list_where(repo: &Repository, user_id: Option<i64>) -> Result<Vec<ApiKey>, String> {
    let rows = if let Some(user_id) = user_id {
        sqlx::query(
            "SELECT id,user_id,name,token_prefix,created_at,last_used_at,enabled
             FROM api_keys WHERE user_id=$1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(repo.pool())
        .await
    } else {
        sqlx::query(
            "SELECT id,user_id,name,token_prefix,created_at,last_used_at,enabled
             FROM api_keys ORDER BY created_at DESC",
        )
        .fetch_all(repo.pool())
        .await
    }
    .map_err(|error| error.to_string())?;
    let mut keys = Vec::with_capacity(rows.len());
    for row in rows {
        keys.push(from_row(repo, row).await?);
    }
    Ok(keys)
}

pub async fn list(repo: &Repository) -> Result<Vec<ApiKey>, String> {
    list_where(repo, None).await
}

pub async fn list_for_user(repo: &Repository, user_id: i64) -> Result<Vec<ApiKey>, String> {
    list_where(repo, Some(user_id)).await
}

pub async fn set_enabled_for_user(
    repo: &Repository,
    id: i64,
    user_id: i64,
    enabled: bool,
) -> Result<bool, String> {
    sqlx::query("UPDATE api_keys SET enabled=$3 WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| error.to_string())
}

pub async fn delete_for_user(repo: &Repository, id: i64, user_id: i64) -> Result<bool, String> {
    sqlx::query("DELETE FROM api_keys WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| error.to_string())
}

pub async fn set_enabled(repo: &Repository, id: i64, enabled: bool) -> Result<bool, String> {
    sqlx::query("UPDATE api_keys SET enabled=$2 WHERE id=$1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| error.to_string())
}

pub async fn delete(repo: &Repository, id: i64) -> Result<bool, String> {
    sqlx::query("DELETE FROM api_keys WHERE id=$1")
        .bind(id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|error| error.to_string())
}

pub async fn delete_all_for_user(repo: &Repository, user_id: i64) -> Result<u64, String> {
    sqlx::query("DELETE FROM api_keys WHERE user_id=$1")
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected())
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
        assert_eq!(
            normalize_scopes(&scopes).unwrap(),
            vec!["paste:read", "paste:write"]
        );
        assert!(normalize_scopes(&["unknown".to_string()]).is_err());
        assert!(normalize_scopes(&[]).is_err());
    }
}
