use actix_web::cookie::Cookie;
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, Rng};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::args::ARGS;

pub const SESSION_COOKIE: &str = "racebin_session";
static LOGIN_FAILURES: Lazy<Mutex<HashMap<String, Vec<i64>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static DUMMY_PASSWORD_HASH: Lazy<String> =
    Lazy::new(|| password_hash("racebin-dummy-password").expect("valid dummy password"));

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub force_password_change: bool,
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
pub struct Invite {
    pub id: i64,
    pub token_prefix: String,
    pub expires: i64,
    pub used: bool,
    pub revoked: bool,
}

impl Invite {
    pub fn status(&self) -> &'static str {
        if self.used {
            "Used"
        } else if self.revoked {
            "Revoked"
        } else if self.expires <= now() {
            "Expired"
        } else {
            "Active"
        }
    }

    pub fn is_active(&self) -> bool {
        !self.used && !self.revoked && self.expires > now()
    }
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
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

pub fn connection_at(data_dir: &str) -> Result<Connection, String> {
    let conn = Connection::open(format!("{data_dir}/database.sqlite"))
        .map_err(|error| error.to_string())?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA busy_timeout = 5000;
        CREATE TABLE IF NOT EXISTS app_user (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user', 'admin')),
            enabled INTEGER NOT NULL DEFAULT 1,
            force_password_change INTEGER NOT NULL DEFAULT 0,
            created INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_session (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
            token_hash TEXT NOT NULL UNIQUE,
            csrf_token TEXT NOT NULL,
            created INTEGER NOT NULL,
            expires INTEGER NOT NULL,
            last_used INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS user_invite (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            token_hash TEXT NOT NULL UNIQUE,
            created_by INTEGER NOT NULL REFERENCES app_user(id),
            expires INTEGER NOT NULL,
            used INTEGER NOT NULL DEFAULT 0,
            revoked INTEGER NOT NULL DEFAULT 0
        );
        ",
    )
    .map_err(|error| error.to_string())?;
    Ok(conn)
}

pub fn connection() -> Result<Connection, String> {
    connection_at(&ARGS.data_dir)
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

fn validate_username(username: &str) -> Result<&str, String> {
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

pub fn verify_user(username: &str, password: &str) -> Result<Option<User>, String> {
    let conn = connection()?;
    let row: Option<(User, String)> = conn
        .query_row(
            "SELECT id, username, role, enabled, force_password_change, password_hash
             FROM app_user WHERE username = ?1",
            params![username],
            |row| {
                Ok((
                    User {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        role: row.get(2)?,
                        enabled: row.get(3)?,
                        force_password_change: row.get(4)?,
                    },
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((user, stored_hash)) = row else {
        if let Ok(parsed) = PasswordHash::new(&DUMMY_PASSWORD_HASH) {
            let _ = Argon2::default().verify_password(password.as_bytes(), &parsed);
        }
        return Ok(None);
    };
    if !user.enabled {
        return Ok(None);
    }
    let valid = PasswordHash::new(&stored_hash).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    });
    Ok(valid.then_some(user))
}

pub fn create_session(user_id: i64, remember: bool) -> Result<(String, String, i64), String> {
    let token = random_token(64);
    let csrf = random_token(48);
    let created = now();
    let expires = created + if remember { 30 * 86400 } else { 86400 };
    let conn = connection()?;
    conn.execute(
        "INSERT INTO user_session (user_id, token_hash, csrf_token, created, expires, last_used)
         VALUES (?1, ?2, ?3, ?4, ?5, ?4)",
        params![user_id, hash(&token), csrf, created, expires],
    )
    .map_err(|error| error.to_string())?;
    Ok((token, csrf, expires))
}

pub fn session_user(token: &str) -> Result<Option<SessionUser>, String> {
    let conn = connection()?;
    let session = conn
        .query_row(
            "SELECT u.id, u.username, u.role, u.enabled, u.force_password_change, s.csrf_token, s.id
             FROM user_session s JOIN app_user u ON u.id = s.user_id
             WHERE s.token_hash = ?1 AND s.expires > ?2 AND u.enabled = 1",
            params![hash(token), now()],
            |row| {
                Ok((
                    SessionUser {
                        user: User {
                            id: row.get(0)?,
                            username: row.get(1)?,
                            role: row.get(2)?,
                            enabled: row.get(3)?,
                            force_password_change: row.get(4)?,
                        },
                        csrf_token: row.get(5)?,
                    },
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some((_, id)) = &session {
        let _ = conn.execute(
            "UPDATE user_session SET last_used = ?2 WHERE id = ?1 AND last_used < ?2 - 300",
            params![id, now()],
        );
    }
    Ok(session.map(|(session, _)| session))
}

pub fn delete_session(token: &str) -> Result<(), String> {
    connection()?
        .execute(
            "DELETE FROM user_session WHERE token_hash = ?1",
            params![hash(token)],
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn list_users() -> Result<Vec<User>, String> {
    let conn = connection()?;
    let mut statement = conn
        .prepare("SELECT id, username, role, enabled, force_password_change FROM app_user ORDER BY username")
        .map_err(|error| error.to_string())?;
    let users = statement
        .query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                username: row.get(1)?,
                role: row.get(2)?,
                enabled: row.get(3)?,
                force_password_change: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    Ok(users)
}

pub fn set_enabled(id: i64, enabled: bool) -> Result<(), String> {
    let mut conn = connection()?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    if !enabled {
        let target_is_admin: bool = tx
            .query_row(
                "SELECT role = 'admin' AND enabled = 1 FROM app_user WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "User not found".to_string())?;
        let enabled_admins: i64 = tx
            .query_row(
                "SELECT count(*) FROM app_user WHERE role = 'admin' AND enabled = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if target_is_admin && enabled_admins <= 1 {
            return Err("The last enabled administrator cannot be disabled".to_string());
        }
    }
    let changed = tx
        .execute(
            "UPDATE app_user SET enabled = ?2 WHERE id = ?1",
            params![id, enabled as i32],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("User not found".to_string());
    }
    if !enabled {
        tx.execute("DELETE FROM user_session WHERE user_id = ?1", params![id])
            .map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())
}

pub fn set_role(id: i64, admin: bool) -> Result<(), String> {
    let mut conn = connection()?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    if !admin {
        let target_is_admin: bool = tx
            .query_row(
                "SELECT role = 'admin' AND enabled = 1 FROM app_user WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "User not found".to_string())?;
        let admins: i64 = tx
            .query_row(
                "SELECT count(*) FROM app_user WHERE role = 'admin' AND enabled = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if target_is_admin && admins <= 1 {
            return Err("The last administrator cannot be demoted".to_string());
        }
    }
    let changed = tx
        .execute(
            "UPDATE app_user SET role = ?2 WHERE id = ?1",
            params![id, if admin { "admin" } else { "user" }],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        return Err("User not found".to_string());
    }
    if !admin {
        tx.execute(
            "UPDATE api_key SET enabled=0 WHERE user_id=?1 AND scopes LIKE '%:admin%'",
            params![id],
        )
        .map_err(|error| error.to_string())?;
    }
    tx.commit().map_err(|error| error.to_string())
}

pub fn set_password(id: i64, password: &str, force: bool) -> Result<(), String> {
    let conn = connection()?;
    conn.execute(
        "UPDATE app_user SET password_hash = ?2, force_password_change = ?3 WHERE id = ?1",
        params![id, password_hash(password)?, force as i32],
    )
    .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM user_session WHERE user_id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn create_invite(created_by: i64) -> Result<String, String> {
    let token = random_token(64);
    connection()?
        .execute(
            "INSERT INTO user_invite (token_hash, created_by, expires) VALUES (?1, ?2, ?3)",
            params![hash(&token), created_by, now() + 86400],
        )
        .map_err(|error| error.to_string())?;
    Ok(token)
}

pub fn list_invites() -> Result<Vec<Invite>, String> {
    let conn = connection()?;
    let mut statement = conn
        .prepare(
            "SELECT id, substr(token_hash, 1, 10), expires, used, revoked
             FROM user_invite ORDER BY id DESC",
        )
        .map_err(|error| error.to_string())?;
    let invites = statement
        .query_map([], |row| {
            Ok(Invite {
                id: row.get(0)?,
                token_prefix: row.get(1)?,
                expires: row.get(2)?,
                used: row.get(3)?,
                revoked: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    Ok(invites)
}

pub fn revoke_invite(id: i64) -> Result<bool, String> {
    connection()?
        .execute(
            "UPDATE user_invite SET revoked = 1 WHERE id = ?1 AND used = 0",
            params![id],
        )
        .map(|changed| changed == 1)
        .map_err(|error| error.to_string())
}

pub fn accept_invite(token: &str, username: &str, password: &str) -> Result<User, String> {
    let mut conn = connection()?;
    let transaction = conn.transaction().map_err(|error| error.to_string())?;
    let id: Option<i64> = transaction
        .query_row(
            "SELECT id FROM user_invite WHERE token_hash = ?1 AND expires > ?2 AND used = 0 AND revoked = 0",
            params![hash(token), now()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some(invite_id) = id else {
        return Err("Invitation is invalid or expired".to_string());
    };
    let username = validate_username(username)?;
    let password_hash = password_hash(password)?;
    transaction
        .execute(
            "INSERT INTO app_user (username, password_hash, role, force_password_change, created)
             VALUES (?1, ?2, 'user', 0, ?3)",
            params![username, password_hash, now()],
        )
        .map_err(|error| error.to_string())?;
    let user = User {
        id: transaction.last_insert_rowid(),
        username: username.to_string(),
        role: "user".to_string(),
        enabled: true,
        force_password_change: false,
    };
    transaction
        .execute(
            "UPDATE user_invite SET used = 1 WHERE id = ?1",
            params![invite_id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(user)
}

pub fn current(req: &actix_web::HttpRequest) -> Option<SessionUser> {
    req.cookie(SESSION_COOKIE)
        .and_then(|cookie| session_user(cookie.value()).ok().flatten())
}

pub fn login_allowed(username: &str, client: &str) -> bool {
    let key = format!("{}\n{}", username.to_ascii_lowercase(), client);
    let mut failures = LOGIN_FAILURES.lock().unwrap();
    let cutoff = now() - 900;
    failures.retain(|_, attempts| {
        attempts.retain(|timestamp| *timestamp > cutoff);
        !attempts.is_empty()
    });
    if failures.len() > 10_000 {
        failures.clear();
    }
    let attempts = failures.entry(key).or_default();
    attempts.len() < 5
}

pub fn record_login_failure(username: &str, client: &str) {
    let key = format!("{}\n{}", username.to_ascii_lowercase(), client);
    LOGIN_FAILURES
        .lock()
        .unwrap()
        .entry(key)
        .or_default()
        .push(now());
}

pub fn clear_login_failures(username: &str, client: &str) {
    let key = format!("{}\n{}", username.to_ascii_lowercase(), client);
    LOGIN_FAILURES.lock().unwrap().remove(&key);
}

pub fn session_cookie(token: String, remember: bool) -> Cookie<'static> {
    let mut builder = Cookie::build(SESSION_COOKIE, token)
        .path("/")
        .http_only(true)
        .secure(!ARGS.insecure_cookie)
        .same_site(actix_web::cookie::SameSite::Lax);
    if remember {
        builder = builder.max_age(actix_web::cookie::time::Duration::days(30));
    }
    builder.finish()
}

fn cli_password(arguments: &[String]) -> Result<String, String> {
    if let Some(index) = arguments
        .iter()
        .position(|value| value == "--password-file")
    {
        let path = arguments
            .get(index + 1)
            .ok_or_else(|| "--password-file requires a path".to_string())?;
        return fs::read_to_string(path)
            .map(|password| password.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|error| error.to_string());
    }
    rpassword::prompt_password("Password: ").map_err(|error| error.to_string())
}

fn cli_data_dir(arguments: &[String]) -> Result<String, String> {
    if let Some(index) = arguments.iter().position(|value| value == "--data-dir") {
        return arguments
            .get(index + 1)
            .cloned()
            .ok_or_else(|| "--data-dir requires a path".to_string());
    }
    Ok(std::env::var("RACEBIN_DATA_DIR").unwrap_or_else(|_| "racebin_data".to_string()))
}

pub fn run_cli_if_requested() -> Result<bool, String> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) != Some("account") {
        return Ok(false);
    }
    let command = arguments.get(2).map(String::as_str).unwrap_or("help");
    let data_dir = cli_data_dir(&arguments)?;
    fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
    let conn = connection_at(&data_dir)?;
    match command {
        "create" => {
            let username = arguments.get(3).ok_or_else(|| {
                "usage: racebin account create USERNAME [--admin] [--password-file PATH] [--data-dir PATH]".to_string()
            })?;
            let username = validate_username(username)?;
            let password = cli_password(&arguments)?;
            let role = if arguments.iter().any(|value| value == "--admin") {
                "admin"
            } else {
                "user"
            };
            conn.execute(
                "INSERT INTO app_user (username, password_hash, role, enabled, force_password_change, created)
                 VALUES (?1, ?2, ?3, 1, 0, ?4)",
                params![username, password_hash(&password)?, role, now()],
            )
            .map_err(|error| error.to_string())?;
            println!("created {role} account {username}");
        }
        "list" => {
            let mut statement = conn
                .prepare("SELECT username, role, enabled FROM app_user ORDER BY username")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                })
                .map_err(|error| error.to_string())?;
            for row in rows {
                let (username, role, enabled) = row.map_err(|error| error.to_string())?;
                println!(
                    "{username}\t{role}\t{}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
        }
        "password" => {
            let username = arguments.get(3).ok_or_else(|| {
                "usage: racebin account password USERNAME [--password-file PATH] [--data-dir PATH]".to_string()
            })?;
            let password = cli_password(&arguments)?;
            let changed = conn
                .execute(
                    "UPDATE app_user SET password_hash = ?2, force_password_change = 0 WHERE username = ?1",
                    params![username, password_hash(&password)?],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err(format!("account not found: {username}"));
            }
            conn.execute(
                "DELETE FROM user_session WHERE user_id = (SELECT id FROM app_user WHERE username = ?1)",
                params![username],
            )
            .map_err(|error| error.to_string())?;
            println!("password updated for {username}; existing sessions revoked");
        }
        "enable" | "disable" => {
            let username = arguments.get(3).ok_or_else(|| {
                format!("usage: racebin account {command} USERNAME [--data-dir PATH]")
            })?;
            let enabled = command == "enable";
            if !enabled {
                let would_remove_last: bool = conn
                    .query_row(
                        "SELECT role='admin' AND enabled=1
                         AND (SELECT count(*) FROM app_user WHERE role='admin' AND enabled=1)<=1
                         FROM app_user WHERE username=?1",
                        params![username],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|error| error.to_string())?
                    .unwrap_or(false);
                if would_remove_last {
                    return Err("The last enabled administrator cannot be disabled".to_string());
                }
            }
            let changed = conn
                .execute(
                    "UPDATE app_user SET enabled = ?2 WHERE username = ?1",
                    params![username, enabled as i32],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err(format!("account not found: {username}"));
            }
            if !enabled {
                conn.execute(
                    "DELETE FROM user_session WHERE user_id = (SELECT id FROM app_user WHERE username = ?1)",
                    params![username],
                )
                .map_err(|error| error.to_string())?;
            }
            println!("{command}d account {username}");
        }
        "role" => {
            let username = arguments.get(3).ok_or_else(|| {
                "usage: racebin account role USERNAME user|admin [--data-dir PATH]".to_string()
            })?;
            let role = arguments.get(4).map(String::as_str).unwrap_or("");
            if !matches!(role, "user" | "admin") {
                return Err("role must be user or admin".to_string());
            }
            if role == "user" {
                let would_remove_last: bool = conn
                    .query_row(
                        "SELECT role='admin' AND enabled=1
                         AND (SELECT count(*) FROM app_user WHERE role='admin' AND enabled=1)<=1
                         FROM app_user WHERE username=?1",
                        params![username],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|error| error.to_string())?
                    .unwrap_or(false);
                if would_remove_last {
                    return Err("The last enabled administrator cannot be demoted".to_string());
                }
            }
            let changed = conn
                .execute(
                    "UPDATE app_user SET role = ?2 WHERE username = ?1",
                    params![username, role],
                )
                .map_err(|error| error.to_string())?;
            if changed == 0 {
                return Err(format!("account not found: {username}"));
            }
            if role == "user" {
                conn.execute(
                    "UPDATE api_key SET enabled=0
                     WHERE user_id=(SELECT id FROM app_user WHERE username=?1)
                     AND scopes LIKE '%:admin%'",
                    params![username],
                )
                .map_err(|error| error.to_string())?;
            }
            println!("set {username} role to {role}");
        }
        _ => {
            println!(
                "usage: racebin account <create|list|password|enable|disable|role> [arguments]\n\
                 use --data-dir PATH to select the database"
            );
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::Invite;

    fn invite(expires: i64, used: bool, revoked: bool) -> Invite {
        Invite {
            id: 1,
            token_prefix: "example".to_string(),
            expires,
            used,
            revoked,
        }
    }

    #[test]
    fn invitation_status_respects_terminal_states_and_expiration() {
        assert_eq!(invite(i64::MAX, false, false).status(), "Active");
        assert_eq!(invite(0, false, false).status(), "Expired");
        assert_eq!(invite(i64::MAX, true, false).status(), "Used");
        assert_eq!(invite(i64::MAX, false, true).status(), "Revoked");
    }
}
