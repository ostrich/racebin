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
    managed_target_contract(&repo).await;
    super::hot_paths::hot_path_contract(&repo).await;
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
    managed_target_contract(&repo).await;
    super::paste_security::paste_content_contract(&repo).await;
    super::hot_paths::hot_path_contract(&repo).await;
    postgres_expiration_race_contract(&repo).await;
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

async fn managed_target_contract(repo: &Database) {
    use racebin::accounts::AdminTargetAuthority;

    insert_user(repo, 110, "managed-target", "user").await;
    insert_user(repo, 111, "ordinary-target", "user").await;
    sqlx::query("UPDATE users SET role='admin',is_owner=1 WHERE id=1")
        .execute(repo.pool())
        .await
        .unwrap();
    let non_owner = AdminTargetAuthority::new(None);
    let owner = AdminTargetAuthority::new(Some(1));
    let (key, _) = api_keys::create(repo, Some(110), "managed key", &["paste:read".into()])
        .await
        .unwrap();

    // Model the exact stale-preflight interleaving: the target was ordinary
    // when the handler inspected it, then the owner promoted it before write.
    assert_eq!(
        accounts::admin_user(repo, 110).await.unwrap().unwrap().role,
        "user"
    );
    accounts::set_role(repo, 110, true).await.unwrap();
    assert_eq!(
        accounts::update_user_managed(repo, 110, Some(false), None, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert_eq!(
        accounts::create_password_reset_managed(repo, 110, 1, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert_eq!(
        accounts::revoke_sessions_managed(repo, 110, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert_eq!(
        api_keys::delete_all_for_user_managed(repo, 110, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert_eq!(
        api_keys::set_enabled_managed(repo, key.id, false, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert_eq!(
        api_keys::delete_managed(repo, key.id, non_owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    assert!(api_keys::get(repo, key.id).await.unwrap().is_some());

    // Ordinary targets remain manageable by a non-owner administrator, and
    // the current owner may still manage the newly promoted administrator.
    assert!(
        accounts::create_password_reset_managed(repo, 111, 1, non_owner)
            .await
            .is_ok()
    );
    assert!(
        accounts::update_user_managed(repo, 110, Some(true), None, owner)
            .await
            .is_ok()
    );

    // A stale owner session cannot perform an owner-only role change after
    // ownership has moved away from that account.
    sqlx::query("UPDATE users SET is_owner=0 WHERE id=1")
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        accounts::update_user_managed(repo, 111, None, Some(true), owner)
            .await
            .unwrap_err()
            .code,
        "owner_required"
    );
    sqlx::query("UPDATE users SET is_owner=1 WHERE id=1")
        .execute(repo.pool())
        .await
        .unwrap();
}

async fn postgres_expiration_race_contract(repo: &Database) {
    let services = PasteService::new(repo.clone());
    let paste = services
        .create_paste(
            &principal(111, "ordinary-target", "user"),
            &paste_input("expiring attachment", "private"),
        )
        .await
        .unwrap();
    let now = racebin::time::unix_timestamp();
    sqlx::query("UPDATE pastes SET expires_at=$2 WHERE id=$1")
        .bind(&paste.id)
        .bind(now - 1)
        .execute(repo.pool())
        .await
        .unwrap();
    let attachment_dir = repo.data_dir.join("attachments").join(&paste.id);
    std::fs::create_dir_all(&attachment_dir).unwrap();
    let attachment = attachment_dir.join("still-live");
    std::fs::write(&attachment, b"keep me").unwrap();

    let mut updating = repo.pool().begin().await.unwrap();
    sqlx::query("SELECT id FROM pastes WHERE id=$1 FOR UPDATE")
        .bind(&paste.id)
        .fetch_one(&mut *updating)
        .await
        .unwrap();
    let cleanup_repo = repo.clone();
    let cleanup = tokio::spawn(async move { cleanup_repo.purge_expired(now).await });

    // Wait for cleanup to reach the locked DELETE, then extend the paste's
    // expiration. PostgreSQL must recheck the predicate and RETURNING must
    // omit this surviving row, so its attachment directory stays intact.
    let mut blocked = false;
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
             WHERE datname=current_database() AND pid<>pg_backend_pid()
               AND query LIKE 'DELETE FROM pastes%' AND wait_event_type='Lock'",
        )
        .fetch_one(repo.pool())
        .await
        .unwrap();
        if waiting > 0 {
            blocked = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(blocked, "cleanup did not reach the locked paste row");
    sqlx::query("UPDATE pastes SET expires_at=$2 WHERE id=$1")
        .bind(&paste.id)
        .bind(now + 3600)
        .execute(&mut *updating)
        .await
        .unwrap();
    updating.commit().await.unwrap();
    cleanup.await.unwrap().unwrap();
    assert!(services
        .get_paste(&principal(111, "ordinary-target", "user"), &paste.id)
        .await
        .unwrap()
        .is_some());
    assert_eq!(std::fs::read(attachment).unwrap(), b"keep me");
}
