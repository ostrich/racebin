use super::{DatabaseKind, Repository};
use sqlx::Row;
use std::path::Path;

pub async fn copy_database(
    source_url: &str,
    destination_url: &str,
    data_dir: impl AsRef<Path>,
) -> Result<(), String> {
    let data_dir = data_dir.as_ref();
    let source = Repository::open(source_url, data_dir).await?;
    let destination = Repository::open(destination_url, data_dir).await?;
    if source.kind == destination.kind && source_url == destination_url {
        return Err("source and destination databases must differ".to_string());
    }
    source.migrate().await?;
    destination.migrate().await?;
    let occupied: i64 = sqlx::query_scalar(
        "SELECT
          (SELECT count(*) FROM app_user) +
          (SELECT count(*) FROM user_session) +
          (SELECT count(*) FROM user_invite) +
          (SELECT count(*) FROM api_key) +
          (SELECT count(*) FROM pasta) +
          (SELECT count(*) FROM pasta_file)",
    )
    .fetch_one(destination.pool())
    .await
    .map_err(|e| e.to_string())?;
    if occupied != 0 {
        return Err("destination database is not empty".to_string());
    }

    let mut source_tx = source.pool.begin().await.map_err(|e| e.to_string())?;
    let users = sqlx::query(
        "SELECT id,username,password_hash,role,enabled,force_password_change,created FROM app_user",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let sessions = sqlx::query(
        "SELECT id,user_id,token_hash,csrf_token,created,expires,last_used FROM user_session",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let invites =
        sqlx::query("SELECT id,token_hash,created_by,expires,used,revoked FROM user_invite")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|e| e.to_string())?;
    let keys = sqlx::query(
        "SELECT id,user_id,name,prefix,token_hash,scopes,created,last_used,enabled FROM api_key",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let pastes = sqlx::query(
        "SELECT id,slug,owner_user_id,title,content,kind,syntax,access,created,expiration,
                last_read,read_count,burn_after_reads FROM pasta",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let files =
        sqlx::query("SELECT id,pasta_id,position,role,name,storage_name,size FROM pasta_file")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|e| e.to_string())?;

    for row in &files {
        let pasta_id: i64 = row.try_get("pasta_id").map_err(|e| e.to_string())?;
        let storage_name: String = row.try_get("storage_name").map_err(|e| e.to_string())?;
        let slug = pastes
            .iter()
            .find_map(|paste| {
                (paste.try_get::<i64, _>("id").ok() == Some(pasta_id))
                    .then(|| paste.try_get::<String, _>("slug").ok())
                    .flatten()
            })
            .ok_or_else(|| format!("file {storage_name:?} references missing paste {pasta_id}"))?;
        if !data_dir
            .join("attachments")
            .join(&slug)
            .join(&storage_name)
            .is_file()
        {
            return Err(format!(
                "attachment {storage_name:?} for paste {slug:?} is missing from data-dir"
            ));
        }
    }

    let counts = [
        users.len(),
        sessions.len(),
        invites.len(),
        keys.len(),
        pastes.len(),
        files.len(),
    ];
    let mut tx = destination.pool.begin().await.map_err(|e| e.to_string())?;
    for row in users {
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("username").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("password_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("role").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("force_password_change")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("created").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in sessions {
        sqlx::query(
            "INSERT INTO user_session(id,user_id,token_hash,csrf_token,created,expires,last_used)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("csrf_token")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("last_used")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in invites {
        sqlx::query(
            "INSERT INTO user_invite(id,token_hash,created_by,expires,used,revoked)
             VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created_by")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("used").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("revoked")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in keys {
        sqlx::query(
            "INSERT INTO api_key(id,user_id,name,prefix,token_hash,scopes,created,last_used,enabled)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("user_id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("name").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("prefix").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("token_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("scopes").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("created").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("last_used").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in pastes {
        sqlx::query(
            "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,created,
                               expiration,last_read,read_count,burn_after_reads)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("slug")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("owner_user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("title")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("content")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("kind")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("syntax")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("access")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("expiration")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("last_read")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("read_count")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("burn_after_reads")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in files {
        sqlx::query(
            "INSERT INTO pasta_file(id,pasta_id,position,role,name,storage_name,size)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("pasta_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("position")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("role")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("name")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("storage_name")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("size").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    if destination.kind == DatabaseKind::Postgres {
        for table in [
            "app_user",
            "user_session",
            "user_invite",
            "api_key",
            "pasta",
            "pasta_file",
        ] {
            sqlx::query(&format!(
                "SELECT setval(pg_get_serial_sequence('{table}','id'),
                               coalesce((SELECT max(id) FROM {table}),1),
                               EXISTS(SELECT 1 FROM {table}))"
            ))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    for (table, expected) in [
        ("app_user", counts[0]),
        ("user_session", counts[1]),
        ("user_invite", counts[2]),
        ("api_key", counts[3]),
        ("pasta", counts[4]),
        ("pasta_file", counts[5]),
    ] {
        let actual: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if actual != expected as i64 {
            return Err(format!(
                "copy verification failed for {table}: expected {expected}, got {actual}"
            ));
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    source_tx.rollback().await.map_err(|e| e.to_string())?;
    Ok(())
}
