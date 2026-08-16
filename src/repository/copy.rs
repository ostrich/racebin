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
          (SELECT count(*) FROM folders) +
          (SELECT count(*) FROM sessions) +
          (SELECT count(*) FROM password_reset_tokens) +
          (SELECT count(*) FROM invitations) +
          (SELECT count(*) FROM api_keys) +
          (SELECT count(*) FROM api_key_scopes) +
          (SELECT count(*) FROM pastes) +
          (SELECT count(*) FROM attachments) +
          (SELECT count(*) FROM instance_settings) +
          (SELECT count(*) FROM audit_events)",
    )
    .fetch_one(destination.pool())
    .await
    .map_err(|e| e.to_string())?;
    if occupied != 0 {
        return Err("destination database is not empty".to_string());
    }

    let mut source_tx = source.pool.begin().await.map_err(|e| e.to_string())?;
    let users = sqlx::query(
        "SELECT id,username,password_hash,role,is_owner,enabled,password_change_required,created_at,last_login_at FROM users",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let folders =
        sqlx::query("SELECT id,owner_id,name,name_key,created_at FROM folders ORDER BY id")
            .fetch_all(&mut *source_tx)
            .await
            .map_err(|e| e.to_string())?;
    let sessions = sqlx::query(
        "SELECT id,user_id,token_hash,csrf_token,created_at,expires_at,last_used_at,reauthenticated_at FROM sessions",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let password_resets = sqlx::query(
        "SELECT user_id,token_hash,created_by_user_id,created_at,expires_at FROM password_reset_tokens",
    )
    .fetch_all(&mut *source_tx)
    .await
    .map_err(|e| e.to_string())?;
    let invitations = sqlx::query(
        "SELECT id,token_hash,token,created_by_user_id,expires_at,redeemed,redeemed_by_user_id,revoked
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
        "SELECT id,owner_id,folder_id,title,content,content_kind,language,visibility,
                created_at,updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit
         FROM pastes",
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
    let settings = sqlx::query("SELECT site_name,home_mode,public_explore_enabled,invitations_enabled,attachments_enabled,qr_codes_enabled,default_format,default_language,default_visibility,default_expiration_seconds,updated_at,updated_by_user_id FROM instance_settings WHERE id=1")
        .fetch_all(&mut *source_tx).await.map_err(|e| e.to_string())?;
    let audit_events = sqlx::query("SELECT id,actor_user_id,actor_username,actor_api_key_id,action,target_type,target_id,target_label,details,created_at FROM audit_events ORDER BY id")
        .fetch_all(&mut *source_tx).await.map_err(|e| e.to_string())?;

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
        folders.len(),
        sessions.len(),
        password_resets.len(),
        invitations.len(),
        api_keys.len(),
        api_key_scopes.len(),
        pastes.len(),
        attachments.len(),
        settings.len(),
        audit_events.len(),
    ];
    let mut tx = destination.pool.begin().await.map_err(|e| e.to_string())?;
    for row in users {
        sqlx::query(
            "INSERT INTO users(id,username,password_hash,role,is_owner,enabled,password_change_required,created_at,last_login_at)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("username")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<String, _>("password_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("role").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("is_owner").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("enabled").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("password_change_required")
                .map_err(|e| e.to_string())?,
        )
        .bind(row.try_get::<i64, _>("created_at").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<Option<i64>, _>("last_login_at")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in folders {
        sqlx::query(
            "INSERT INTO folders(id,owner_id,name,name_key,created_at) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<i64, _>("owner_id")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("name")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<String, _>("name_key")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<i64, _>("created_at")
                .map_err(|e| e.to_string())?,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in sessions {
        sqlx::query(
            "INSERT INTO sessions(id,user_id,token_hash,csrf_token,created_at,expires_at,last_used_at,reauthenticated_at)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
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
        .bind(row.try_get::<Option<i64>, _>("reauthenticated_at").map_err(|e| e.to_string())?)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    for row in password_resets {
        sqlx::query(
            "INSERT INTO password_reset_tokens(user_id,token_hash,created_by_user_id,created_at,expires_at)
             VALUES($1,$2,$3,$4,$5)",
        )
        .bind(row.try_get::<i64, _>("user_id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<String, _>("token_hash").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("created_by_user_id").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("created_at").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("expires_at").map_err(|e| e.to_string())?)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    for row in invitations {
        sqlx::query(
            "INSERT INTO invitations(
                id,token_hash,token,created_by_user_id,expires_at,redeemed,redeemed_by_user_id,revoked
             ) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(row.try_get::<i64, _>("id").map_err(|e| e.to_string())?)
        .bind(
            row.try_get::<String, _>("token_hash")
                .map_err(|e| e.to_string())?,
        )
        .bind(
            row.try_get::<Option<String>, _>("token")
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
            "INSERT INTO pastes(id,owner_id,folder_id,title,content,content_kind,language,visibility,
                               created_at,updated_at,revision,consumed_at,expires_at,last_read_at,read_count,read_limit)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
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
            row.try_get::<Option<i64>, _>("folder_id")
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
        .bind(row.try_get::<i64, _>("updated_at").map_err(|e| e.to_string())?)
        .bind(row.try_get::<i64, _>("revision").map_err(|e| e.to_string())?)
        .bind(row.try_get::<Option<i64>, _>("consumed_at").map_err(|e| e.to_string())?)
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
    for row in settings {
        sqlx::query("INSERT INTO instance_settings(id,site_name,home_mode,public_explore_enabled,invitations_enabled,attachments_enabled,qr_codes_enabled,default_format,default_language,default_visibility,default_expiration_seconds,updated_at,updated_by_user_id) VALUES(1,$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
            .bind(row.try_get::<String,_>("site_name").map_err(|e|e.to_string())?).bind(row.try_get::<String,_>("home_mode").map_err(|e|e.to_string())?)
            .bind(row.try_get::<i64,_>("public_explore_enabled").map_err(|e|e.to_string())?).bind(row.try_get::<i64,_>("invitations_enabled").map_err(|e|e.to_string())?)
            .bind(row.try_get::<i64,_>("attachments_enabled").map_err(|e|e.to_string())?).bind(row.try_get::<i64,_>("qr_codes_enabled").map_err(|e|e.to_string())?)
            .bind(row.try_get::<String,_>("default_format").map_err(|e|e.to_string())?).bind(row.try_get::<String,_>("default_language").map_err(|e|e.to_string())?)
            .bind(row.try_get::<String,_>("default_visibility").map_err(|e|e.to_string())?).bind(row.try_get::<Option<i64>,_>("default_expiration_seconds").map_err(|e|e.to_string())?)
            .bind(row.try_get::<i64,_>("updated_at").map_err(|e|e.to_string())?).bind(row.try_get::<Option<i64>,_>("updated_by_user_id").map_err(|e|e.to_string())?)
            .execute(&mut *tx).await.map_err(|e|e.to_string())?;
    }
    for row in audit_events {
        sqlx::query("INSERT INTO audit_events(id,actor_user_id,actor_username,actor_api_key_id,action,target_type,target_id,target_label,details,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(row.try_get::<i64,_>("id").map_err(|e|e.to_string())?).bind(row.try_get::<Option<i64>,_>("actor_user_id").map_err(|e|e.to_string())?)
            .bind(row.try_get::<String,_>("actor_username").map_err(|e|e.to_string())?).bind(row.try_get::<Option<i64>,_>("actor_api_key_id").map_err(|e|e.to_string())?)
            .bind(row.try_get::<String,_>("action").map_err(|e|e.to_string())?).bind(row.try_get::<String,_>("target_type").map_err(|e|e.to_string())?)
            .bind(row.try_get::<Option<String>,_>("target_id").map_err(|e|e.to_string())?).bind(row.try_get::<Option<String>,_>("target_label").map_err(|e|e.to_string())?)
            .bind(row.try_get::<String,_>("details").map_err(|e|e.to_string())?).bind(row.try_get::<i64,_>("created_at").map_err(|e|e.to_string())?)
            .execute(&mut *tx).await.map_err(|e|e.to_string())?;
    }
    if destination.kind == DatabaseKind::Postgres {
        for table in [
            "users",
            "folders",
            "sessions",
            "invitations",
            "api_keys",
            "attachments",
            "audit_events",
        ] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT setval(pg_get_serial_sequence('{table}','id'),
                               coalesce((SELECT max(id) FROM {table}),1),
                               EXISTS(SELECT 1 FROM {table}))"
            )))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    for (table, expected) in [
        ("users", counts[0]),
        ("folders", counts[1]),
        ("sessions", counts[2]),
        ("password_reset_tokens", counts[3]),
        ("invitations", counts[4]),
        ("api_keys", counts[5]),
        ("api_key_scopes", counts[6]),
        ("pastes", counts[7]),
        ("attachments", counts[8]),
        ("instance_settings", counts[9]),
        ("audit_events", counts[10]),
    ] {
        let actual: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
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
