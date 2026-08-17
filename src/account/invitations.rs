use super::*;

pub async fn create_invitation(
    repo: &Repository,
    created_by_user_id: i64,
    comment: Option<&str>,
) -> DomainResult<String> {
    let token = random_token(64);
    let now = unix_timestamp();
    sqlx::query(
        "INSERT INTO invitations(token_hash,token,created_by_user_id,comment,created_at,expires_at)
         VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(hash(&token))
    .bind(&token)
    .bind(created_by_user_id)
    .bind(comment)
    .bind(now)
    .bind(now + 86400)
    .execute(repo.pool())
    .await
    .map_err(DomainError::from)?;
    Ok(token)
}

pub async fn list_invitations(
    repo: &Repository,
    history: bool,
    search: Option<&str>,
    status: Option<&str>,
    page: u32,
    page_size: u32,
) -> DomainResult<crate::services::Page<Invitation>> {
    let now = unix_timestamp();
    let lifecycle = if history {
        "(i.redeemed=1 OR i.revoked=1 OR i.expires_at<=$1)"
    } else {
        "(i.redeemed=0 AND i.revoked=0 AND i.expires_at>$1)"
    };
    let status_clause = match status {
        Some("redeemed") => " AND i.redeemed=1",
        Some("revoked") => " AND i.revoked=1 AND i.redeemed=0",
        Some("expired") => " AND i.redeemed=0 AND i.revoked=0 AND i.expires_at<=$1",
        _ => "",
    };
    let search = search
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{}%", value.trim().to_lowercase()));
    let search_clause = if search.is_some() {
        " AND (LOWER(COALESCE(i.comment,'')) LIKE $2 OR LOWER(c.username) LIKE $2 OR LOWER(COALESCE(r.username,'')) LIKE $2 OR LOWER(COALESCE(i.token, i.token_hash)) LIKE $2)"
    } else {
        ""
    };
    let count_sql = sqlx::AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM invitations i JOIN users c ON c.id=i.created_by_user_id LEFT JOIN users r ON r.id=i.redeemed_by_user_id WHERE {lifecycle}{status_clause}{search_clause}"
    ));
    let mut count = sqlx::query_scalar::<_, i64>(count_sql).bind(now);
    if let Some(search) = &search {
        count = count.bind(search);
    }
    let total_items = count
        .fetch_one(repo.pool())
        .await
        .map_err(DomainError::from)?;
    let limit_parameter = if search.is_some() { "$3" } else { "$2" };
    let offset_parameter = if search.is_some() { "$4" } else { "$3" };
    let query_sql = sqlx::AssertSqlSafe(format!(
        "SELECT i.id,COALESCE(substr(i.token,1,10),substr(i.token_hash,1,10)) AS token_prefix,
                i.token,i.comment,i.created_at,c.username AS created_by_username,
                i.expires_at,i.redeemed,i.redeemed_at,
                r.username AS redeemed_by_username,i.revoked
         FROM invitations i
         JOIN users c ON c.id=i.created_by_user_id
         LEFT JOIN users r ON r.id=i.redeemed_by_user_id
         WHERE {lifecycle}{status_clause}{search_clause}
         ORDER BY i.id DESC LIMIT {limit_parameter} OFFSET {offset_parameter}"
    ));
    let mut query = sqlx::query_as(query_sql).bind(now);
    if let Some(search) = &search {
        query = query.bind(search);
    }
    let items = query
        .bind(i64::from(page_size))
        .bind(i64::from(page.saturating_sub(1)) * i64::from(page_size))
        .fetch_all(repo.pool())
        .await
        .map_err(DomainError::from)?;
    Ok(crate::services::Page {
        items,
        page,
        page_size,
        total_items,
    })
}

pub async fn revoke_invitation(repo: &Repository, id: i64) -> DomainResult<bool> {
    sqlx::query("UPDATE invitations SET revoked=1,token=NULL WHERE id=$1 AND redeemed=0")
        .bind(id)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn update_invitation_comment(
    repo: &Repository,
    id: i64,
    comment: Option<&str>,
) -> DomainResult<bool> {
    sqlx::query("UPDATE invitations SET comment=$2 WHERE id=$1")
        .bind(id)
        .bind(comment)
        .execute(repo.pool())
        .await
        .map(|result| result.rows_affected() == 1)
        .map_err(DomainError::from)
}

pub async fn redeem_invitation(
    repo: &Repository,
    token: &str,
    username: &str,
    password: &str,
) -> DomainResult<User> {
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
    .map_err(DomainError::from)?;
    if active.is_none() {
        return Err(DomainError::validation_code(
            "invalid_invitation",
            "Invitation is invalid or expired",
        ));
    }
    let encoded = password_hash(password)?;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    let lock = if repo.kind() == DatabaseKind::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let invitation_id: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM invitations
         WHERE token_hash=$1 AND expires_at>$2 AND redeemed=0 AND revoked=0{lock}"
    )))
    .bind(token_hash)
    .bind(unix_timestamp())
    .fetch_optional(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    let invitation_id = invitation_id.ok_or_else(|| {
        DomainError::validation_code("invalid_invitation", "Invitation is invalid or expired")
    })?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users(username,password_hash,role,password_change_required,created_at)
         VALUES($1,$2,'user',0,$3) RETURNING id",
    )
    .bind(&username)
    .bind(encoded)
    .bind(unix_timestamp())
    .fetch_one(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    sqlx::query("UPDATE invitations SET redeemed=1,redeemed_by_user_id=$2,redeemed_at=$3,token=NULL WHERE id=$1")
        .bind(invitation_id)
        .bind(id)
        .bind(unix_timestamp())
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(User {
        id,
        username,
        role: "user".to_string(),
        is_owner: false,
        enabled: true,
        password_change_required: false,
    })
}
