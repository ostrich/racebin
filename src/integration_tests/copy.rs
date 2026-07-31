use super::*;

pub(super) async fn database_copy_contract(postgres_url: &str, data_dir: &Path) {
    let source_dir = data_dir.join("copy-source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source_url = sqlite_url(&source_dir);
    let source = Repository::open(&source_url, &source_dir).await.unwrap();
    source.migrate().await.unwrap();
    insert_user(&source, 42, "copied-user", "admin").await;
    sqlx::query("UPDATE users SET last_login_at=1234 WHERE id=42")
        .execute(source.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO folders(id,owner_id,name,name_key,created_at) VALUES(80,42,'Copied folder','copied folder',1)")
        .execute(source.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO pastes(id,owner_id,folder_id,title,content,content_kind,language,visibility,created_at,read_count,read_limit)
         VALUES('copied-paste',42,80,'copied','body','text','plaintext','private',1,0,NULL)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO password_reset_tokens(user_id,token_hash,created_by_user_id,created_at,expires_at)
         VALUES(42,'reset-hash',42,1,9999999999)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pastes(id,owner_id,title,content,document_json,content_kind,language,visibility,created_at,read_count,read_limit)
         VALUES('copied-rich',42,'script','ADA\nHello.',$1,'rich_text','plaintext','private',1,0,NULL)",
    )
    .bind(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"ADA"},{"type":"hardBreak"},{"type":"text","text":"Hello."}]}]}"#)
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions(id,user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
         VALUES(60,42,'session-hash','csrf',1,9999999999,1)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invitations(
            id,token_hash,token,created_by_user_id,expires_at,redeemed,redeemed_by_user_id,revoked
         ) VALUES(61,'invitation-hash','copy-token',42,9999999999,1,42,0)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO api_keys(id,user_id,name,token_prefix,token_hash,created_at,enabled)
         VALUES(62,42,'copied key','prefix','key-hash',1,1)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO api_key_scopes(api_key_id,scope) VALUES(62,'paste:read')")
        .execute(source.pool())
        .await
        .unwrap();
    let attachment_dir = source_dir.join("attachments/copied-paste");
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("stored"), b"data").unwrap();
    sqlx::query(
        "INSERT INTO attachments(id,paste_id,sort_order,filename,storage_key,size_bytes)
         VALUES(70,'copied-paste',0,'file.txt','stored',4)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    drop(source);

    copy_database(&source_url, postgres_url, &source_dir)
        .await
        .unwrap();
    let destination = Repository::open(postgres_url, &source_dir).await.unwrap();
    let copied: (i64, String) = sqlx::query_as("SELECT id,username FROM users")
        .fetch_one(destination.pool())
        .await
        .unwrap();
    assert_eq!(copied, (42, "copied-user".to_string()));
    let last_login: Option<i64> = sqlx::query_scalar("SELECT last_login_at FROM users WHERE id=42")
        .fetch_one(destination.pool())
        .await
        .unwrap();
    assert_eq!(last_login, Some(1234));
    for table in [
        "folders",
        "sessions",
        "password_reset_tokens",
        "invitations",
        "api_keys",
        "api_key_scopes",
        "pastes",
        "attachments",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(destination.pool())
            .await
            .unwrap();
        assert_eq!(count, if table == "pastes" { 2 } else { 1 }, "{table}");
    }
    let rich_document: String =
        sqlx::query_scalar("SELECT document_json FROM pastes WHERE id='copied-rich'")
            .fetch_one(destination.pool())
            .await
            .unwrap();
    assert!(rich_document.contains("\"hardBreak\""));
    let invitation_redeemer: Option<i64> =
        sqlx::query_scalar("SELECT redeemed_by_user_id FROM invitations WHERE id=61")
            .fetch_one(destination.pool())
            .await
            .unwrap();
    assert_eq!(invitation_redeemer, Some(42));
    let invitation_token: Option<String> =
        sqlx::query_scalar("SELECT token FROM invitations WHERE id=61")
            .fetch_one(destination.pool())
            .await
            .unwrap();
    assert_eq!(invitation_token.as_deref(), Some("copy-token"));
    let copied_folder: Option<i64> =
        sqlx::query_scalar("SELECT folder_id FROM pastes WHERE id='copied-paste'")
            .fetch_one(destination.pool())
            .await
            .unwrap();
    assert_eq!(copied_folder, Some(80));
    let next_user: i64 = sqlx::query_scalar(
        "INSERT INTO users(username,password_hash,role,created_at)
         VALUES('after-copy','hash','user',1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_user > 42);
    let next_folder: i64 = sqlx::query_scalar(
        "INSERT INTO folders(owner_id,name,name_key,created_at) VALUES(42,'After copy','after copy',1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_folder > 80);
    let next_paste: String = sqlx::query_scalar(
        "INSERT INTO pastes(id,title,content,content_kind,language,visibility,created_at,read_count,read_limit)
         VALUES('after-copy','','','text','plaintext','public',1,0,NULL) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert_eq!(next_paste, "after-copy");
    let next_session: i64 = sqlx::query_scalar(
        "INSERT INTO sessions(user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
         VALUES(42,'after-copy-session','csrf',1,9999999999,1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_session > 60);
    let next_invite: i64 = sqlx::query_scalar(
        "INSERT INTO invitations(token_hash,created_by_user_id,expires_at,redeemed,revoked)
         VALUES('after-copy-invitation',42,9999999999,0,0) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_invite > 61);
    let next_key: i64 = sqlx::query_scalar(
        "INSERT INTO api_keys(user_id,name,token_prefix,token_hash,created_at,enabled)
         VALUES(42,'after copy','after-prefix','after-key-hash',1,1)
         RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_key > 62);
    let next_attachment: i64 = sqlx::query_scalar(
        "INSERT INTO attachments(paste_id,sort_order,filename,storage_key,size_bytes)
         VALUES('copied-paste',1,'after.txt','after-store',1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_attachment > 70);
    let error = copy_database(&source_url, postgres_url, &source_dir)
        .await
        .unwrap_err();
    assert!(error.contains("not empty"));
    sqlx::query(
        "TRUNCATE attachments,pastes,folders,api_key_scopes,api_keys,password_reset_tokens,sessions,invitations,users
         RESTART IDENTITY CASCADE",
    )
    .execute(destination.pool())
    .await
    .unwrap();
    drop(destination);

    let missing_dir = data_dir.join("missing-source");
    std::fs::create_dir_all(&missing_dir).unwrap();
    let missing_url = sqlite_url(&missing_dir);
    let missing = Repository::open(&missing_url, &missing_dir).await.unwrap();
    missing.migrate().await.unwrap();
    insert_user(&missing, 1, "missing-owner", "user").await;
    sqlx::query(
        "INSERT INTO pastes(id,owner_id,title,content,content_kind,language,visibility,created_at,read_count,read_limit)
         VALUES('missing-attachment',1,'','','text','plaintext','private',1,0,NULL)",
    )
    .execute(missing.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO attachments(id,paste_id,sort_order,filename,storage_key,size_bytes)
         VALUES(1,'missing-attachment',0,'missing.txt','not-on-disk',4)",
    )
    .execute(missing.pool())
    .await
    .unwrap();
    drop(missing);
    assert!(copy_database(&missing_url, postgres_url, &missing_dir)
        .await
        .unwrap_err()
        .contains("is missing"));

    let invalid_dir = data_dir.join("invalid-source");
    std::fs::create_dir_all(&invalid_dir).unwrap();
    let invalid_url = sqlite_url(&invalid_dir);
    let invalid = Repository::open(&invalid_url, &invalid_dir).await.unwrap();
    invalid.migrate().await.unwrap();
    let mut connection = invalid.pool().acquire().await.unwrap();
    sqlx::query("PRAGMA ignore_check_constraints=ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role,enabled,password_change_required,created_at)
         VALUES(1,'valid-before-failure','hash','user',1,0,1),
               (2,'invalid-role','hash','invalid',1,0,1)",
    )
    .execute(&mut *connection)
    .await
    .unwrap();
    drop(connection);
    drop(invalid);
    assert!(copy_database(&invalid_url, postgres_url, &invalid_dir)
        .await
        .is_err());
    let destination = Repository::open(postgres_url, data_dir).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(destination.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "failed copy must roll back all inserted rows");
}
