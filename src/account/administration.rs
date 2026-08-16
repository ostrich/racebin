use super::*;

fn admin_user_query(repo: &Repository) -> String {
    let text_size = if repo.kind() == DatabaseKind::Postgres {
        "CAST(octet_length(p.content) AS BIGINT)"
    } else {
        "length(CAST(p.content AS BLOB))"
    };
    format!(
        "SELECT u.id,u.username,u.role,u.is_owner,u.enabled,u.password_change_required,u.created_at,u.last_login_at,
          CAST((SELECT count(*) FROM pastes p WHERE p.owner_id=u.id) AS BIGINT) AS paste_count,
          CAST(COALESCE((SELECT sum({text_size} + COALESCE((SELECT sum(a.size_bytes) FROM attachments a WHERE a.paste_id=p.id),0)) FROM pastes p WHERE p.owner_id=u.id),0) AS BIGINT) AS storage_bytes,
          CAST((SELECT count(*) FROM sessions s WHERE s.user_id=u.id AND s.expires_at>$1) AS BIGINT) AS active_session_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id) AS BIGINT) AS api_key_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id AND k.enabled=1 AND u.enabled=1) AS BIGINT) AS active_api_key_count
         FROM users u"
    )
}

pub async fn list_admin_users(repo: &Repository) -> DomainResult<Vec<AdminUser>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{} ORDER BY lower(u.username)",
        admin_user_query(repo)
    )))
    .bind(unix_timestamp())
    .fetch_all(repo.pool())
    .await
    .map_err(DomainError::from)
}

pub async fn admin_user(repo: &Repository, id: i64) -> DomainResult<Option<AdminUser>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{} WHERE u.id=$2",
        admin_user_query(repo)
    )))
    .bind(unix_timestamp())
    .bind(id)
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)
}

pub async fn set_enabled(repo: &Repository, id: i64, enabled: bool) -> DomainResult<()> {
    update_user(repo, id, Some(enabled), None).await
}

pub async fn set_role(repo: &Repository, id: i64, admin: bool) -> DomainResult<()> {
    update_user(repo, id, None, Some(admin)).await
}

pub async fn update_user(
    repo: &Repository,
    id: i64,
    enabled: Option<bool>,
    admin: Option<bool>,
) -> DomainResult<()> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let target: Option<(String, i64, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT role,enabled,is_owner FROM users WHERE id=$1{lock}"
    )))
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    let (current_role, currently_enabled, is_owner) =
        target.ok_or_else(|| DomainError::not_found("User not found"))?;
    if is_owner != 0 && (enabled == Some(false) || admin == Some(false)) {
        return Err(DomainError::validation_code(
            "owner_protected",
            "The owner cannot be disabled or demoted",
        ));
    }
    let final_enabled = enabled.unwrap_or(currently_enabled != 0);
    let final_admin = admin.unwrap_or(current_role == "admin");
    if current_role == "admin" && currently_enabled != 0 && (!final_enabled || !final_admin) {
        let admins: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
                .fetch_one(&mut *tx)
                .await
                .map_err(DomainError::from)?;
        if admins <= 1 {
            return Err(DomainError::validation_code(
                "last_administrator",
                if !final_enabled {
                    "The last enabled administrator cannot be disabled"
                } else {
                    "The last administrator cannot be demoted"
                },
            ));
        }
    }
    let result = sqlx::query("UPDATE users SET enabled=$2,role=$3 WHERE id=$1")
        .bind(id)
        .bind(i64::from(final_enabled))
        .bind(if final_admin { "admin" } else { "user" })
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    if result.rows_affected() == 0 {
        return Err(DomainError::not_found("User not found"));
    }
    if !final_enabled {
        sqlx::query("DELETE FROM sessions WHERE user_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(DomainError::from)?;
        sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(DomainError::from)?;
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
        .map_err(DomainError::from)?;
    }
    tx.commit().await.map_err(DomainError::from)
}

