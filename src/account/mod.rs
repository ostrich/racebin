use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use rand::{distributions::Alphanumeric, Rng};
use sha2::{Digest, Sha256};
use sqlx::any::AnyRow;
use sqlx::{FromRow, Row};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::repository::{DatabaseKind, Repository};
use crate::time::unix_timestamp;

pub mod api_keys;

pub const SESSION_COOKIE: &str = "racebin_session";
static LOGIN_FAILURES: LazyLock<Mutex<HashMap<String, Vec<i64>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
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

pub fn password_hash(password: &str) -> Result<String, String> {
    if password.chars().count() < 12 {
        return Err("Password must contain at least 12 characters".to_string());
    }
    Argon2::default()
        .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|hash| hash.to_string())
        .map_err(|error| error.to_string())
}

pub(crate) fn validate_username(username: &str) -> Result<&str, String> {
    let username = username.trim();
    if username.len() < 3
        || username.len() > 64
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    {
        return Err(
            "Username must be 3-64 ASCII letters, numbers, underscores, or hyphens".to_string(),
        );
    }
    Ok(username)
}

pub async fn verify_user(
    repo: &Repository,
    username: &str,
    password: &str,
) -> Result<Option<User>, String> {
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
    .map_err(|e| e.to_string())?;
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
) -> Result<(String, String, i64), String> {
    let token = random_token(64);
    let csrf = random_token(48);
    let created_at = unix_timestamp();
    let expires_at = created_at + if remember { 30 * 86400 } else { 12 * 3600 };
    sqlx::query(
        "INSERT INTO sessions(user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
         VALUES($1,$2,$3,$4,$5,$4)",
    )
    .bind(user_id)
    .bind(hash(&token))
    .bind(&csrf)
    .bind(created_at)
    .bind(expires_at)
    .execute(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok((token, csrf, expires_at))
}

pub async fn session_user(repo: &Repository, token: &str) -> Result<Option<SessionUser>, String> {
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
    .map_err(|e| e.to_string())?;
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

pub async fn delete_session(repo: &Repository, token: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(hash(token))
        .execute(repo.pool())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn list_users(repo: &Repository) -> Result<Vec<User>, String> {
    sqlx::query_as(
        "SELECT id,username,role,enabled,password_change_required
         FROM users ORDER BY username",
    )
    .fetch_all(repo.pool())
    .await
    .map_err(|e| e.to_string())
}

pub async fn set_enabled(repo: &Repository, id: i64, enabled: bool) -> Result<(), String> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    if !enabled {
        let target: Option<(String, i64)> =
            sqlx::query_as(&format!("SELECT role,enabled FROM users WHERE id=$1{lock}"))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        let (role, currently_enabled) = target.ok_or("User not found")?;
        let admins: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        if role == "admin" && currently_enabled != 0 && admins <= 1 {
            return Err("The last enabled administrator cannot be disabled".to_string());
        }
    }
    let result = sqlx::query("UPDATE users SET enabled=$2 WHERE id=$1")
        .bind(id)
        .bind(i64::from(enabled))
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("User not found".to_string());
    }
    if !enabled {
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn set_role(repo: &Repository, id: i64, admin: bool) -> Result<(), String> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    if !admin {
        let target: Option<(String, i64)> =
            sqlx::query_as("SELECT role,enabled FROM users WHERE id=$1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        let (role, enabled) = target.ok_or("User not found")?;
        let admins: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        if role == "admin" && enabled != 0 && admins <= 1 {
            return Err("The last administrator cannot be demoted".to_string());
        }
    }
    let result = sqlx::query("UPDATE users SET role=$2 WHERE id=$1")
        .bind(id)
        .bind(if admin { "admin" } else { "user" })
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("User not found".to_string());
    }
    if !admin {
        sqlx::query(
            "UPDATE api_keys SET enabled=0 WHERE user_id=$1 AND EXISTS (
               SELECT 1 FROM api_key_scopes
               WHERE api_key_id=api_keys.id
                 AND scope IN ('paste:manage','user:manage','invitation:manage','api_key:manage')
             )",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn set_password(
    repo: &Repository,
    id: i64,
    password: &str,
    force: bool,
) -> Result<(), String> {
    let encoded = password_hash(password)?;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE users SET password_hash=$2,password_change_required=$3 WHERE id=$1")
        .bind(id)
        .bind(encoded)
        .bind(i64::from(force))
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn create_invitation(
    repo: &Repository,
    created_by_user_id: i64,
) -> Result<String, String> {
    let token = random_token(64);
    sqlx::query(
        "INSERT INTO invitations(token_hash,created_by_user_id,expires_at) VALUES($1,$2,$3)",
    )
    .bind(hash(&token))
    .bind(created_by_user_id)
    .bind(unix_timestamp() + 86400)
    .execute(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok(token)
}

pub async fn list_invitations(repo: &Repository) -> Result<Vec<Invitation>, String> {
    sqlx::query_as(
        "SELECT i.id,substr(i.token_hash,1,10) AS token_prefix,i.expires_at,i.redeemed,
                u.username AS redeemed_by_username,i.revoked
         FROM invitations i
         LEFT JOIN users u ON u.id=i.redeemed_by_user_id
         ORDER BY i.id DESC",
    )
    .fetch_all(repo.pool())
    .await
    .map_err(|e| e.to_string())
}

pub async fn revoke_invitation(repo: &Repository, id: i64) -> Result<bool, String> {
    sqlx::query("UPDATE invitations SET revoked=1 WHERE id=$1 AND redeemed=0")
        .bind(id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(|e| e.to_string())
}

pub async fn redeem_invitation(
    repo: &Repository,
    token: &str,
    username: &str,
    password: &str,
) -> Result<User, String> {
    let _write_guard = repo.lock_writes().await;
    let username = validate_username(username)?.to_string();
    let encoded = password_hash(password)?;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let invitation_id: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT id FROM invitations
         WHERE token_hash=$1 AND expires_at>$2 AND redeemed=0 AND revoked=0{lock}"
    ))
    .bind(hash(token))
    .bind(unix_timestamp())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let invitation_id = invitation_id.ok_or("Invitation is invalid or expired")?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users(username,password_hash,role,password_change_required,created_at)
         VALUES($1,$2,'user',0,$3) RETURNING id",
    )
    .bind(&username)
    .bind(encoded)
    .bind(unix_timestamp())
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query("UPDATE invitations SET redeemed=1,redeemed_by_user_id=$2 WHERE id=$1")
        .bind(invitation_id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(User {
        id,
        username,
        role: "user".to_string(),
        enabled: true,
        password_change_required: false,
    })
}

pub fn login_allowed(username: &str, client: &str) -> bool {
    let key = format!("{}\n{}", username.to_ascii_lowercase(), client);
    let mut failures = LOGIN_FAILURES.lock().unwrap();
    let cutoff = unix_timestamp() - 900;
    failures.retain(|_, attempts| {
        attempts.retain(|timestamp| *timestamp > cutoff);
        !attempts.is_empty()
    });
    if failures.len() > 10_000 {
        failures.clear();
    }
    failures.entry(key).or_default().len() < 5
}

pub fn record_login_failure(username: &str, client: &str) {
    LOGIN_FAILURES
        .lock()
        .unwrap()
        .entry(format!("{}\n{}", username.to_ascii_lowercase(), client))
        .or_default()
        .push(unix_timestamp());
}

pub fn clear_login_failures(username: &str, client: &str) {
    LOGIN_FAILURES.lock().unwrap().remove(&format!(
        "{}\n{}",
        username.to_ascii_lowercase(),
        client
    ));
}

#[cfg(test)]
mod tests {
    use super::Invitation;

    #[test]
    fn invitation_status_respects_terminal_states_and_expiration() {
        let invitation = |expires_at, redeemed, revoked| Invitation {
            id: 1,
            token_prefix: "example".to_string(),
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
