use super::*;

pub(super) async fn database_copy_contract(postgres_url: &str, data_dir: &Path) {
    let source_dir = data_dir.join("copy-source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source_url = sqlite_url(&source_dir);
    let source = Repository::open(&source_url, &source_dir).await.unwrap();
    source.migrate().await.unwrap();
    insert_user(&source, 42, "copied-user", "admin").await;
    sqlx::query(
        "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,created,read_count,burn_after_reads)
         VALUES(100,'copied-paste',42,'copied','body','text','none','owner',1,0,0)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_session(id,user_id,token_hash,csrf_token,created,expires,last_used)
         VALUES(60,42,'session-hash','csrf',1,9999999999,1)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_invite(id,token_hash,created_by,expires,used,revoked)
         VALUES(61,'invite-hash',42,9999999999,0,0)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO api_key(id,user_id,name,prefix,token_hash,scopes,created,enabled)
         VALUES(62,42,'copied key','prefix','key-hash','paste:read',1,1)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    let attachment_dir = source_dir.join("attachments/copied-paste");
    std::fs::create_dir_all(&attachment_dir).unwrap();
    std::fs::write(attachment_dir.join("stored"), b"data").unwrap();
    sqlx::query(
        "INSERT INTO pasta_file(id,pasta_id,position,role,name,storage_name,size)
         VALUES(70,100,0,'primary','file.txt','stored',4)",
    )
    .execute(source.pool())
    .await
    .unwrap();
    drop(source);

    copy_database(&source_url, postgres_url, &source_dir)
        .await
        .unwrap();
    let destination = Repository::open(postgres_url, &source_dir).await.unwrap();
    let copied: (i64, String) = sqlx::query_as("SELECT id,username FROM app_user")
        .fetch_one(destination.pool())
        .await
        .unwrap();
    assert_eq!(copied, (42, "copied-user".to_string()));
    for table in [
        "user_session",
        "user_invite",
        "api_key",
        "pasta",
        "pasta_file",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(destination.pool())
            .await
            .unwrap();
        assert_eq!(count, 1, "{table}");
    }
    let next_user: i64 = sqlx::query_scalar(
        "INSERT INTO app_user(username,password_hash,role,created)
         VALUES('after-copy','hash','user',1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_user > 42);
    let next_paste: i64 = sqlx::query_scalar(
        "INSERT INTO pasta(slug,title,content,kind,syntax,access,created,read_count,burn_after_reads)
         VALUES('after-copy','','','text','none','public',1,0,0) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_paste > 100);
    let next_session: i64 = sqlx::query_scalar(
        "INSERT INTO user_session(user_id,token_hash,csrf_token,created,expires,last_used)
         VALUES(42,'after-copy-session','csrf',1,9999999999,1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_session > 60);
    let next_invite: i64 = sqlx::query_scalar(
        "INSERT INTO user_invite(token_hash,created_by,expires,used,revoked)
         VALUES('after-copy-invite',42,9999999999,0,0) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_invite > 61);
    let next_key: i64 = sqlx::query_scalar(
        "INSERT INTO api_key(user_id,name,prefix,token_hash,scopes,created,enabled)
         VALUES(42,'after copy','after-prefix','after-key-hash','paste:read',1,1)
         RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_key > 62);
    let next_file: i64 = sqlx::query_scalar(
        "INSERT INTO pasta_file(pasta_id,position,role,name,storage_name,size)
         VALUES(100,1,'attachment','after.txt','after-store',1) RETURNING id",
    )
    .fetch_one(destination.pool())
    .await
    .unwrap();
    assert!(next_file > 70);
    let error = copy_database(&source_url, postgres_url, &source_dir)
        .await
        .unwrap_err();
    assert!(error.contains("not empty"));
    sqlx::query(
        "TRUNCATE pasta_file,pasta,api_key,user_session,user_invite,app_user
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
        "INSERT INTO pasta(id,slug,owner_user_id,title,content,kind,syntax,access,created,read_count,burn_after_reads)
         VALUES(1,'missing-file',1,'','','text','none','owner',1,0,0)",
    )
    .execute(missing.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pasta_file(id,pasta_id,position,role,name,storage_name,size)
         VALUES(1,1,0,'primary','missing.txt','not-on-disk',4)",
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
        "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
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
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM app_user")
        .fetch_one(destination.pool())
        .await
        .unwrap();
    assert_eq!(count, 0, "failed copy must roll back all inserted rows");
}
