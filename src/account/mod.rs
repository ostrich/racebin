use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use rand::{distributions::Alphanumeric, Rng};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::any::AnyRow;
use sqlx::{FromRow, Row};
use std::sync::LazyLock;

use crate::repository::{DatabaseKind, Repository};
use crate::time::unix_timestamp;

pub mod api_keys;

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

#[derive(Clone, Debug, Serialize)]
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
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND expires_at<=$2")
        .bind(user_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
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
    .map_err(|e| e.to_string())?;
    sqlx::query("UPDATE users SET last_login_at=$2 WHERE id=$1")
        .bind(user_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "DELETE FROM sessions WHERE user_id=$1 AND id NOT IN (
           SELECT id FROM sessions WHERE user_id=$1
           ORDER BY last_used_at DESC,id DESC LIMIT 20
         )",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
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

fn admin_user_query(repo: &Repository) -> String {
    let text_size = if repo.kind() == DatabaseKind::Postgres {
        "CAST(octet_length(p.content) AS BIGINT) + COALESCE(CAST(octet_length(p.document_json) AS BIGINT),0)"
    } else {
        "length(CAST(p.content AS BLOB)) + COALESCE(length(CAST(p.document_json AS BLOB)),0)"
    };
    format!(
        "SELECT u.id,u.username,u.role,u.enabled,u.password_change_required,u.created_at,u.last_login_at,
          CAST((SELECT count(*) FROM pastes p WHERE p.owner_id=u.id) AS BIGINT) AS paste_count,
          CAST(COALESCE((SELECT sum({text_size} + COALESCE((SELECT sum(a.size_bytes) FROM attachments a WHERE a.paste_id=p.id),0)) FROM pastes p WHERE p.owner_id=u.id),0) AS BIGINT) AS storage_bytes,
          CAST((SELECT count(*) FROM sessions s WHERE s.user_id=u.id AND s.expires_at>$1) AS BIGINT) AS active_session_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id) AS BIGINT) AS api_key_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id AND k.enabled=1 AND u.enabled=1) AS BIGINT) AS active_api_key_count
         FROM users u"
    )
}

pub async fn list_admin_users(repo: &Repository) -> Result<Vec<AdminUser>, String> {
    sqlx::query_as(&format!(
        "{} ORDER BY lower(u.username)",
        admin_user_query(repo)
    ))
    .bind(unix_timestamp())
    .fetch_all(repo.pool())
    .await
    .map_err(|e| e.to_string())
}

pub async fn admin_user(repo: &Repository, id: i64) -> Result<Option<AdminUser>, String> {
    sqlx::query_as(&format!("{} WHERE u.id=$2", admin_user_query(repo)))
        .bind(unix_timestamp())
        .bind(id)
        .fetch_optional(repo.pool())
        .await
        .map_err(|e| e.to_string())
}

pub async fn set_enabled(repo: &Repository, id: i64, enabled: bool) -> Result<(), String> {
    update_user(repo, id, Some(enabled), None).await
}

pub async fn set_role(repo: &Repository, id: i64, admin: bool) -> Result<(), String> {
    update_user(repo, id, None, Some(admin)).await
}

pub async fn update_user(
    repo: &Repository,
    id: i64,
    enabled: Option<bool>,
    admin: Option<bool>,
) -> Result<(), String> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let target: Option<(String, i64)> =
        sqlx::query_as(&format!("SELECT role,enabled FROM users WHERE id=$1{lock}"))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    let (current_role, currently_enabled) = target.ok_or("User not found")?;
    let final_enabled = enabled.unwrap_or(currently_enabled != 0);
    let final_admin = admin.unwrap_or(current_role == "admin");
    if current_role == "admin" && currently_enabled != 0 && (!final_enabled || !final_admin) {
        let admins: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        if admins <= 1 {
            return Err(if !final_enabled {
                "The last enabled administrator cannot be disabled".to_string()
            } else {
                "The last administrator cannot be demoted".to_string()
            });
        }
    }
    let result = sqlx::query("UPDATE users SET enabled=$2,role=$3 WHERE id=$1")
        .bind(id)
        .bind(i64::from(final_enabled))
        .bind(if final_admin { "admin" } else { "user" })
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err("User not found".to_string());
    }
    if !final_enabled {
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    if !final_admin {
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
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn create_password_reset(
    repo: &Repository,
    user_id: i64,
    created_by_user_id: i64,
) -> Result<String, String> {
    let _write_guard = repo.lock_writes().await;
    let token = random_token(64);
    let now = unix_timestamp();
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    match enabled {
        None => return Err("User not found".to_string()),
        Some(0) => return Err("Disabled users cannot reset their password".to_string()),
        Some(_) => {}
    }
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "INSERT INTO password_reset_tokens(user_id,token_hash,created_by_user_id,created_at,expires_at)
         VALUES($1,$2,$3,$4,$5)",
    )
    .bind(user_id)
    .bind(hash(&token))
    .bind(created_by_user_id)
    .bind(now)
    .bind(now + 3600)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(token)
}

pub async fn reset_password(repo: &Repository, token: &str, password: &str) -> Result<(), String> {
    let token_hash = hash(token);
    let valid: Option<i64> = sqlx::query_scalar(
        "SELECT r.user_id FROM password_reset_tokens r JOIN users u ON u.id=r.user_id
         WHERE r.token_hash=$1 AND r.expires_at>$2 AND u.enabled=1",
    )
    .bind(&token_hash)
    .bind(unix_timestamp())
    .fetch_optional(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    if valid.is_none() {
        return Err("Password reset link is invalid or expired".to_string());
    }
    let encoded = password_hash(password)?;
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let user_id: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT r.user_id FROM password_reset_tokens r JOIN users u ON u.id=r.user_id
         WHERE r.token_hash=$1 AND r.expires_at>$2 AND u.enabled=1{lock}"
    ))
    .bind(token_hash)
    .bind(unix_timestamp())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let user_id = user_id.ok_or("Password reset link is invalid or expired")?;
    sqlx::query("UPDATE users SET password_hash=$2,password_change_required=0 WHERE id=$1")
        .bind(user_id)
        .bind(encoded)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn revoke_sessions(repo: &Repository, user_id: i64) -> Result<bool, String> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_optional(repo.pool())
        .await
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Ok(false);
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|_| true)
        .map_err(|e| e.to_string())
}

