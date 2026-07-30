use crate::account::{self as accounts, api_keys};
use crate::repository::{copy_database, DatabaseKind, Repository};
use crate::services::{now, PasteInput, PasteQuery, Principal, Services};
use std::path::{Path, PathBuf};

fn sqlite_url(data_dir: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        data_dir.join("database.sqlite").display()
    )
}

async fn sqlite_repository(label: &str) -> (Repository, PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("racebin-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let repository = Repository::open(&sqlite_url(&data_dir), &data_dir)
        .await
        .unwrap();
    repository.migrate().await.unwrap();
    (repository, data_dir)
}

async fn insert_user(repo: &Repository, id: i64, username: &str, role: &str) {
    sqlx::query(
        "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
         VALUES($1,$2,$3,$4,1,0,$5)",
    )
    .bind(id)
    .bind(username)
    .bind(accounts::password_hash("correct horse battery staple").unwrap())
    .bind(role)
    .bind(now())
    .execute(repo.pool())
    .await
    .unwrap();
    if repo.kind() == DatabaseKind::Postgres {
        sqlx::query(
            "SELECT setval(pg_get_serial_sequence('app_user','id'),
                           (SELECT max(id) FROM app_user),TRUE)",
        )
        .execute(repo.pool())
        .await
        .unwrap();
    }
}

fn principal(id: i64, username: &str, role: &str) -> Principal {
    Principal::User(accounts::SessionUser {
        user: accounts::User {
            id,
            username: username.to_string(),
            role: role.to_string(),
            enabled: true,
            force_password_change: false,
        },
        csrf_token: "csrf".to_string(),
    })
}

fn paste_input(title: &str, access: &str) -> PasteInput {
    PasteInput {
        title: Some(title.to_string()),
        content: Some(format!("content for {title}")),
        kind: Some("text".to_string()),
        syntax: Some("none".to_string()),
        access: Some(access.to_string()),
        expiration: None,
        burn_after_reads: None,
    }
}

