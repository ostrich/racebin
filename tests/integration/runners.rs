use super::backend::backend_contract;
use super::concurrency::concurrency_contract;
use super::copy::database_copy_contract;
use super::*;

#[actix_web::test]
async fn sqlite_backend_contract_and_concurrency() {
    let (repo, data_dir) = sqlite_repository("contract").await;
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    account_security_contract(&repo).await;
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
    let bootstrap = Database::open(&url, &scratch).await.unwrap();
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    sqlx::query("CREATE SCHEMA public")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    drop(bootstrap);

    let repo = Database::open(&url, &scratch).await.unwrap();
    repo.migrate().await.unwrap();
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    account_security_contract(&repo).await;
    super::paste_security::paste_content_contract(&repo).await;
    drop(repo);

    sqlx::query(
        "TRUNCATE audit_events,instance_settings,attachments,pastes,folders,api_key_scopes,api_keys,password_reset_tokens,sessions,invitations,users
         RESTART IDENTITY CASCADE",
    )
    .execute(Database::open(&url, &scratch).await.unwrap().pool())
    .await
    .unwrap();
    database_copy_contract(&url, &scratch).await;
    let _ = std::fs::remove_dir_all(scratch);
}

async fn account_security_contract(repo: &Database) {
    insert_user(repo, 100, "retired-admin", "admin").await;
    let scopes = vec!["paste:manage".to_string()];
    let (key, token) = api_keys::create(repo, Some(100), "privileged", &scopes)
        .await
        .unwrap();
    assert!(api_keys::authenticate(repo, &token)
        .await
        .unwrap()
        .is_some());
    accounts::set_role(repo, 100, false).await.unwrap();
    assert!(!api_keys::get(repo, key.id).await.unwrap().unwrap().enabled);
    assert!(api_keys::set_enabled_for_user(repo, key.id, 100, true)
        .await
        .is_err());
    assert!(api_keys::set_enabled(repo, key.id, true).await.is_err());
    assert!(api_keys::create(repo, Some(100), "new privileged", &scopes)
        .await
        .is_err());
    // Authentication also fails closed if an out-of-band update bypasses activation.
    sqlx::query("UPDATE api_keys SET enabled=1 WHERE id=$1")
        .bind(key.id)
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(api_keys::authenticate(repo, &token)
        .await
        .unwrap()
        .is_none());

    accounts::set_role(repo, 100, true).await.unwrap();
    let (manager, _) = api_keys::create(
        repo,
        Some(100),
        "delegated manager",
        &["api_key:manage".into()],
    )
    .await
    .unwrap();
    let (target, _) = api_keys::create(repo, Some(100), "read key", &["paste:read".into()])
        .await
        .unwrap();
    api_keys::set_enabled(repo, target.id, false).await.unwrap();
    let services = PasteService::new(repo.clone());
    assert!(services
        .set_api_key_enabled(&Principal::ApiKey(manager), target.id, true)
        .await
        .is_err());
    assert!(
        !api_keys::get(repo, target.id)
            .await
            .unwrap()
            .unwrap()
            .enabled
    );
    assert!(api_keys::set_enabled_for_user(repo, target.id, 100, true)
        .await
        .unwrap());

    for _ in 0..5 {
        assert!(
            accounts::reserve_login_attempt(repo, "CaseSensitive", "case-test")
                .await
                .unwrap()
                .is_none()
        );
    }
    assert!(
        accounts::reserve_login_attempt(repo, "casesensitive", "case-test")
            .await
            .unwrap()
            .is_none()
    );
    accounts::clear_login_failures(repo, "casesensitive")
        .await
        .unwrap();
    assert!(
        accounts::reserve_login_attempt(repo, "CaseSensitive", "case-test")
            .await
            .unwrap()
            .is_some()
    );
    accounts::reserve_login_attempt(repo, &"x".repeat(32_768), "bounded-test")
        .await
        .unwrap();
    let oversized: i64 =
        sqlx::query_scalar("SELECT count(*) FROM auth_buckets WHERE length(subject)>64")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(oversized, 0);
    let now = racebin::time::unix_timestamp();
    sqlx::query("UPDATE auth_buckets SET window_started_at=$1")
        .bind(now - 901)
        .execute(repo.pool())
        .await
        .unwrap();
    repo.purge_expired(now).await.unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_buckets")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}
