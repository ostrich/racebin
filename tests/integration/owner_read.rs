use super::*;

#[actix_web::test]
async fn owner_session_reads_do_not_increment_or_consume() {
    let (repository, data_dir) = sqlite_repository("limited-owner-read").await;
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role,created_at)
         VALUES(7,'owner','unused','user',0)",
    )
    .execute(repository.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pastes(id,owner_id,title,content,content_kind,language,visibility,
         created_at,read_count,read_limit)
         VALUES('limited',7,'','secret','text','plaintext','unlisted',0,0,1)",
    )
    .execute(repository.pool())
    .await
    .unwrap();
    let owner = Principal::Session(accounts::SessionUser {
        user: accounts::User {
            id: 7,
            username: "private".to_string(),
            role: "user".to_string(),
            is_owner: false,
            enabled: true,
            password_change_required: false,
        },
        csrf_token: "csrf".to_string(),
        reauthenticated_at: None,
    });
    let services = PasteService::new(repository.clone());
    let owner_read = services
        .read_paste(&owner, "limited", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(owner_read.paste.content, "secret");
    assert_eq!(owner_read.paste.read_count, 0);
    assert_eq!(owner_read.paste.last_read_at, None);
    assert_eq!(owner_read.paste.revision, 1);
    assert_eq!(owner_read.grant_token, None);
    let unchanged: (i64, Option<i64>, i64, Option<i64>) = sqlx::query_as(
        "SELECT read_count,last_read_at,revision,consumed_at FROM pastes WHERE id=$1",
    )
    .bind("limited")
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(unchanged, (0, None, 1, None));

    let consumed = services
        .read_paste(&Principal::Anonymous, "limited", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(consumed.paste.read_count, 1);
    let grant = consumed
        .grant_token
        .expect("a limited read produces a follow-up access grant");
    assert!(services.valid_read_grant("limited", &grant).await.unwrap());
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pastes WHERE id=$1 AND consumed_at IS NULL")
            .bind("limited")
            .fetch_one(repository.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    let tombstone: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pastes WHERE id=$1 AND consumed_at IS NOT NULL")
            .bind("limited")
            .fetch_one(repository.pool())
            .await
            .unwrap();
    assert_eq!(tombstone, 1);
    drop(repository);
    let _ = std::fs::remove_dir_all(data_dir);
}