async fn backend_contract(repo: Repository) {
    insert_user(&repo, 1, "administrator", "admin").await;
    insert_user(&repo, 2, "paste-owner", "user").await;
    let services = Services::new(repo.clone());
    let owner = principal(2, "paste-owner", "user");
    let anonymous = Principal::Anonymous;

    let verified = accounts::verify_user(&repo, "paste-owner", "correct horse battery staple")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified.id, 2);
    assert!(
        accounts::verify_user(&repo, "paste-owner", "wrong password")
            .await
            .unwrap()
            .is_none()
    );
    let (session_token, _, _) = accounts::create_session(&repo, 2, false).await.unwrap();
    assert_eq!(
        accounts::session_user(&repo, &session_token)
            .await
            .unwrap()
            .unwrap()
            .user
            .id,
        2
    );

    let invite = accounts::create_invite(&repo, 1).await.unwrap();
    let invited = accounts::accept_invite(
        &repo,
        &invite,
        "invited-user",
        "another correct horse battery staple",
    )
    .await
    .unwrap();
    assert_eq!(invited.username, "invited-user");

    let scopes = vec!["paste:read".to_string(), "paste:list".to_string()];
    let (key, token) = api_keys::create(&repo, Some(2), "contract key", &scopes)
        .await
        .unwrap();
    assert_eq!(
        api_keys::authenticate(&repo, &token)
            .await
            .unwrap()
            .unwrap()
            .id,
        key.id
    );
    assert_eq!(api_keys::list_for_user(&repo, 2).await.unwrap().len(), 1);
    assert!(api_keys::set_enabled_for_user(&repo, key.id, 2, false)
        .await
        .unwrap());
    assert!(api_keys::authenticate(&repo, &token)
        .await
        .unwrap()
        .is_none());
    assert!(api_keys::set_enabled_for_user(&repo, key.id, 2, true)
        .await
        .unwrap());
    sqlx::query("UPDATE app_user SET enabled=0 WHERE id=2")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(api_keys::authenticate(&repo, &token)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE app_user SET enabled=1 WHERE id=2")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(api_keys::authenticate(&repo, &token)
        .await
        .unwrap()
        .is_some());
    assert!(api_keys::delete_for_user(&repo, key.id, 2).await.unwrap());

    let public = services
        .create_paste(&owner, &paste_input("searchable public", "public"))
        .await
        .unwrap();
    let unlisted = services
        .create_paste(&owner, &paste_input("private link", "unlisted"))
        .await
        .unwrap();
    let owner_only = services
        .create_paste(&owner, &paste_input("owner secret", "owner"))
        .await
        .unwrap();
    assert!(services
        .get_paste(&anonymous, &public.slug)
        .await
        .unwrap()
        .is_some());
    assert!(services
        .get_paste(&anonymous, &unlisted.slug)
        .await
        .unwrap()
        .is_some());
    assert!(services
        .get_paste(&anonymous, &owner_only.slug)
        .await
        .unwrap()
        .is_none());

    let update = PasteInput {
        title: Some("updated title".to_string()),
        content: None,
        kind: None,
        syntax: None,
        access: None,
        expiration: None,
        burn_after_reads: None,
    };
    assert_eq!(
        services
            .update_paste(&owner, &public.slug, &update)
            .await
            .unwrap()
            .unwrap()
            .title,
        "updated title"
    );
    let search = services
        .list_pastes(
            &anonymous,
            &PasteQuery {
                search: Some("updated".to_string()),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(search.items.len(), 1);

    for index in 0..35 {
        services
            .create_paste(&owner, &paste_input(&format!("page-{index:02}"), "public"))
            .await
            .unwrap();
    }
    let first_page = services
        .list_pastes(
            &anonymous,
            &PasteQuery {
                page: Some(1),
                page_size: Some(10),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    let second_page = services
        .list_pastes(
            &anonymous,
            &PasteQuery {
                page: Some(2),
                page_size: Some(10),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(first_page.items.len(), 10);
    assert_eq!(second_page.items.len(), 10);
    assert!(first_page.total >= 36);
    assert_ne!(first_page.items[0].slug, second_page.items[0].slug);

    let expired = PasteInput {
        expiration: Some(Some(now() + 3600)),
        ..paste_input("expired", "public")
    };
    let expired = services.create_paste(&owner, &expired).await.unwrap();
    sqlx::query("UPDATE pasta SET expiration=$2 WHERE id=$1")
        .bind(expired.id)
        .bind(now() - 1)
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(services
        .get_paste(&anonymous, &expired.slug)
        .await
        .unwrap()
        .is_none());

    let cascade = services
        .create_paste(&owner, &paste_input("cascade", "owner"))
        .await
        .unwrap();
    services
        .add_files(
            &owner,
            &cascade.slug,
            &[("file.txt".to_string(), "stored-file".to_string(), 4)],
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM pasta WHERE id=$1")
        .bind(cascade.id)
        .execute(repo.pool())
        .await
        .unwrap();
    let file_count: i64 = sqlx::query_scalar("SELECT count(*) FROM pasta_file WHERE pasta_id=$1")
        .bind(cascade.id)
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(file_count, 0);

    let orphaned = services
        .create_paste(
            &principal(invited.id, "invited-user", "user"),
            &paste_input("orphaned owner", "owner"),
        )
        .await
        .unwrap();
    let (invited_session, _, _) = accounts::create_session(&repo, invited.id, false)
        .await
        .unwrap();
    sqlx::query("DELETE FROM app_user WHERE id=$1")
        .bind(invited.id)
        .execute(repo.pool())
        .await
        .unwrap();
    let owner_after_delete: Option<i64> =
        sqlx::query_scalar("SELECT owner_user_id FROM pasta WHERE id=$1")
            .bind(orphaned.id)
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(owner_after_delete, None);
    assert!(accounts::session_user(&repo, &invited_session)
        .await
        .unwrap()
        .is_none());

    let expiration_dir = repo.data_dir.join("attachments").join(&expired.slug);
    std::fs::create_dir_all(&expiration_dir).unwrap();
    std::fs::write(expiration_dir.join("stale"), b"stale").unwrap();
    sqlx::query(
        "INSERT INTO user_session(user_id,token_hash,csrf_token,created,expires,last_used)
         VALUES(2,'expired-session','csrf',1,1,1)",
    )
    .execute(repo.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_invite(token_hash,created_by,expires,used,revoked)
         VALUES('expired-invite',1,$1,0,0)",
    )
    .bind(now() - 3_000_000)
    .execute(repo.pool())
    .await
    .unwrap();
    assert!(repo.purge_expired(now()).await.unwrap() >= 1);
    assert!(!expiration_dir.exists());
    let expired_records: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM user_session WHERE token_hash='expired-session') +
                (SELECT count(*) FROM user_invite WHERE token_hash='expired-invite')",
    )
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(expired_records, 0);

    accounts::delete_session(&repo, &session_token)
        .await
        .unwrap();
    assert!(accounts::session_user(&repo, &session_token)
        .await
        .unwrap()
        .is_none());
    assert!(services.delete_paste(&owner, &unlisted.slug).await.unwrap());
}

async fn concurrency_contract(repo: Repository) {
    sqlx::query("UPDATE app_user SET role='user' WHERE role='admin'")
        .execute(repo.pool())
        .await
        .unwrap();
    insert_user(&repo, 10, "concurrency-admin", "admin").await;
    insert_user(&repo, 11, "second-admin", "admin").await;
    let services = Services::new(repo.clone());
    let admin = principal(10, "concurrency-admin", "admin");

    let burn = PasteInput {
        burn_after_reads: Some(1),
        ..paste_input("one read", "public")
    };
    let burn = services.create_paste(&admin, &burn).await.unwrap();
    let (left, right) = futures::join!(
        services.read_paste(&Principal::Anonymous, &burn.slug),
        services.read_paste(&Principal::Anonymous, &burn.slug)
    );
    assert_eq!(
        [left, right]
            .into_iter()
            .filter(|result| matches!(result, Ok(Some(_))))
            .count(),
        1
    );

    let invite = accounts::create_invite(&repo, 10).await.unwrap();
    let (left, right) = futures::join!(
        accounts::accept_invite(
            &repo,
            &invite,
            "invite-winner-a",
            "correct horse battery staple a"
        ),
        accounts::accept_invite(
            &repo,
            &invite,
            "invite-winner-b",
            "correct horse battery staple b"
        )
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);

    let (left, right) = futures::join!(
        accounts::set_enabled(&repo, 10, false),
        accounts::set_enabled(&repo, 11, false)
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);
    let enabled_admins: i64 =
        sqlx::query_scalar("SELECT count(*) FROM app_user WHERE role='admin' AND enabled=1")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(enabled_admins, 1);
    sqlx::query("UPDATE app_user SET enabled=1 WHERE id IN (10,11)")
        .execute(repo.pool())
        .await
        .unwrap();

    let (left, right) = futures::join!(
        accounts::set_role(&repo, 10, false),
        accounts::set_role(&repo, 11, false)
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);
    let admins: i64 = sqlx::query_scalar("SELECT count(*) FROM app_user WHERE role='admin'")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(admins, 1);

    let paste = services
        .create_paste(&admin, &paste_input("concurrent files", "owner"))
        .await
        .unwrap();
    let left_file = [("left.txt".to_string(), "left-store".to_string(), 1)];
    let right_file = [("right.txt".to_string(), "right-store".to_string(), 1)];
    let (left, right) = futures::join!(
        services.add_files(&admin, &paste.slug, &left_file),
        services.add_files(&admin, &paste.slug, &right_file)
    );
    assert!(left.is_ok());
    assert!(right.is_ok());
    let positions: Vec<i64> =
        sqlx::query_scalar("SELECT position FROM pasta_file WHERE pasta_id=$1 ORDER BY position")
            .bind(paste.id)
            .fetch_all(repo.pool())
            .await
            .unwrap();
    assert_eq!(positions, vec![0, 1]);
}

#[actix_web::test]
async fn sqlite_backend_contract_and_concurrency() {
    let (repo, data_dir) = sqlite_repository("contract").await;
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn postgres_backend_contract_concurrency_and_copy() {
    let Ok(url) = std::env::var("RACEBIN_TEST_POSTGRES_URL") else {
        return;
    };
    let scratch = std::env::temp_dir().join(format!("racebin-postgres-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&scratch).unwrap();
    let bootstrap = Repository::open(&url, &scratch).await.unwrap();
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    sqlx::query("CREATE SCHEMA public")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    drop(bootstrap);

    let repo = Repository::open(&url, &scratch).await.unwrap();
    repo.migrate().await.unwrap();
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    drop(repo);

    sqlx::query(
        "TRUNCATE pasta_file,pasta,api_key,user_session,user_invite,app_user
         RESTART IDENTITY CASCADE",
    )
    .execute(Repository::open(&url, &scratch).await.unwrap().pool())
    .await
    .unwrap();
    database_copy_contract(&url, &scratch).await;
    let _ = std::fs::remove_dir_all(scratch);
}

async fn database_copy_contract(postgres_url: &str, data_dir: &Path) {
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

#[actix_web::test]
async fn sqlite_migration_adoption_repeatability_and_checksum_validation() {
    let (repo, data_dir) = sqlite_repository("migration").await;
    repo.migrate().await.unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations")
        .execute(repo.pool())
        .await
        .unwrap();
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
