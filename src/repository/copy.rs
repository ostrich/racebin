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
          (SELECT count(*) FROM users) +
          (SELECT count(*) FROM sessions) +
          (SELECT count(*) FROM invitations) +
          (SELECT count(*) FROM api_keys) +
          (SELECT count(*) FROM api_key_scopes) +
          (SELECT count(*) FROM pastes) +
          (SELECT count(*) FROM attachments)",
    )
    .fetch_one(destination.pool())
    .await
    .map_err(|e| e.to_string())?;
    if occupied != 0 {
        return Err("destination database is not empty".to_string());
    }

    let mut source_tx = source.pool.begin().await.map_err(|e| e.to_string())?;
    let users = sqlx::query(
        "SELECT id,username,password_hash,role,enabled,password_change_required,created_at FROM users",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let sessions = sqlx::query(
        "SELECT id,user_id,token_hash,csrf_token,created_at,expires_at,last_used_at FROM sessions",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let invitations = sqlx::query(
        "SELECT id,token_hash,created_by_user_id,expires_at,redeemed,redeemed_by_user_id,revoked
         FROM invitations",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let api_keys = sqlx::query(
        "SELECT id,user_id,name,token_prefix,token_hash,created_at,last_used_at,enabled FROM api_keys",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let api_key_scopes =
        sqlx::query("SELECT api_key_id,scope FROM api_key_scopes ORDER BY api_key_id,scope")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|error| error.to_string())?;
    let pastes = sqlx::query(
        "SELECT id,owner_id,title,content,document_json,content_kind,language,visibility,created_at,expires_at,
                last_read_at,read_count,read_limit FROM pastes",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let attachments = sqlx::query(
        "SELECT id,paste_id,sort_order,filename,storage_key,size_bytes FROM attachments",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;

    for row in &attachments {
        let paste_id: String = row.try_get("paste_id").map_err(|e| e.to_string())?;
        let storage_key: String = row.try_get("storage_key").map_err(|e| e.to_string())?;
        if !pastes
            .iter()
            .any(|paste| paste.try_get::<String, _>("id").ok().as_ref() == Some(&paste_id))
        {
            return Err(format!(
                "attachment {storage_key:?} references missing paste {paste_id}"
            ));
        }
        if !data_dir
            .join("attachments")
            .join(&paste_id)
            .join(&storage_key)
            .is_file()
        {
            return Err(format!(
                "attachment {storage_key:?} for paste {paste_id:?} is missing from data-dir"
            ));
        }
    }

    let counts = [
        users.len(),
        sessions.len(),
        invitations.len(),
        api_keys.len(),
        api_key_scopes.len(),
        pastes.len(),
        attachments.len(),
    ];
    let mut tx = destination.pool.begin().await.map_err(|e| e.to_string())?;
    for row in users {
        sqlx::query(
            "INSERT INTO users(id,username,password_hash,role,enabled,password_change_required,created_at)
             VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("username").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("password_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("role").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("password_change_required")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("created_at").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in sessions {
        sqlx::query(
            "INSERT INTO sessions(id,user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
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
            row.try_get::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("last_used_at")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in invitations {
        sqlx::query(
            "INSERT INTO invitations(
                id,token_hash,created_by_user_id,expires_at,redeemed,redeemed_by_user_id,revoked
             ) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created_by_user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("expires_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("redeemed")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("redeemed_by_user_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("revoked")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in api_keys {
        sqlx::query(
            "INSERT INTO api_keys(id,user_id,name,token_prefix,token_hash,created_at,last_used_at,enabled)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("user_id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("name").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("token_prefix").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("token_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("created_at").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("last_used_at").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in api_key_scopes {
        sqlx::query("INSERT INTO api_key_scopes(api_key_id,scope) VALUES($1,$2)")
            .bind(
                row.try_get::<i64, _>("api_key_id")
                    .map_err(|error| error.to_string())?,
            )
            .bind(
                row.try_get::<String, _>("scope")
                    .map_err(|error| error.to_string())?,
            )
            .execute(&mut *tx)
            .await
            .map_err(|error| error.to_string())?;
    }
    for row in pastes {
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,title,content,document_json,content_kind,language,visibility,
                               created_at,expires_at,last_read_at,read_count,read_limit)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(
            row.try_get::<String, _>("id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("owner_id")
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
            row.try_get::<Option<String>, _>("document_json")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("content_kind")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("language")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("visibility")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("expires_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("last_read_at")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("read_count")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<i64>, _>("read_limit")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in attachments {
        sqlx::query(
            "INSERT INTO attachments(id,paste_id,sort_order,filename,storage_key,size_bytes)
             VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("paste_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("sort_order")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("filename")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("storage_key")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("size_bytes")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    if destination.kind == DatabaseKind::Postgres {
        for table in [
            "users",
            "sessions",
            "invitations",
            "api_keys",
            "attachments",
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
        ("users", counts[0]),
        ("sessions", counts[1]),
        ("invitations", counts[2]),
        ("api_keys", counts[3]),
        ("api_key_scopes", counts[4]),
        ("pastes", counts[5]),
        ("attachments", counts[6]),
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
