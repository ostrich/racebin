use super::lifecycle::AccountState;
use super::*;

/// The owner session may manage administrator accounts; other administrators
/// may manage ordinary accounts only. Recheck this at the final database write.
#[derive(Clone, Copy, Debug)]
pub struct AdminTargetAuthority {
    owner_session_user_id: Option<i64>,
}

impl AdminTargetAuthority {
    pub fn new(owner_session_user_id: Option<i64>) -> Self {
        Self {
            owner_session_user_id,
        }
    }

    async fn require_current_owner(
        self,
        repo: &Database,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    ) -> DomainResult<()> {
        let owner_id = self.owner_session_user_id.ok_or_else(|| {
            DomainError::forbidden_code(
                "owner_required",
                "Only the owner can manage an administrator account",
            )
        })?;
        let lock = if repo.kind() == DatabaseKind::Postgres {
            " FOR UPDATE"
        } else {
            ""
        };
        let current_owner: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM users WHERE id=$1 AND is_owner=1 AND enabled=1{lock}"
        )))
        .bind(owner_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(DomainError::from)?;
        if current_owner.is_none() {
            return Err(DomainError::forbidden_code(
                "owner_required",
                "Only the current owner can manage an administrator account",
            ));
        }
        Ok(())
    }
}

pub(crate) async fn require_manageable_target(
    repo: &Database,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    target_id: i64,
    authority: AdminTargetAuthority,
) -> DomainResult<()> {
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let target: Option<(String, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT role,is_owner FROM users WHERE id=$1{lock}"
    )))
    .bind(target_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(DomainError::from)?;
    let (role, is_owner) = target.ok_or_else(|| DomainError::not_found("User not found"))?;
    if role == "admin" || is_owner != 0 {
        authority.require_current_owner(repo, tx).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AdminSummary {
    pub user_count: i64,
    pub paste_count: i64,
    pub storage_bytes: i64,
    pub active_session_count: i64,
    pub active_invitation_count: i64,
    pub expiring_invitation_count: i64,
    pub password_change_required_count: i64,
}

pub async fn admin_summary(repo: &Database) -> DomainResult<AdminSummary> {
    let text_size = if repo.kind() == DatabaseKind::Postgres {
        "CAST(octet_length(p.content) AS BIGINT)"
    } else {
        "length(CAST(p.content AS BLOB))"
    };
    let now = unix_timestamp();
    // One statement gives the overview one snapshot without seven round trips
    // or monopolizing seven pool connections for independent aggregates.
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT
          (SELECT COUNT(*) FROM users) AS user_count,
          (SELECT COUNT(*) FROM pastes WHERE creation_state='complete') AS paste_count,
          (SELECT CAST(COALESCE(SUM({text_size} + COALESCE((SELECT SUM(a.size_bytes) FROM attachments a WHERE a.paste_id=p.id),0)),0) AS BIGINT)
             FROM pastes p WHERE p.creation_state='complete') AS storage_bytes,
          (SELECT COUNT(*) FROM sessions WHERE expires_at>$1) AS active_session_count,
          (SELECT COUNT(*) FROM invitations WHERE redeemed=0 AND revoked=0 AND expires_at>$1) AS active_invitation_count,
          (SELECT COUNT(*) FROM invitations WHERE redeemed=0 AND revoked=0 AND expires_at>$1 AND expires_at<=$2) AS expiring_invitation_count,
          (SELECT COUNT(*) FROM users WHERE password_change_required=1) AS password_change_required_count"
    )))
    .bind(now)
    .bind(now + 4 * 60 * 60)
    .fetch_one(repo.pool())
    .await
    .map_err(DomainError::from)
}

