use serde::Serialize;
use sqlx::{any::AnyRow, FromRow, Row};

use crate::accounts::api_keys::ApiKey;
use crate::database::Database;
use crate::domain_error::{DomainError, DomainResult};
use crate::pastes::{Page, Principal};
use crate::time::unix_timestamp;

#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub id: i64,
    pub actor_user_id: Option<i64>,
    pub actor_username: String,
    pub actor_api_key_id: Option<i64>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub target_label: Option<String>,
    pub details: String,
    pub created_at: i64,
}

impl<'r> FromRow<'r, AnyRow> for AuditEvent {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            actor_user_id: row.try_get("actor_user_id")?,
            actor_username: row.try_get("actor_username")?,
            actor_api_key_id: row.try_get("actor_api_key_id")?,
            action: row.try_get("action")?,
            target_type: row.try_get("target_type")?,
            target_id: row.try_get("target_id")?,
            target_label: row.try_get("target_label")?,
            details: row.try_get("details")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

fn actor(principal: &Principal) -> Option<(i64, Option<String>, Option<i64>)> {
    match principal {
        Principal::Session(session) => {
            Some((session.user.id, Some(session.user.username.clone()), None))
        }
        Principal::ApiKey(ApiKey {
            user_id: Some(id),
            id: key_id,
            ..
        }) => Some((*id, None, Some(*key_id))),
        _ => None,
    }
}

pub async fn record(
    repo: &Database,
    principal: &Principal,
    action: &str,
    target_type: &str,
    target_id: Option<String>,
    target_label: Option<String>,
    details: impl Serialize,
) -> DomainResult<()> {
    let Some((actor_user_id, actor_username, actor_api_key_id)) = actor(principal) else {
        return Ok(());
    };
    let actor_username = match actor_username {
        Some(username) => username,
        None => sqlx::query_scalar("SELECT username FROM users WHERE id=$1")
            .bind(actor_user_id)
            .fetch_optional(repo.pool())
            .await
            .map_err(DomainError::from)?
            .unwrap_or_else(|| format!("User #{actor_user_id}")),
    };
    let details = serde_json::to_string(&details)
        .map_err(|error| DomainError::internal(error.to_string()))?;
    sqlx::query(
        "INSERT INTO audit_events(actor_user_id,actor_username,actor_api_key_id,action,target_type,target_id,target_label,details,created_at)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(actor_user_id).bind(actor_username).bind(actor_api_key_id).bind(action).bind(target_type)
    .bind(target_id).bind(target_label).bind(details).bind(unix_timestamp())
    .execute(repo.pool()).await.map(|_| ()).map_err(DomainError::from)
}

pub async fn list_page(
    repo: &Database,
    search: Option<&str>,
    page: u32,
    page_size: u32,
) -> DomainResult<Page<AuditEvent>> {
    let search = search
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::database::literal_like_pattern(&value.trim().to_lowercase()))
        .unwrap_or_default();
    let condition = " WHERE ($1='' OR LOWER(actor_username) LIKE $1 ESCAPE '\\' OR LOWER(action) LIKE $1 ESCAPE '\\'
        OR LOWER(target_type) LIKE $1 ESCAPE '\\' OR LOWER(COALESCE(target_label,'')) LIKE $1 ESCAPE '\\' OR LOWER(COALESCE(target_id,'')) LIKE $1 ESCAPE '\\')";
    let total_items = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM audit_events{condition}"
    )))
    .bind(&search)
    .fetch_one(repo.pool())
    .await
    .map_err(DomainError::from)?;
    let items = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT id,actor_user_id,actor_username,actor_api_key_id,action,target_type,target_id,target_label,details,created_at
         FROM audit_events{condition} ORDER BY created_at DESC,id DESC LIMIT $2 OFFSET $3"
    )))
    .bind(search)
    .bind(i64::from(page_size))
    .bind(i64::from(page.saturating_sub(1)) * i64::from(page_size))
    .fetch_all(repo.pool()).await.map_err(DomainError::from)?;
    Ok(Page {
        items,
        page,
        page_size,
        total_items,
    })
}