pub async fn transfer_ownership(
    repo: &Repository,
    current_owner_id: i64,
    target_id: i64,
) -> DomainResult<()> {
    if current_owner_id == target_id {
        return Err(DomainError::validation_code(
            "invalid_owner",
            "Choose another administrator",
        ));
    }
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    let current: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND is_owner=1 AND enabled=1")
            .bind(current_owner_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(DomainError::from)?;
    if current.is_none() {
        return Err(DomainError::forbidden(
            "Only the current owner can transfer ownership",
        ));
    }
    let target: Option<(String, i64)> =
        sqlx::query_as("SELECT role,enabled FROM users WHERE id=$1")
            .bind(target_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(DomainError::from)?;
    match target {
        None => return Err(DomainError::not_found("User not found")),
        Some((role, 1)) if role == "admin" => {}
        Some(_) => {
            return Err(DomainError::validation_code(
                "invalid_owner",
                "The new owner must be an enabled administrator",
            ))
        }
    }
    sqlx::query("UPDATE users SET is_owner=0 WHERE id=$1")
        .bind(current_owner_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("UPDATE users SET is_owner=1 WHERE id=$1")
        .bind(target_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("UPDATE sessions SET reauthenticated_at=NULL WHERE user_id IN ($1,$2)")
        .bind(current_owner_id)
        .bind(target_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)
}

pub async fn set_password(
    repo: &Repository,
    id: i64,
    password: &str,
    force: bool,
) -> DomainResult<()> {
    let encoded = password_hash(password)?;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    sqlx::query("UPDATE users SET password_hash=$2,password_change_required=$3 WHERE id=$1")
        .bind(id)
        .bind(encoded)
        .bind(i64::from(force))
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)
}

pub async fn create_password_reset(
    repo: &Repository,
    user_id: i64,
    created_by_user_id: i64,
) -> DomainResult<String> {
    let _write_guard = repo.lock_writes().await;
    let token = random_token(64);
    let now = unix_timestamp();
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    let enabled: Option<i64> = sqlx::query_scalar("SELECT enabled FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    match enabled {
        None => return Err(DomainError::not_found("User not found")),
        Some(0) => {
            return Err(DomainError::validation_code(
                "disabled_user",
                "Disabled users cannot reset their password",
            ))
        }
        Some(_) => {}
    }
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
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
    .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(token)
}

pub async fn reset_password(repo: &Repository, token: &str, password: &str) -> DomainResult<()> {
    let token_hash = hash(token);
    let valid: Option<i64> = sqlx::query_scalar(
        "SELECT r.user_id FROM password_reset_tokens r JOIN users u ON u.id=r.user_id
         WHERE r.token_hash=$1 AND r.expires_at>$2 AND u.enabled=1",
    )
    .bind(&token_hash)
    .bind(unix_timestamp())
    .fetch_optional(repo.pool())
    .await
    .map_err(DomainError::from)?;
    if valid.is_none() {
        return Err(DomainError::validation_code(
            "invalid_password_reset",
            "Password reset link is invalid or expired",
        ));
    }
    let encoded = password_hash(password)?;
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let user_id: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT r.user_id FROM password_reset_tokens r JOIN users u ON u.id=r.user_id
         WHERE r.token_hash=$1 AND r.expires_at>$2 AND u.enabled=1{lock}"
    )))
    .bind(token_hash)
    .bind(unix_timestamp())
    .fetch_optional(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    let user_id = user_id.ok_or_else(|| {
        DomainError::validation_code(
            "invalid_password_reset",
            "Password reset link is invalid or expired",
        )
    })?;
    sqlx::query("UPDATE users SET password_hash=$2,password_change_required=0 WHERE id=$1")
        .bind(user_id)
        .bind(encoded)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    sqlx::query("DELETE FROM password_reset_tokens WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)
}

pub async fn revoke_sessions(repo: &Repository, user_id: i64) -> DomainResult<bool> {
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1")
        .bind(user_id)
        .fetch_optional(repo.pool())
        .await
        .map_err(DomainError::from)?;
    if exists.is_none() {
        return Ok(false);
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(repo.pool())
        .await
        .map(|_| true)
        .map_err(DomainError::from)
}
