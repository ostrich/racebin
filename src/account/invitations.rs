use super::*;

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
    .map_err(|e| e.to_string())?;
    if active.is_none() {
        return Err(DomainError::validation_code(
            "invalid_invitation",
            "Invitation is invalid or expired",
        ));
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
