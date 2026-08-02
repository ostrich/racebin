use super::*;
use crate::services::ErrorKind;

pub(super) async fn backend_contract(repo: Repository) {
    insert_user(&repo, 1, "administrator", "admin").await;
    insert_user(&repo, 2, "paste-owner", "user").await;
    let last_admin = accounts::update_user(&repo, 1, Some(false), Some(false))
        .await
        .unwrap_err();
    assert_eq!(last_admin.code, "last_administrator");
    assert_eq!(
        last_admin.message,
        "The last enabled administrator cannot be disabled"
    );
    let administrator: (String, i64) = sqlx::query_as("SELECT role,enabled FROM users WHERE id=1")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(administrator, ("admin".to_string(), 1));
    accounts::update_user(&repo, 2, Some(false), Some(true))
        .await
        .unwrap();
    let updated_user: (String, i64) = sqlx::query_as("SELECT role,enabled FROM users WHERE id=2")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(updated_user, ("admin".to_string(), 0));
    accounts::update_user(&repo, 2, Some(true), Some(false))
        .await
        .unwrap();
    let services = PasteService::new(repo.clone());
    let owner = principal(2, "paste-owner", "user");
    let anonymous = Principal::Anonymous;
    let unsupported_kind = PasteInput {
        content_kind: Some("redirect".into()),
        ..paste_input("unsupported", "public")
    };
    assert!(services
        .create_paste(&owner, &unsupported_kind)
        .await
        .unwrap_err()
        .message
        .contains("Content kind"));

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
    let last_login: Option<i64> = sqlx::query_scalar("SELECT last_login_at FROM users WHERE id=2")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert!(last_login.is_some());
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
    let invalid_invitation =
        accounts::redeem_invitation(&repo, "invalid-token", "valid-name", "short")
            .await
            .unwrap_err();
    assert_eq!(invalid_invitation.code, "invalid_invitation");
    assert_eq!(
        accounts::list_invitations(&repo).await.unwrap()[0]
            .token
            .as_deref(),
        Some(invitation.as_str())
    );
    let invited = accounts::redeem_invitation(
        &repo,
        &invitation,
        "invited-user",
        "another correct horse battery staple",
    )
    .await
    .unwrap();
    let invitations = accounts::list_invitations(&repo).await.unwrap();
    assert_eq!(
        invitations[0].redeemed_by_username.as_deref(),
        Some("invited-user")
    );
    assert!(invitations[0].token.is_none());
    assert_eq!(invited.username, "invited-user");

    let scopes = vec!["paste:read".to_string(), "paste:list".to_string()];
    let invalid_key = api_keys::create(&repo, Some(2), "", &scopes)
        .await
        .unwrap_err();
    assert_eq!(invalid_key.kind, ErrorKind::Validation);
    assert_eq!(invalid_key.code, "invalid_api_key_name");
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
    let folder = services.create_folder(&owner, "Scripts").await.unwrap();
    assert!(services.create_folder(&owner, "scripts").await.is_err());
    services
        .move_pastes(
            &owner,
            std::slice::from_ref(&private_paste.id),
            Some(folder.id),
        )
        .await
        .unwrap();
    assert_eq!(
        services
            .get_paste(&owner, &private_paste.id)
            .await
            .unwrap()
            .unwrap()
            .folder_id,
        Some(folder.id)
    );
    assert_eq!(
        services.list_folders(&owner).await.unwrap().items[0].paste_count,
        1
    );
    let folder_page = services
        .list_pastes(
            &owner,
            &PasteQuery {
                mine: Some(true),
                folder_id: Some(folder.id),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(folder_page.items.len(), 1);
    assert!(services.delete_folder(&owner, folder.id).await.unwrap());
    assert_eq!(
        services
            .get_paste(&owner, &private_paste.id)
            .await
            .unwrap()
            .unwrap()
            .folder_id,
        None
    );
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
        document: None,
        content_kind: None,
        language: None,
        visibility: None,
        expires_at: None,
        read_limit: None,
        folder_id: None,
    };
    assert_eq!(
        services
            .update_paste(&owner, &public.id, &update, None)
            .await
            .unwrap()
            .unwrap()
            .title,
        "updated title"
    );
    let paste_count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM pastes")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    let expired_input = PasteInput {
        expires_at: Some(Some(crate::time::unix_timestamp())),
        ..paste_input("already expired", "private")
    };
    assert_eq!(
        services
            .create_paste(&owner, &expired_input)
            .await
            .unwrap_err()
            .message,
        "Expiration must be in the future"
    );
    let paste_count_after: i64 = sqlx::query_scalar("SELECT count(*) FROM pastes")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(paste_count_after, paste_count_before);
    let invalid_update = PasteInput {
        title: Some("must not persist".to_string()),
        expires_at: Some(Some(crate::time::unix_timestamp())),
        ..PasteInput::default()
    };
    assert_eq!(
        services
            .update_paste(&owner, &public.id, &invalid_update, None)
            .await
            .unwrap_err()
            .message,
        "Expiration must be in the future"
    );
    assert_eq!(
        services
            .get_source(&owner, &public.id)
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

    let script_document = serde_json::json!({
        "type": "doc",
        "content": [
            {"type":"heading","attrs":{"level":1,"textAlign":"center"},"content":[
                {"type":"text","text":"The Test Episode","marks":[{"type":"bold"}]}
            ]},
            {"type":"paragraph","content":[
                {"type":"text","text":"INT. LAB - NIGHT"},
                {"type":"hardBreak"},
                {"type":"text","text":"ADA: The conversion works.","marks":[{"type":"italic"}]}
            ]}
        ]
    });
    let rich_input = PasteInput {
        title: Some("Rich script".into()),
        content: None,
        document: Some(script_document.clone()),
        content_kind: Some("rich_text".into()),
        language: Some("plaintext".into()),
        visibility: Some("public".into()),
        expires_at: None,
        read_limit: None,
        folder_id: None,
    };
    let rich = services.create_paste(&owner, &rich_input).await.unwrap();
    assert_eq!(rich.document, Some(script_document));
    assert!(rich.content.contains("ADA: The conversion works."));
    let rich_search = services
        .list_pastes(
            &anonymous,
            &PasteQuery {
                search: Some("conversion works".into()),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(rich_search.items.len(), 1);

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
            None,
        )
        .await
        .unwrap();
    let attachment = services
        .get_source(&owner, &cascade.id)
        .await
        .unwrap()
        .unwrap()
        .attachments
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(
        services
            .add_attachments(
                &owner,
                &cascade.id,
                &[("stale.txt".to_string(), "stale-file".to_string(), 5)],
                Some(1),
            )
            .await
            .unwrap_err()
            .message,
        "Paste revision changed"
    );
    assert_eq!(
        services
            .delete_attachment(&owner, &cascade.id, attachment.id, Some(1))
            .await
            .unwrap_err()
            .message,
        "Paste revision changed"
    );
    assert_eq!(
        services
            .delete_paste(&owner, &cascade.id, Some(1))
            .await
            .unwrap_err()
            .message,
        "Paste revision changed"
    );
    let unchanged = services
        .get_source(&owner, &cascade.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.revision, 2);
    assert_eq!(unchanged.attachments.len(), 1);
    assert_eq!(unchanged.attachments[0].filename, "file.txt");
    let attached = services
        .list_pastes(
            &owner,
            &PasteQuery {
                mine: Some(true),
                search: Some("file.txt".to_string()),
                content_kind: Some("text".to_string()),
                has_attachments: Some(true),
                min_reads: Some(0),
                max_reads: Some(0),
                min_size_bytes: Some(4),
                read_limit: Some("unlimited".to_string()),
                ..PasteQuery::default()
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(attached.items.len(), 1);
    assert_eq!(attached.items[0].attachment_count, 1);
    assert!(attached.items[0].size_bytes >= 4);
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
    sqlx::query(
        "INSERT INTO password_reset_tokens(user_id,token_hash,created_by_user_id,created_at,expires_at)
         VALUES(2,'expired-reset',1,1,1)",
    )
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
                (SELECT count(*) FROM invitations WHERE token_hash='expired-invitation') +
                (SELECT count(*) FROM password_reset_tokens WHERE token_hash='expired-reset')",
    )
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(expired_records, 0);

    let reset_key_scopes = vec!["paste:read".to_string()];
    let (_, reset_key_token) =
        api_keys::create(&repo, Some(2), "survives password reset", &reset_key_scopes)
            .await
            .unwrap();
    let old_reset = accounts::create_password_reset(&repo, 2, 1).await.unwrap();
    let reset = accounts::create_password_reset(&repo, 2, 1).await.unwrap();
    assert!(
        accounts::reset_password(&repo, &old_reset, "replacement password phrase")
            .await
            .is_err()
    );
    accounts::reset_password(&repo, &reset, "replacement password phrase")
        .await
        .unwrap();
    assert!(accounts::session_user(&repo, &session_token)
        .await
        .unwrap()
        .is_none());
    assert!(
        accounts::verify_user(&repo, "paste-owner", "replacement password phrase")
            .await
            .unwrap()
            .is_some()
    );
    assert!(api_keys::authenticate(&repo, &reset_key_token)
        .await
        .unwrap()
        .is_some());
    assert!(
        accounts::reset_password(&repo, &reset, "another replacement phrase")
            .await
            .is_err()
    );

    let managed = accounts::admin_user(&repo, 2).await.unwrap().unwrap();
    assert_eq!(managed.username, "paste-owner");
    assert!(managed.paste_count > 0);
    assert!(managed.api_key_count > 0);
    assert!(accounts::list_admin_users(&repo).await.unwrap().len() >= 2);

    for index in 0..5 {
        accounts::record_login_failure(
            &repo,
            "limited-account",
            &format!("account-client-{index}"),
        )
        .await
        .unwrap();
    }
    assert!(
        accounts::login_retry_after(&repo, "limited-account", "fresh-client")
            .await
            .unwrap()
            .is_some()
    );
    for index in 0..20 {
        accounts::record_login_failure(
            &repo,
            &format!("address-account-{index}"),
            "limited-address",
        )
        .await
        .unwrap();
    }
    assert!(
        accounts::login_retry_after(&repo, "fresh-account", "limited-address")
            .await
            .unwrap()
            .is_some()
    );
    for _ in 0..20 {
        accounts::record_invitation_failure(&repo, "limited-invitation-address")
            .await
            .unwrap();
    }
    assert!(
        accounts::invitation_retry_after(&repo, "limited-invitation-address")
            .await
            .unwrap()
            .is_some()
    );
    for _ in 0..20 {
        accounts::record_password_reset_failure(&repo, "limited-reset-address")
            .await
            .unwrap();
    }
    assert!(
        accounts::password_reset_retry_after(&repo, "limited-reset-address")
            .await
            .unwrap()
            .is_some()
    );

    for _ in 0..25 {
        accounts::create_session(&repo, 2, false).await.unwrap();
    }
    let capped_sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id=2 AND expires_at>$1")
            .bind(crate::time::unix_timestamp())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(capped_sessions, 20);

    accounts::delete_session(&repo, &session_token)
        .await
        .unwrap();
    assert!(accounts::session_user(&repo, &session_token)
        .await
        .unwrap()
        .is_none());
    assert!(services
        .delete_paste(&owner, &unlisted.id, None)
        .await
        .unwrap());
}