pub async fn create_invitation(
    repo: &Repository,
    created_by_user_id: i64,
) -> Result<String, String> {
    let token = random_token(64);
    sqlx::query(
        "INSERT INTO invitations(token_hash,token,created_by_user_id,expires_at) VALUES($1,$2,$3,$4)",
    )
    .bind(hash(&token))
    .bind(&token)
    .bind(created_by_user_id)
    .bind(unix_timestamp() + 86400)
    .execute(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    Ok(token)
}

pub async fn list_invitations(repo: &Repository) -> Result<Vec<Invitation>, String> {
    sqlx::query_as(
        "SELECT i.id,COALESCE(substr(i.token,1,10),substr(i.token_hash,1,10)) AS token_prefix,
                i.token,i.expires_at,i.redeemed,
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
    sqlx::query("UPDATE invitations SET revoked=1,token=NULL WHERE id=$1 AND redeemed=0")
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
    let token_hash = hash(token);
    let active: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM invitations
         WHERE token_hash=$1 AND expires_at>$2 AND redeemed=0 AND revoked=0",
    )
    .bind(&token_hash)
    .bind(unix_timestamp())
    .fetch_optional(repo.pool())
    .await
    .map_err(|e| e.to_string())?;
    if active.is_none() {
        return Err("Invitation is invalid or expired".into());
    }
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
    .bind(token_hash)
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
    sqlx::query("UPDATE invitations SET redeemed=1,redeemed_by_user_id=$2,token=NULL WHERE id=$1")
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

const ATTEMPT_WINDOW_SECONDS: i64 = 900;

async fn retry_after(
    repo: &Repository,
    keys: &[(&str, String, i64)],
) -> Result<Option<u64>, String> {
    let now = unix_timestamp();
    let cutoff = now - ATTEMPT_WINDOW_SECONDS;
    sqlx::query("DELETE FROM auth_attempts WHERE occurred_at<=$1")
        .bind(cutoff)
        .execute(repo.pool())
        .await
        .map_err(|e| e.to_string())?;
    let mut retry = None;
    for (kind, subject, limit) in keys {
        let (count, first): (i64, Option<i64>) = sqlx::query_as(
            "SELECT count(*),min(occurred_at) FROM auth_attempts
             WHERE kind=$1 AND subject=$2 AND occurred_at>$3",
        )
        .bind(kind)
        .bind(subject)
        .bind(cutoff)
        .fetch_one(repo.pool())
        .await
        .map_err(|e| e.to_string())?;
        if count >= *limit {
            let seconds = (first.unwrap_or(now) + ATTEMPT_WINDOW_SECONDS - now).max(1) as u64;
            retry = Some(retry.map_or(seconds, |current: u64| current.max(seconds)));
        }
    }
    Ok(retry)
}

pub async fn login_retry_after(
    repo: &Repository,
    username: &str,
    client: &str,
) -> Result<Option<u64>, String> {
    retry_after(
        repo,
        &[
            ("login_account", username.to_ascii_lowercase(), 5),
            ("login_address", client.to_string(), 20),
        ],
    )
    .await
}

pub async fn record_login_failure(
    repo: &Repository,
    username: &str,
    client: &str,
) -> Result<(), String> {
    let now = unix_timestamp();
    let mut tx = repo.pool().begin().await.map_err(|e| e.to_string())?;
    for (kind, subject) in [
        ("login_account", username.to_ascii_lowercase()),
        ("login_address", client.to_string()),
    ] {
        sqlx::query("INSERT INTO auth_attempts(kind,subject,occurred_at) VALUES($1,$2,$3)")
            .bind(kind)
            .bind(subject)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn clear_login_failures(repo: &Repository, username: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM auth_attempts WHERE kind='login_account' AND subject=$1")
        .bind(username.to_ascii_lowercase())
        .execute(repo.pool())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub async fn invitation_retry_after(
    repo: &Repository,
    client: &str,
) -> Result<Option<u64>, String> {
    retry_after(repo, &[("invitation_address", client.to_string(), 20)]).await
}

pub async fn record_invitation_failure(repo: &Repository, client: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO auth_attempts(kind,subject,occurred_at)
         VALUES('invitation_address',$1,$2)",
    )
    .bind(client)
    .bind(unix_timestamp())
    .execute(repo.pool())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub async fn password_reset_retry_after(
    repo: &Repository,
    client: &str,
) -> Result<Option<u64>, String> {
    retry_after(repo, &[("password_reset_address", client.to_string(), 20)]).await
}

pub async fn record_password_reset_failure(repo: &Repository, client: &str) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO auth_attempts(kind,subject,occurred_at)
         VALUES('password_reset_address',$1,$2)",
    )
    .bind(client)
    .bind(unix_timestamp())
    .execute(repo.pool())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
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
