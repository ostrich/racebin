use super::*;

const ATTEMPT_WINDOW_SECONDS: i64 = 900;

async fn retry_after(repo: &Database, keys: &[(&str, String, i64)]) -> DomainResult<Option<u64>> {
    let now = unix_timestamp();
    let cutoff = now - ATTEMPT_WINDOW_SECONDS;
    sqlx::query("DELETE FROM auth_attempts WHERE occurred_at<=$1")
        .bind(cutoff)
        .execute(repo.pool())
        .await
        .map_err(DomainError::from)?;
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
        .map_err(DomainError::from)?;
        if count >= *limit {
            let seconds = (first.unwrap_or(now) + ATTEMPT_WINDOW_SECONDS - now).max(1) as u64;
            retry = Some(retry.map_or(seconds, |current: u64| current.max(seconds)));
        }
    }
    Ok(retry)
}

pub async fn reserve_login_attempt(
    repo: &Database,
    username: &str,
    client: &str,
) -> DomainResult<Option<u64>> {
    let now = unix_timestamp();
    let cutoff = now - ATTEMPT_WINDOW_SECONDS;
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    for (kind, subject, limit) in [
        ("login_account", username.to_ascii_lowercase(), 5_i64),
        ("login_address", client.to_string(), 20_i64),
    ] {
        let (started, count): (i64, i64) = sqlx::query_as(
            "INSERT INTO auth_buckets(kind,subject,window_started_at,attempt_count)
             VALUES($1,$2,$3,1)
             ON CONFLICT(kind,subject) DO UPDATE SET
               window_started_at=CASE WHEN auth_buckets.window_started_at<=$4 THEN $3 ELSE auth_buckets.window_started_at END,
               attempt_count=CASE WHEN auth_buckets.window_started_at<=$4 THEN 1 ELSE auth_buckets.attempt_count+1 END
             RETURNING window_started_at,attempt_count",
        )
        .bind(kind)
        .bind(subject)
        .bind(now)
        .bind(cutoff)
        .fetch_one(&mut *tx)
        .await
        .map_err(DomainError::from)?;
        if count > limit {
            tx.rollback().await.map_err(DomainError::from)?;
            return Ok(Some((started + ATTEMPT_WINDOW_SECONDS - now).max(1) as u64));
        }
    }
    tx.commit().await.map_err(DomainError::from)?;
    Ok(None)
}

pub async fn release_login_attempt(
    repo: &Database,
    username: &str,
    client: &str,
) -> DomainResult<()> {
    let mut tx = repo.pool().begin().await.map_err(DomainError::from)?;
    for (kind, subject) in [
        ("login_account", username.to_ascii_lowercase()),
        ("login_address", client.to_string()),
    ] {
        sqlx::query(
            "UPDATE auth_buckets SET attempt_count=attempt_count-1
             WHERE kind=$1 AND subject=$2 AND attempt_count>0",
        )
        .bind(kind)
        .bind(subject)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    }
    tx.commit().await.map_err(DomainError::from)
}

pub async fn clear_login_failures(repo: &Database, username: &str) -> DomainResult<()> {
    sqlx::query("DELETE FROM auth_buckets WHERE kind='login_account' AND subject=$1")
        .bind(username.to_ascii_lowercase())
        .execute(repo.pool())
        .await
        .map(|_| ())
        .map_err(DomainError::from)
}

pub async fn invitation_retry_after(repo: &Database, client: &str) -> DomainResult<Option<u64>> {
    retry_after(repo, &[("invitation_address", client.to_string(), 20)]).await
}

pub async fn record_invitation_failure(repo: &Database, client: &str) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO auth_attempts(kind,subject,occurred_at)
         VALUES('invitation_address',$1,$2)",
    )
    .bind(client)
    .bind(unix_timestamp())
    .execute(repo.pool())
    .await
    .map(|_| ())
    .map_err(DomainError::from)
}

pub async fn password_reset_retry_after(
    repo: &Database,
    client: &str,
) -> DomainResult<Option<u64>> {
    retry_after(repo, &[("password_reset_address", client.to_string(), 20)]).await
}

pub async fn record_password_reset_failure(repo: &Database, client: &str) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO auth_attempts(kind,subject,occurred_at)
         VALUES('password_reset_address',$1,$2)",
    )
    .bind(client)
    .bind(unix_timestamp())
    .execute(repo.pool())
    .await
    .map(|_| ())
    .map_err(DomainError::from)
}
