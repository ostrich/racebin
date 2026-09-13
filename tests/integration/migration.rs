use super::*;

const SQLITE_MIGRATIONS_BEFORE_PENDING_PASTES: &[&str] = &[
    include_str!("../../migrations/sqlite/0001_schema.sql"),
    include_str!("../../migrations/sqlite/0002_rich_text.sql"),
    include_str!("../../migrations/sqlite/0003_invitation_redeemer.sql"),
    include_str!("../../migrations/sqlite/0004_remove_redirect_pastes.sql"),
    include_str!("../../migrations/sqlite/0005_folders.sql"),
    include_str!("../../migrations/sqlite/0006_invitation_tokens.sql"),
    include_str!("../../migrations/sqlite/0007_user_management.sql"),
    include_str!("../../migrations/sqlite/0008_api_contract.sql"),
    include_str!("../../migrations/sqlite/0009_auth_attempts.sql"),
    include_str!("../../migrations/sqlite/0010_query_indexes.sql"),
    include_str!("../../migrations/sqlite/0011_markdown.sql"),
    include_str!("../../migrations/sqlite/0012_instance_administration.sql"),
    include_str!("../../migrations/sqlite/0013_invitation_management.sql"),
    include_str!("../../migrations/sqlite/0014_paste_modifications.sql"),
    include_str!("../../migrations/sqlite/0015_attachment_lifecycle.sql"),
    include_str!("../../migrations/sqlite/0016_atomic_login_throttling.sql"),
];

#[actix_web::test]
async fn sqlite_migration_repeatability_and_checksum_validation() {
    let (repo, data_dir) = sqlite_repository("migration").await;
    repo.migrate().await.unwrap();
    repo.migrate().await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1")
        .bind(vec![0_u8])
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(repo.migrate().await.is_err());
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn pending_paste_migration_reconciles_interrupted_legacy_creations() {
    let data_dir = std::env::temp_dir().join(format!(
        "racebin-pending-migration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let repo = Database::open(&sqlite_url(&data_dir), &data_dir)
        .await
        .unwrap();
    for migration in SQLITE_MIGRATIONS_BEFORE_PENDING_PASTES {
        sqlx::raw_sql(*migration)
            .execute(repo.pool())
            .await
            .unwrap();
    }
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role,is_owner,created_at)
         VALUES(1,'owner','unused','admin',1,1)",
    )
    .execute(repo.pool())
    .await
    .unwrap();
    for id in ["finished", "interrupted"] {
        sqlx::query(
            "INSERT INTO pastes(id,owner_id,title,content,content_kind,language,visibility,
                                created_at,updated_at,revision,read_count)
             VALUES($1,1,'','','text','plaintext','private',1,1,1,0)",
        )
        .bind(id)
        .execute(repo.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO idempotency_records(
               user_id,operation,key_hash,request_hash,paste_id,created_at,expires_at,
               state,owner_token,expected_attachment_count
             ) VALUES(1,'create_paste',$1,$1,$2,1,9999999999,'pending',$1,1)",
        )
        .bind(format!("key-{id}"))
        .bind(id)
        .execute(repo.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO attachments(paste_id,sort_order,filename,storage_key,size_bytes)
         VALUES('finished',0,'done.txt','done-storage',1)",
    )
    .execute(repo.pool())
    .await
    .unwrap();

    sqlx::raw_sql(include_str!(
        "../../migrations/sqlite/0017_pending_pastes.sql"
    ))
    .execute(repo.pool())
    .await
    .unwrap();

    let states: Vec<(String, String)> = sqlx::query_as(
        "SELECT id,creation_state FROM pastes WHERE id IN ('finished','interrupted') ORDER BY id",
    )
    .fetch_all(repo.pool())
    .await
    .unwrap();
    assert_eq!(
        states,
        vec![
            ("finished".into(), "complete".into()),
            ("interrupted".into(), "pending".into())
        ]
    );
    let finished_operation: (String, Option<String>) = sqlx::query_as(
        "SELECT state,owner_token FROM idempotency_records WHERE paste_id='finished'",
    )
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(finished_operation, ("completed".into(), None));
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn sqlite_query_indexes_match_repository_workloads() {
    let (repo, data_dir) = sqlite_repository("migration-indexes").await;
    repo.migrate().await.unwrap();

    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master
         WHERE type='index' AND name IN (
           'folders_owner_idx',
           'folders_owner_name_idx',
           'sessions_expiry_idx',
           'sessions_user_expiry_idx',
           'api_keys_user_idx',
           'invitations_expiry_idx',
           'password_reset_tokens_expiry_idx',
           'auth_attempts_expiry_idx',
           'pastes_folder_only_idx'
         )
         ORDER BY name",
    )
    .fetch_all(repo.pool())
    .await
    .unwrap();

    assert_eq!(
        indexes,
        [
            "api_keys_user_idx",
            "auth_attempts_expiry_idx",
            "folders_owner_name_idx",
            "invitations_expiry_idx",
            "password_reset_tokens_expiry_idx",
            "pastes_folder_only_idx",
            "sessions_expiry_idx",
            "sessions_user_expiry_idx",
        ]
    );

    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn sqlite_redirect_records_become_plaintext_pastes() {
    let data_dir = std::env::temp_dir().join(format!("racebin-redirect-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let repo = Database::open(&sqlite_url(&data_dir), &data_dir)
        .await
        .unwrap();
    for migration in [
        include_str!("../../migrations/sqlite/0001_schema.sql"),
        include_str!("../../migrations/sqlite/0002_rich_text.sql"),
        include_str!("../../migrations/sqlite/0003_invitation_redeemer.sql"),
    ] {
        sqlx::raw_sql(migration).execute(repo.pool()).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO pastes(
           id,title,content,content_kind,language,visibility,created_at,read_count
         ) VALUES('old-link','Old link','https://example.com','redirect','auto','unlisted',1,0)",
    )
    .execute(repo.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attachments(paste_id,sort_order,filename,storage_key,size_bytes)
         VALUES('old-link',0,'note.txt','stored-note',4)",
    )
    .execute(repo.pool())
    .await
    .unwrap();

    sqlx::raw_sql(include_str!(
        "../../migrations/sqlite/0004_remove_redirect_pastes.sql"
    ))
    .execute(repo.pool())
    .await
    .unwrap();

    let converted: (String, String, String) =
        sqlx::query_as("SELECT content_kind,language,content FROM pastes WHERE id='old-link'")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(
        converted,
        (
            "text".into(),
            "plaintext".into(),
            "https://example.com".into()
        )
    );
    let attachment_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM attachments WHERE paste_id='old-link'")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(attachment_count, 1);
    assert!(sqlx::query(
        "INSERT INTO pastes(
           id,title,content,content_kind,language,visibility,created_at,read_count
         ) VALUES('new-link','','https://example.com','redirect','plaintext','unlisted',1,0)",
    )
    .execute(repo.pool())
    .await
    .is_err());
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}
