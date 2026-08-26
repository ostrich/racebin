use rand::{distr::Alphanumeric, RngExt};
use sqlx::{Any, Row};

use crate::crypto::sha256_hex;
use crate::database::Database;
use crate::domain_error::{DomainError, DomainResult};
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

#[derive(Clone, Debug)]
pub struct ApiKey {
    pub id: i64,
    pub user_id: Option<i64>,
    pub owner_username: Option<String>,
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
    sha256_hex(token)
}

pub fn normalize_scopes(scopes: &[String]) -> Result<Vec<String>, &'static str> {
    let mut normalized = Vec::new();
    for scope in scopes {
        let scope = scope.trim();
        if !VALID_SCOPES.contains(&scope) {
            return Err("Unknown API key scope");
        }
        if normalized.iter().any(|candidate| candidate == scope) {
            return Err("API key scopes must be unique");
        }
        normalized.push(scope.to_string());
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
) -> DomainResult<Vec<String>> {
    sqlx::query_scalar("SELECT scope FROM api_key_scopes WHERE api_key_id=$1 ORDER BY scope")
        .bind(api_key_id)
        .fetch_all(executor)
        .await
        .map_err(DomainError::from)
}

async fn from_row(repo: &Database, row: sqlx::any::AnyRow) -> DomainResult<ApiKey> {
    let id = row.try_get("id").map_err(DomainError::from)?;
    Ok(ApiKey {
        id,
        user_id: row.try_get("user_id").map_err(DomainError::from)?,
        owner_username: row.try_get("owner_username").map_err(DomainError::from)?,
        name: row.try_get("name").map_err(DomainError::from)?,
        token_prefix: row.try_get("token_prefix").map_err(DomainError::from)?,
        scopes: scopes_for(repo.pool(), id).await?,
        created_at: row.try_get("created_at").map_err(DomainError::from)?,
        last_used_at: row.try_get("last_used_at").map_err(DomainError::from)?,
        enabled: row
            .try_get::<i64, _>("enabled")
            .map_err(DomainError::from)?
            != 0,
    })
}

