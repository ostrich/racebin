use super::*;

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
