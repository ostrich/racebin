use super::*;

pub(super) async fn concurrency_contract(repo: Database) {
    sqlx::query("UPDATE users SET role='user' WHERE role='admin'")
        .execute(repo.pool())
        .await
        .unwrap();
    insert_user(&repo, 10, "concurrency-admin", "admin").await;
    insert_user(&repo, 11, "second-admin", "admin").await;
    let services = PasteService::new(repo.clone());
    let admin = principal(10, "concurrency-admin", "admin");

    let limited = PasteInput {
        read_limit: Some(Some(1)),
        ..paste_input("one read", "public")
    };
    let limited = services.create_paste(&admin, &limited).await.unwrap();
    let (left, right) = futures::join!(
        services.read_paste(&Principal::Anonymous, &limited.id, None),
        services.read_paste(&Principal::Anonymous, &limited.id, None)
    );
    assert_eq!(
        [left, right]
            .into_iter()
            .filter(|result| matches!(result, Ok(Some(_))))
            .count(),
        1
    );

    let invitation = accounts::create_invitation(&repo, 10, None).await.unwrap();
    let (left, right) = futures::join!(
        accounts::redeem_invitation(
            &repo,
            &invitation,
            "invitation-winner-a",
            "correct horse battery staple a"
        ),
        accounts::redeem_invitation(
            &repo,
            &invitation,
            "invitation-winner-b",
            "correct horse battery staple b"
        )
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);

    let reset = accounts::create_password_reset(&repo, 11, 10)
        .await
        .unwrap();
    let (left, right) = futures::join!(
        accounts::reset_password(&repo, &reset, "new concurrent password one"),
        accounts::reset_password(&repo, &reset, "new concurrent password two")
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);

    let (left, right) = futures::join!(
        accounts::set_enabled(&repo, 10, false),
        accounts::set_enabled(&repo, 11, false)
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);
    let enabled_admins: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin' AND enabled=1")
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(enabled_admins, 1);
    sqlx::query("UPDATE users SET enabled=1 WHERE id IN (10,11)")
        .execute(repo.pool())
        .await
        .unwrap();

    let (left, right) = futures::join!(
        accounts::set_role(&repo, 10, false),
        accounts::set_role(&repo, 11, false)
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);
    let admins: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE role='admin'")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(admins, 1);

    let paste = services
        .create_paste(&admin, &paste_input("concurrent attachments", "private"))
        .await
        .unwrap();
    let left_file = [attachment("left.txt", "left-store", 1)];
    let right_file = [attachment("right.txt", "right-store", 1)];
    let (left, right) = futures::join!(
        services.add_attachments(&admin, &paste.id, &left_file, None),
        services.add_attachments(&admin, &paste.id, &right_file, None)
    );
    assert!(left.is_ok());
    assert!(right.is_ok());
    let positions: Vec<i64> = sqlx::query_scalar(
        "SELECT sort_order FROM attachments WHERE paste_id=$1 ORDER BY sort_order",
    )
    .bind(paste.id)
    .fetch_all(repo.pool())
    .await
    .unwrap();
    assert_eq!(positions, vec![0, 1]);

    let paste = services
        .create_paste(&admin, &paste_input("revision race", "private"))
        .await
        .unwrap();
    let revision = paste.revision;
    let left_update = PasteInput {
        title: Some("left update".to_string()),
        ..PasteInput::default()
    };
    let right_update = PasteInput {
        title: Some("right update".to_string()),
        ..PasteInput::default()
    };
    let (left, right) = futures::join!(
        services.update_paste(&admin, &paste.id, &left_update, Some(revision)),
        services.update_paste(&admin, &paste.id, &right_update, Some(revision))
    );
    assert_eq!([left, right].into_iter().filter(Result::is_ok).count(), 1);
    let current = services
        .get_source(&admin, &paste.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.revision, revision + 1);
    assert!(matches!(
        current.title.as_str(),
        "left update" | "right update"
    ));

    let idempotent = paste_input("idempotent multipart", "private");
    let (left, right) = futures::join!(
        services.create_paste_idempotent(
            &admin,
            &idempotent,
            Some("shared-multipart-key"),
            "shared-request-hash",
            1,
        ),
        services.create_paste_idempotent(
            &admin,
            &idempotent,
            Some("shared-multipart-key"),
            "shared-request-hash",
            1,
        )
    );
    assert_eq!(
        [&left, &right]
            .into_iter()
            .filter(|result| result.is_ok())
            .count(),
        1
    );
    assert!([&left, &right].into_iter().any(|result| {
        result
            .as_ref()
            .is_err_and(|error| error.code == "idempotency_in_progress")
    }));
    let (paste, token) = [left, right]
        .into_iter()
        .find_map(|result| result.ok())
        .map(|(paste, replayed, token)| {
            assert!(!replayed);
            (paste, token.unwrap())
        })
        .unwrap();
    assert!(services
        .get_paste(&admin, &paste.id)
        .await
        .unwrap()
        .is_none());
    assert!(services
        .read_paste(&admin, &paste.id, None)
        .await
        .unwrap()
        .is_none());
    assert!(!services
        .list_pastes(&admin, &PasteQuery::default(), true)
        .await
        .unwrap()
        .items
        .iter()
        .any(|item| item.id == paste.id));
    services
        .add_creation_attachments(
            &admin,
            &paste.id,
            &[attachment("idempotent.txt", "idempotent-store", 1)],
        )
        .await
        .unwrap();
    let (recovered, was_recovered, recovered_token) = services
        .create_paste_idempotent(
            &admin,
            &idempotent,
            Some("shared-multipart-key"),
            "shared-request-hash",
            1,
        )
        .await
        .unwrap();
    assert_eq!(recovered.id, paste.id);
    assert!(was_recovered);
    assert!(recovered_token.is_none());
    assert!(services
        .complete_create_idempotency(&admin, "shared-multipart-key", &token)
        .await
        .unwrap());
    let (replayed, was_replayed, token) = services
        .create_paste_idempotent(
            &admin,
            &idempotent,
            Some("shared-multipart-key"),
            "shared-request-hash",
            1,
        )
        .await
        .unwrap();
    assert_eq!(replayed.id, paste.id);
    assert!(was_replayed);
    assert!(token.is_none());

    let (abandoned, _, _) = services
        .create_paste_idempotent(
            &admin,
            &paste_input("abandoned keyed multipart", "private"),
            Some("abandoned-multipart-key"),
            "abandoned-request-hash",
            1,
        )
        .await
        .unwrap();
    sqlx::query("UPDATE pastes SET created_at=1 WHERE id=$1")
        .bind(&abandoned.id)
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE idempotency_records SET expires_at=1
         WHERE operation='create_paste' AND paste_id=$1",
    )
    .bind(&abandoned.id)
    .execute(repo.pool())
    .await
    .unwrap();
    repo.purge_expired(racebin::time::unix_timestamp())
        .await
        .unwrap();
    let abandoned_rows: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM pastes WHERE id=$1) +
                (SELECT count(*) FROM idempotency_records WHERE paste_id=$1)",
    )
    .bind(&abandoned.id)
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(abandoned_rows, 0);

    let reservations = (0..20).map(|index| {
        let repo = repo.clone();
        async move {
            accounts::reserve_login_attempt(
                &repo,
                "concurrent-login-limit",
                &format!("concurrent-client-{index}"),
            )
            .await
            .unwrap()
        }
    });
    let reservations = futures::future::join_all(reservations).await;
    assert_eq!(
        reservations
            .iter()
            .filter(|retry_after| retry_after.is_none())
            .count(),
        5
    );
}
