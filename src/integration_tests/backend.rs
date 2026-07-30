use super::*;

pub(super) async fn backend_contract(repo: Repository) {
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