pub async fn create(
    repo: &Database,
    user_id: Option<i64>,
    name: &str,
    scopes: &[String],
) -> DomainResult<(ApiKey, String)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(DomainError::validation_code(
            "invalid_api_key_name",
            "Key name must contain 1 to 100 characters",
        ));
    }
    let scopes = normalize_scopes(scopes)
        .map_err(|message| DomainError::validation_code("invalid_api_key_scopes", message))?;
    let secret: String = rand::rng()
        .sample_iter(Alphanumeric)
        .take(48)
        .map(char::from)
        .collect();
    let token_prefix: String = rand::rng()
        .sample_iter(Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let token = format!("rbk_{token_prefix}_{secret}");
    let created_at = unix_timestamp();
    let mut transaction = repo.pool().begin().await.map_err(DomainError::from)?;
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
    .map_err(DomainError::from)?;
    for scope in &scopes {
        sqlx::query("INSERT INTO api_key_scopes(api_key_id,scope) VALUES($1,$2)")
            .bind(id)
            .bind(scope)
            .execute(&mut *transaction)
            .await
            .map_err(DomainError::from)?;
    }
    transaction.commit().await.map_err(DomainError::from)?;
    Ok((
        ApiKey {
            id,
            user_id,
            owner_username: None,
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

pub async fn authenticate(repo: &Database, token: &str) -> DomainResult<Option<ApiKey>> {
    if !token.starts_with("rbk_") {
        return Ok(None);
    }
    let row = sqlx::query(
        "SELECT k.id,k.user_id,k.name,k.token_prefix,k.created_at,k.last_used_at,k.enabled,u.username AS owner_username
         FROM api_keys k LEFT JOIN users u ON u.id=k.user_id
         WHERE k.token_hash=$1 AND k.enabled=1
           AND (k.user_id IS NULL OR u.enabled=1)",
    )
    .bind(token_hash(token))
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?;
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
    .map_err(DomainError::from)?;
    Ok(Some(key))
}

pub struct ApiKeyListQuery<'a> {
    pub user_id: Option<i64>,
    pub include_privileged: bool,
    pub search: Option<&'a str>,
    pub enabled: Option<bool>,
    pub sort: &'a str,
    pub descending: bool,
    pub page: u32,
    pub page_size: u32,
}

pub async fn list_page(
    repo: &Database,
    query: &ApiKeyListQuery<'_>,
) -> DomainResult<crate::pastes::Page<ApiKey>> {
    let search = query
        .search
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{}%", value.trim().to_lowercase()))
        .unwrap_or_default();
    let owner_id = query.user_id.unwrap_or(0);
    let enabled_value = query.enabled.map_or(-1, i64::from);
    let privileged = i64::from(query.include_privileged);
    let where_clause = " WHERE ($1=0 OR k.user_id=$1)
        AND ($2=1 OR u.id IS NULL OR (u.role<>'admin' AND u.is_owner=0))
        AND ($3='' OR LOWER(k.name) LIKE $3 OR LOWER(k.token_prefix) LIKE $3 OR LOWER(COALESCE(u.username,'')) LIKE $3
             OR EXISTS (SELECT 1 FROM api_key_scopes s WHERE s.api_key_id=k.id AND LOWER(s.scope) LIKE $3))
        AND ($4=-1 OR k.enabled=$4)";
    let total_items = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM api_keys k LEFT JOIN users u ON u.id=k.user_id{where_clause}"
    )))
    .bind(owner_id)
    .bind(privileged)
    .bind(&search)
    .bind(enabled_value)
    .fetch_one(repo.pool())
    .await
    .map_err(DomainError::from)?;
    let sort_field = match query.sort {
        "name" => "LOWER(k.name)",
        "owner" => "LOWER(COALESCE(u.username,''))",
        "used" => "COALESCE(k.last_used_at,0)",
        _ => "k.created_at",
    };
    let direction = if query.descending { "DESC" } else { "ASC" };
    let sql = sqlx::AssertSqlSafe(format!(
        "SELECT k.id,k.user_id,k.name,k.token_prefix,k.created_at,k.last_used_at,k.enabled,u.username AS owner_username
         FROM api_keys k LEFT JOIN users u ON u.id=k.user_id{where_clause}
         ORDER BY {sort_field} {direction},k.id {direction} LIMIT $5 OFFSET $6"
    ));
    let rows = sqlx::query(sql)
        .bind(owner_id)
        .bind(privileged)
        .bind(search)
        .bind(enabled_value)
        .bind(i64::from(query.page_size))
        .bind(i64::from(query.page.saturating_sub(1)) * i64::from(query.page_size))
        .fetch_all(repo.pool())
        .await
        .map_err(DomainError::from)?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(from_row(repo, row).await?);
    }
    Ok(crate::pastes::Page {
        items,
        page: query.page,
        page_size: query.page_size,
        total_items,
    })
}

pub async fn get(repo: &Database, id: i64) -> DomainResult<Option<ApiKey>> {
    let row = sqlx::query(
        "SELECT k.id,k.user_id,k.name,k.token_prefix,k.created_at,k.last_used_at,k.enabled,u.username AS owner_username
         FROM api_keys k LEFT JOIN users u ON u.id=k.user_id WHERE k.id=$1",
    )
    .bind(id)
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?;
    match row {
        Some(row) => from_row(repo, row).await.map(Some),
        None => Ok(None),
    }
}

pub async fn set_enabled_for_user(
    repo: &Database,
    id: i64,
    user_id: i64,
    enabled: bool,
) -> DomainResult<bool> {
    sqlx::query("UPDATE api_keys SET enabled=$3 WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn delete_for_user(repo: &Database, id: i64, user_id: i64) -> DomainResult<bool> {
    sqlx::query("DELETE FROM api_keys WHERE id=$1 AND user_id=$2")
        .bind(id)
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn set_enabled(repo: &Database, id: i64, enabled: bool) -> DomainResult<bool> {
    sqlx::query("UPDATE api_keys SET enabled=$2 WHERE id=$1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn delete(repo: &Database, id: i64) -> DomainResult<bool> {
    sqlx::query("DELETE FROM api_keys WHERE id=$1")
        .bind(id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn delete_all_for_user(repo: &Database, user_id: i64) -> DomainResult<u64> {
    sqlx::query("DELETE FROM api_keys WHERE user_id=$1")
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected())
        .map_err(DomainError::from)
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
        assert!(normalize_scopes(&scopes).is_err());
        assert_eq!(
            normalize_scopes(&["paste:write".to_string(), "paste:read".to_string()]).unwrap(),
            vec!["paste:read", "paste:write"]
        );
        assert!(normalize_scopes(&["unknown".to_string()]).is_err());
        assert!(normalize_scopes(&[]).is_err());
    }
}