fn admin_user_query(repo: &Database) -> String {
    let text_size = if repo.kind() == DatabaseKind::Postgres {
        "CAST(octet_length(p.content) AS BIGINT)"
    } else {
        "length(CAST(p.content AS BLOB))"
    };
    format!(
        "SELECT u.id,u.username,u.role,u.is_owner,u.enabled,u.password_change_required,u.created_at,u.last_login_at,
          CAST((SELECT count(*) FROM pastes p WHERE p.owner_id=u.id AND p.creation_state='complete') AS BIGINT) AS paste_count,
          CAST(COALESCE((SELECT sum({text_size} + COALESCE((SELECT sum(a.size_bytes) FROM attachments a WHERE a.paste_id=p.id),0)) FROM pastes p WHERE p.owner_id=u.id AND p.creation_state='complete'),0) AS BIGINT) AS storage_bytes,
          CAST((SELECT count(*) FROM sessions s WHERE s.user_id=u.id AND s.expires_at>$1) AS BIGINT) AS active_session_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id) AS BIGINT) AS api_key_count,
          CAST((SELECT count(*) FROM api_keys k WHERE k.user_id=u.id AND k.enabled=1 AND u.enabled=1) AS BIGINT) AS active_api_key_count
         FROM users u"
    )
}

pub struct AdminUserListQuery<'a> {
    pub search: Option<&'a str>,
    pub role: Option<&'a str>,
    pub enabled: Option<bool>,
    pub sort: &'a str,
    pub descending: bool,
    pub page: u32,
    pub page_size: u32,
}

pub async fn list_admin_users(
    repo: &Database,
    query: &AdminUserListQuery<'_>,
) -> DomainResult<crate::pastes::Page<AdminUser>> {
    let search = query
        .search
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::database::literal_like_pattern(&value.trim().to_lowercase()));
    let search_value = search.as_deref().unwrap_or("");
    let role_value = query.role.unwrap_or("");
    let enabled_value = query.enabled.map_or(-1, i64::from);
    let count_where = " WHERE ($1='' OR LOWER(u.username) LIKE $1 ESCAPE '\\')
        AND ($2='' OR ($2='owner' AND u.is_owner=1) OR ($2<>'owner' AND u.role=$2 AND u.is_owner=0))
        AND ($3=-1 OR u.enabled=$3)";
    let list_where = " WHERE ($2='' OR LOWER(u.username) LIKE $2 ESCAPE '\\')
        AND ($3='' OR ($3='owner' AND u.is_owner=1) OR ($3<>'owner' AND u.role=$3 AND u.is_owner=0))
        AND ($4=-1 OR u.enabled=$4)";
    let sort_field = match query.sort {
        "created" => "u.created_at",
        "login" => "COALESCE(u.last_login_at,0)",
        "pastes" => "paste_count",
        "storage" => "storage_bytes",
        _ => "LOWER(u.username)",
    };
    let direction = if query.descending { "DESC" } else { "ASC" };

    let total_items = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM users u{count_where}"
    )))
    .bind(search_value)
    .bind(role_value)
    .bind(enabled_value)
    .fetch_one(repo.pool());

    let sql = format!(
        "{}{list_where} ORDER BY {sort_field} {direction},u.id {direction} LIMIT $5 OFFSET $6",
        admin_user_query(repo)
    );
    let items = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(unix_timestamp())
        .bind(search_value)
        .bind(role_value)
        .bind(enabled_value)
        .bind(i64::from(query.page_size))
        .bind(i64::from(query.page.saturating_sub(1)) * i64::from(query.page_size))
        .fetch_all(repo.pool());
    let (total_items, items) = futures::try_join!(total_items, items).map_err(DomainError::from)?;
    Ok(crate::pastes::Page {
        items,
        page: query.page,
        page_size: query.page_size,
        total_items,
    })
}

