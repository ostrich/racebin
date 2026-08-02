use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};
use sqlx::any::AnyRow;
use sqlx::{FromRow, Row};
use std::sync::LazyLock;

use crate::domain_error::{DomainError, DomainResult};
use crate::repository::{DatabaseKind, Repository};
use crate::time::unix_timestamp;

mod administration;
pub mod api_keys;
mod invitations;
mod throttling;

pub use administration::*;
pub use invitations::*;
pub use throttling::*;

pub const SESSION_COOKIE: &str = "racebin_session";
static DUMMY_PASSWORD_HASH: LazyLock<String> =
    LazyLock::new(|| password_hash("racebin-dummy-password").expect("valid dummy password"));

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub password_change_required: bool,
}

#[derive(Clone, Debug)]
pub struct AdminUser {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub password_change_required: bool,
    pub created_at: i64,
    pub last_login_at: Option<i64>,
    pub paste_count: i64,
    pub storage_bytes: i64,
    pub active_session_count: i64,
    pub api_key_count: i64,
    pub active_api_key_count: i64,
}

impl<'r> FromRow<'r, AnyRow> for AdminUser {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            role: row.try_get("role")?,
            enabled: row.try_get::<i64, _>("enabled")? != 0,
            password_change_required: row.try_get::<i64, _>("password_change_required")? != 0,
            created_at: row.try_get("created_at")?,
            last_login_at: row.try_get("last_login_at")?,
            paste_count: row.try_get("paste_count")?,
            storage_bytes: row.try_get("storage_bytes")?,
            active_session_count: row.try_get("active_session_count")?,
            api_key_count: row.try_get("api_key_count")?,
            active_api_key_count: row.try_get("active_api_key_count")?,
        })
    }
}

impl<'r> FromRow<'r, AnyRow> for User {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            role: row.try_get("role")?,
            enabled: row.try_get::<i64, _>("enabled")? != 0,
            password_change_required: row.try_get::<i64, _>("password_change_required")? != 0,
        })
    }
}

impl User {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Clone, Debug)]
pub struct SessionUser {
    pub user: User,
    pub csrf_token: String,
}

#[derive(Clone, Debug)]
pub struct Invitation {
    pub id: i64,
    pub token_prefix: String,
    pub token: Option<String>,
    pub expires_at: i64,
    pub redeemed: bool,
    pub redeemed_by_username: Option<String>,
    pub revoked: bool,
}

impl<'r> FromRow<'r, AnyRow> for Invitation {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            token_prefix: row.try_get("token_prefix")?,
            token: row.try_get("token")?,
            expires_at: row.try_get("expires_at")?,
            redeemed: row.try_get::<i64, _>("redeemed")? != 0,
            redeemed_by_username: row.try_get("redeemed_by_username")?,
            revoked: row.try_get::<i64, _>("revoked")? != 0,
        })
    }
}

impl Invitation {
    pub fn status(&self) -> &'static str {
        if self.redeemed {
            "Redeemed"
        } else if self.revoked {
            "Revoked"
        } else if self.expires_at <= unix_timestamp() {
            "Expired"
        } else {
            "Active"
        }
    }

    pub fn is_active(&self) -> bool {
        !self.redeemed && !self.revoked && self.expires_at > unix_timestamp()
    }
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn random_token(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn password_hash(password: &str) -> DomainResult<String> {
    if password.chars().count() < 12 {
        return Err(DomainError::validation_code(
            "invalid_password",
            "Password must contain at least 12 characters",
        ));
    }
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|error| DomainError::internal(error.to_string()))
}

pub(crate) fn validate_username(username: &str) -> DomainResult<&str> {
    let username = username.trim();
    if username.len() < 3
        || username.len() > 64
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    {
        return Err(DomainError::validation_code(
            "invalid_username",
            "Username must be 3-64 ASCII letters, numbers, underscores, or hyphens",
        ));
    }
    Ok(username)
}

