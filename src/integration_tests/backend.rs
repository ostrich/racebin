use super::*;

pub(super) async fn backend_contract(repo: Repository) {
    insert_user(&repo, 1, "administrator", "admin").await;
    insert_user(&repo, 2, "paste-owner", "user").await;
    let services = PasteService::new(repo.clone());
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

    let invitation = accounts::create_invitation(&repo, 1).await.unwrap();
    let invited = accounts::redeem_invitation(
        &repo,
        &invitation,
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
    sqlx::query("UPDATE users SET enabled=0 WHERE id=2")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(api_keys::authenticate(&repo, &token)
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE users SET enabled=1 WHERE id=2")
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
    let private_paste = services
        .create_paste(&owner, &paste_input("owner secret", "private"))
        .await
        .unwrap();
    assert!(services
        .get_paste(&anonymous, &public.id)
        .await
        .unwrap()
        .is_some());
    assert!(services
        .get_paste(&anonymous, &unlisted.id)
        .await
        .unwrap()
        .is_some());
    assert!(services
        .get_paste(&anonymous, &private_paste.id)
        .await
        .unwrap()
        .is_none());

    let update = PasteInput {
        title: Some("updated title".to_string()),
        content: None,
        content_kind: None,
        language: None,
        visibility: None,
        expires_at: None,
        read_limit: None,
    };
    assert_eq!(
        services
            .update_paste(&owner, &public.id, &update)
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
    assert!(first_page.total_items >= 36);
    assert_ne!(first_page.items[0].id, second_page.items[0].id);

    let expired = PasteInput {
        expires_at: Some(Some(crate::time::unix_timestamp() + 3600)),
        ..paste_input("expired", "public")
    };
    let expired = services.create_paste(&owner, &expired).await.unwrap();
    sqlx::query("UPDATE pastes SET expires_at=$2 WHERE id=$1")
        .bind(&expired.id)
        .bind(crate::time::unix_timestamp() - 1)
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(services
        .get_paste(&anonymous, &expired.id)
        .await
        .unwrap()
        .is_none());

    let cascade = services
        .create_paste(&owner, &paste_input("cascade", "private"))
        .await
        .unwrap();
    services
        .add_attachments(
            &owner,
            &cascade.id,
            &[("file.txt".to_string(), "stored-file".to_string(), 4)],
        )
        .await
        .unwrap();
    sqlx::query("DELETE FROM pastes WHERE id=$1")
        .bind(&cascade.id)
        .execute(repo.pool())
        .await
        .unwrap();
    let file_count: i64 = sqlx::query_scalar("SELECT count(*) FROM attachments WHERE paste_id=$1")
        .bind(&cascade.id)
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(file_count, 0);

    let orphaned = services
        .create_paste(
            &principal(invited.id, "invited-user", "user"),
            &paste_input("orphaned owner", "private"),
        )
        .await
        .unwrap();
    let (invited_session, _, _) = accounts::create_session(&repo, invited.id, false)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(invited.id)
        .execute(repo.pool())
        .await
        .unwrap();
    let owner_after_delete: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM pastes WHERE id=$1")
            .bind(orphaned.id)
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(owner_after_delete, None);
    assert!(accounts::session_user(&repo, &invited_session)
        .await
        .unwrap()
        .is_none());

    let expiration_dir = repo.data_dir.join("attachments").join(&expired.id);
    std::fs::create_dir_all(&expiration_dir).unwrap();
    std::fs::write(expiration_dir.join("stale"), b"stale").unwrap();
    sqlx::query(
        "INSERT INTO sessions(user_id,token_hash,csrf_token,created_at,expires_at,last_used_at)
         VALUES(2,'expired-session','csrf',1,1,1)",
    )
    .execute(repo.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO invitations(token_hash,created_by_user_id,expires_at,redeemed,revoked)
         VALUES('expired-invitation',1,$1,0,0)",
    )
    .bind(crate::time::unix_timestamp() - 3_000_000)
    .execute(repo.pool())
    .await
    .unwrap();
    assert!(
        repo.purge_expired(crate::time::unix_timestamp())
            .await
            .unwrap()
            >= 1
    );
    assert!(!expiration_dir.exists());
    let expired_records: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM sessions WHERE token_hash='expired-session') +
                (SELECT count(*) FROM invitations WHERE token_hash='expired-invitation')",
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
    assert!(services.delete_paste(&owner, &unlisted.id).await.unwrap());
}