pub async fn admin_user(repo: &Database, id: i64) -> DomainResult<Option<AdminUser>> {
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

pub async fn set_enabled(repo: &Database, id: i64, enabled: bool) -> DomainResult<()> {
    update_user(repo, id, Some(enabled), None).await
}

pub async fn set_role(repo: &Database, id: i64, admin: bool) -> DomainResult<()> {
    update_user(repo, id, None, Some(admin)).await
}

pub async fn update_user(
    repo: &Database,
    id: i64,
    enabled: Option<bool>,
    admin: Option<bool>,
) -> DomainResult<()> {
    update_user_with_authority(repo, id, enabled, admin, None).await
}

pub async fn update_user_managed(
    repo: &Database,
    id: i64,
    enabled: Option<bool>,
    admin: Option<bool>,
    authority: AdminTargetAuthority,
) -> DomainResult<()> {
    update_user_with_authority(repo, id, enabled, admin, Some(authority)).await
}

async fn update_user_with_authority(
    repo: &Database,
    id: i64,
    enabled: Option<bool>,
    admin: Option<bool>,
    authority: Option<AdminTargetAuthority>,
) -> DomainResult<()> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    if let Some(authority) = authority {
        require_manageable_target(repo, &mut tx, id, authority).await?;
        if admin.is_some() {
            authority.require_current_owner(repo, &mut tx).await?;
        }
    }
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
    let current = AccountState::stored(&current_role, currently_enabled != 0, is_owner != 0)?;
    let next = current.update(enabled, admin)?;
    if current.enabled_admin() && !next.enabled_admin() {
        let admins: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
                .fetch_one(&mut *tx)
                .await
                .map_err(DomainError::from)?;
        next.require_successor(admins)?;
    }
    let result = sqlx::query("UPDATE users SET enabled=$2,role=$3 WHERE id=$1")
        .bind(id)
        .bind(i64::from(next.enabled()))
        .bind(if next.admin() { "admin" } else { "user" })
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    if result.rows_affected() == 0 {
        return Err(DomainError::not_found("User not found"));
    }
    if !next.enabled() {
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
    if !next.admin() {
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
    repo: &Database,
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
        Some((role, enabled)) => {
            let state = AccountState::stored(&role, enabled != 0, false)?;
            if !state.enabled_admin() {
                return Err(DomainError::validation_code(
                    "invalid_owner",
                    "The new owner must be an enabled administrator",
                ));
            }
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
    repo: &Database,
    id: i64,
    password: &str,
    force: bool,
) -> DomainResult<()> {
    let encoded = password_hash(password)?;
    replace_password(repo, id, &encoded, force, None).await
}

pub async fn set_verified_password(
    repo: &Database,
    verified: &VerifiedUser,
    password: &str,
) -> DomainResult<()> {
    let encoded = password_hash(password)?;
    replace_password(repo, verified.id, &encoded, false, Some(verified)).await
}

async fn replace_password(
    repo: &Database,
    id: i64,
    encoded: &str,
    force: bool,
    verified: Option<&VerifiedUser>,
) -> DomainResult<()> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    if let Some(verified) = verified {
        verified.check(repo, &mut tx).await?;
    }
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
    repo: &Database,
    user_id: i64,
    created_by_user_id: i64,
) -> DomainResult<String> {
    create_password_reset_with_authority(repo, user_id, created_by_user_id, None).await
}

pub async fn create_password_reset_managed(
    repo: &Database,
    user_id: i64,
    created_by_user_id: i64,
    authority: AdminTargetAuthority,
) -> DomainResult<String> {
    create_password_reset_with_authority(repo, user_id, created_by_user_id, Some(authority)).await
}

async fn create_password_reset_with_authority(
    repo: &Database,
    user_id: i64,
    created_by_user_id: i64,
    authority: Option<AdminTargetAuthority>,
) -> DomainResult<String> {
    let _write_guard = repo.lock_writes().await;
    let token = random_token(64);
    let now = unix_timestamp();
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    if let Some(authority) = authority {
        require_manageable_target(repo, &mut tx, user_id, authority).await?;
    }
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

pub async fn reset_password(repo: &Database, token: &str, password: &str) -> DomainResult<()> {
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

pub async fn revoke_sessions(repo: &Database, user_id: i64) -> DomainResult<bool> {
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

pub async fn revoke_sessions_managed(
    repo: &Database,
    user_id: i64,
    authority: AdminTargetAuthority,
) -> DomainResult<bool> {
    let _write_guard = repo.lock_writes().await;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    require_manageable_target(repo, &mut tx, user_id, authority).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(true)
}