pub async fn verify_user(
    repo: &Repository,
    username: &str,
    password: &str,
) -> DomainResult<Option<User>> {
    #[derive(FromRow)]
    struct UserPassword {
        id: i64,
        username: String,
        role: String,
        enabled: i64,
        password_change_required: i64,
        password_hash: String,
    }
    let row = sqlx::query_as::<_, UserPassword>(
        "SELECT id,username,role,enabled,password_change_required,password_hash
         FROM users WHERE username=$1",
    )
    .bind(username)
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?;
    let encoded = row
        .as_ref()
        .map(|value| value.password_hash.as_str())
        .unwrap_or(DUMMY_PASSWORD_HASH.as_str());
    let valid = PasswordHash::new(encoded).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    });
    Ok(row.and_then(|value| {
        (valid && value.enabled != 0).then_some(User {
            id: value.id,
            username: value.username,
            role: value.role,
            enabled: value.enabled != 0,
            password_change_required: value.password_change_required != 0,
        })
    }))
}

pub async fn create_session(
    repo: &Repository,
    user_id: i64,
    remember: bool,
) -> DomainResult<(String, String, i64)> {
    let token = random_token(64);
    let csrf = random_token(48);
    let created_at = unix_timestamp();
    let expires_at = created_at + if remember { 30 * 86400 } else { 12 * 3600 };
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND expires_at<=$2")
        .bind(user_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query(
        "INSERT INTO sessions(user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
         VALUES($1,$2,$3,$4,$5,$4)",
    )
    .bind(user_id)
    .bind(hash(&token))
    .bind(&csrf)
    .bind(created_at)
    .bind(expires_at)
    .execute(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    sqlx::query("UPDATE users SET last_login_at=$2 WHERE id=$1")
        .bind(user_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query(
        "DELETE FROM sessions WHERE user_id=$1 AND id NOT IN (
           SELECT id FROM sessions WHERE user_id=$1
           ORDER BY last_used_at DESC,id DESC LIMIT 20
         )",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok((token, csrf, expires_at))
}

pub async fn session_user(repo: &Repository, token: &str) -> DomainResult<Option<SessionUser>> {
    #[derive(FromRow)]
    struct SessionRow {
        id: i64,
        username: String,
        role: String,
        enabled: i64,
        password_change_required: i64,
        csrf_token: String,
        session_id: i64,
    }
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT u.id,u.username,u.role,u.enabled,u.password_change_required,
                s.csrf_token,s.id AS session_id
         FROM sessions s JOIN users u ON u.id=s.user_id
         WHERE s.token_hash=$1 AND s.expires_at>$2 AND u.enabled=1",
    )
    .bind(hash(token))
    .bind(unix_timestamp())
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?;
    if let Some(value) = &row {
        let current = unix_timestamp();
        let _ =
            sqlx::query("UPDATE sessions SET last_used_at=$2 WHERE id=$1 AND last_used_at<$2-300")
                .bind(value.session_id)
                .bind(current)
                .execute(repo.pool())
                .await;
    }
    Ok(row.map(|value| SessionUser {
        user: User {
            id: value.id,
            username: value.username,
            role: value.role,
            enabled: value.enabled != 0,
            password_change_required: value.password_change_required != 0,
        },
        csrf_token: value.csrf_token,
    }))
}

pub async fn delete_session(repo: &Repository, token: &str) -> DomainResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(hash(token))
        .execute(repo.pool())
        .await
        .map(|_| ())
        .map_err(DomainError::from)
}

pub async fn list_users(repo: &Repository) -> DomainResult<Vec<User>> {
    sqlx::query_as(
        "SELECT id,username,role,enabled,password_change_required
         FROM users ORDER BY username",
    )
    .fetch_all(repo.pool())
    .await
    .map_err(DomainError::from)
}

#[cfg(test)]
mod tests {
    use super::Invitation;

    #[test]
    fn invitation_status_respects_terminal_states_and_expiration() {
        let invitation = |expires_at, redeemed, revoked| Invitation {
            id: 1,
            token_prefix: "example".to_string(),
            token: None,
            expires_at,
            redeemed,
            redeemed_by_username: None,
            revoked,
        };
        assert_eq!(invitation(i64::MAX, false, false).status(), "Active");
        assert_eq!(invitation(0, false, false).status(), "Expired");
        assert_eq!(invitation(i64::MAX, true, false).status(), "Redeemed");
        assert_eq!(invitation(i64::MAX, false, true).status(), "Revoked");
    }
}
