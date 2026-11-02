use super::*;

/// Run with both backends. Read-only pool sessions prove that warm bearer
/// authentication does not merely issue an UPDATE whose WHERE clause matches
/// no rows: even that attempted write would fail in this mode.
async fn read_only_connections(repo: &Database, enabled: bool) {
    let mut connections = Vec::new();
    for _ in 0..repo.pool().options().get_max_connections() {
        connections.push(repo.pool().acquire().await.unwrap());
    }
    let statement = match (repo.kind(), enabled) {
        (DatabaseKind::Sqlite, true) => "PRAGMA query_only=ON",
        (DatabaseKind::Sqlite, false) => "PRAGMA query_only=OFF",
        (DatabaseKind::Postgres, true) => "SET default_transaction_read_only=on",
        (DatabaseKind::Postgres, false) => "SET default_transaction_read_only=off",
    };
    for connection in &mut connections {
        sqlx::query(statement)
            .execute(&mut **connection)
            .await
            .unwrap();
    }
}

pub(super) async fn hot_path_contract(repo: &Database) {
    let owner_id = 4100;
    insert_user(repo, owner_id, "query-reader", "user").await;
    let before = accounts::admin_summary(repo).await.unwrap();
    let verified = accounts::verify_user(repo, "query-reader", "correct horse battery staple")
        .await
        .unwrap()
        .unwrap();
    let (session, _, _) = accounts::create_session(repo, &verified, false)
        .await
        .unwrap();
    let scopes = vec![
        "paste:list".into(),
        "paste:read".into(),
        "paste:write".into(),
    ];
    let (warm, warm_token) = api_keys::create(repo, Some(owner_id), "Warm reader", &scopes)
        .await
        .unwrap();
    let (_, cold_token) = api_keys::create(repo, Some(owner_id), "Cold reader", &scopes)
        .await
        .unwrap();
    api_keys::authenticate(repo, &warm_token)
        .await
        .unwrap()
        .unwrap();
    let last_used = api_keys::get(repo, warm.id)
        .await
        .unwrap()
        .unwrap()
        .last_used_at;
    assert!(last_used.is_some());

    let values = ["($1,$2,$3)"; 20].join(",");
    for kind in ["invitation_address", "password_reset_address"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO auth_attempts(kind,subject,occurred_at) VALUES {values}"
        )))
        .bind(kind)
        .bind("query-client")
        .bind(racebin::time::unix_timestamp() - 901)
        .execute(repo.pool())
        .await
        .unwrap();
    }

    read_only_connections(repo, true).await;
    assert_eq!(
        accounts::invitation_retry_after(repo, "query-client")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        accounts::password_reset_retry_after(repo, "query-client")
            .await
            .unwrap(),
        None
    );
    let authenticated = api_keys::authenticate(repo, &warm_token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(authenticated.scopes, scopes);
    assert_eq!(
        authenticated.owner_username.as_deref(),
        Some("query-reader")
    );
    assert_eq!(authenticated.last_used_at, last_used);
    assert!(accounts::session_user(repo, &session)
        .await
        .unwrap()
        .is_some());
    // Validate the test restriction: a genuinely due timestamp write fails.
    assert!(api_keys::authenticate(repo, &cold_token).await.is_err());
    read_only_connections(repo, false).await;
    assert!(api_keys::authenticate(repo, &cold_token)
        .await
        .unwrap()
        .is_some());

    sqlx::query("UPDATE api_keys SET last_used_at=1 WHERE id=$1")
        .bind(warm.id)
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE sessions SET last_used_at=1 WHERE user_id=$1")
        .bind(owner_id)
        .execute(repo.pool())
        .await
        .unwrap();
    api_keys::authenticate(repo, &warm_token)
        .await
        .unwrap()
        .unwrap();
    accounts::session_user(repo, &session)
        .await
        .unwrap()
        .unwrap();
    assert!(
        api_keys::get(repo, warm.id)
            .await
            .unwrap()
            .unwrap()
            .last_used_at
            .unwrap()
            > 1
    );
    let session_used: i64 =
        sqlx::query_scalar("SELECT last_used_at FROM sessions WHERE user_id=$1")
            .bind(owner_id)
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert!(session_used > 1);

    let owner = principal(owner_id, "query-reader", "user");
    let services = PasteService::new(repo.clone());
    let folder = services.create_folder(&owner, "Examples").await.unwrap();
    for index in 0..3 {
        let paste = services
            .create_paste(
                &owner,
                &PasteInput {
                    content: Some("éλ".into()),
                    folder_id: Some((index < 2).then_some(folder.id)),
                    ..paste_input(&format!("query example {index}"), "private")
                },
            )
            .await
            .unwrap();
        if index == 0 {
            services
                .add_attachments(
                    &owner,
                    &paste.id,
                    &[attachment("example.txt", "example-store", 7)],
                    None,
                )
                .await
                .unwrap();
        }
    }
    for page in 1..=4 {
        let result = services
            .list_pastes(
                &owner,
                &PasteQuery {
                    mine: Some(true),
                    page: Some(page),
                    page_size: Some(1),
                    ..Default::default()
                },
                false,
            )
            .await
            .unwrap();
        assert_eq!(result.total_items, 3);
        assert_eq!(result.items.len(), usize::from(page <= 3));
        assert!(result
            .items
            .iter()
            .all(|paste| paste.owner_username.is_none()));
    }
    insert_user(repo, 4101, "query-admin", "admin").await;
    let administrative = services
        .list_pastes(
            &principal(4101, "query-admin", "admin"),
            &PasteQuery {
                owner_id: Some(owner_id),
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap();
    assert_eq!(administrative.total_items, 3);
    for paste in &administrative.items {
        assert_eq!(paste.owner_username.as_deref(), Some("query-reader"));
        assert!(serde_json::to_value(paste)
            .unwrap()
            .get("owner_username")
            .is_none());
    }
    let folders = services.list_folders(&owner).await.unwrap();
    assert_eq!((folders.total_count, folders.unfiled_count), (3, 1));
    assert_eq!(folders.items[0].paste_count, 2);

    accounts::create_invitation(repo, owner_id, Some("later invitation"))
        .await
        .unwrap();
    accounts::create_invitation(repo, owner_id, Some("soon invitation"))
        .await
        .unwrap();
    sqlx::query("UPDATE invitations SET expires_at=$2 WHERE created_by_user_id=$1 AND comment='soon invitation'")
        .bind(owner_id).bind(racebin::time::unix_timestamp() + 3600).execute(repo.pool()).await.unwrap();
    sqlx::query("UPDATE users SET password_change_required=1 WHERE id=$1")
        .bind(owner_id)
        .execute(repo.pool())
        .await
        .unwrap();
    let after = accounts::admin_summary(repo).await.unwrap();
    assert_eq!(after.user_count, before.user_count + 1);
    assert_eq!(after.paste_count, before.paste_count + 3);
    assert_eq!(
        after.storage_bytes,
        before.storage_bytes + 3 * "éλ".len() as i64 + 7
    );
    assert_eq!(after.active_session_count, before.active_session_count + 1);
    assert_eq!(
        after.active_invitation_count,
        before.active_invitation_count + 2
    );
    assert_eq!(
        after.expiring_invitation_count,
        before.expiring_invitation_count + 1
    );
    assert_eq!(
        after.password_change_required_count,
        before.password_change_required_count + 1
    );
}
