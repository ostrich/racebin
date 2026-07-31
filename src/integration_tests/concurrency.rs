use super::*;

pub(super) async fn concurrency_contract(repo: Repository) {
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
        services.consume_paste(&Principal::Anonymous, &limited.id),
        services.consume_paste(&Principal::Anonymous, &limited.id)
    );
    assert_eq!(
        [left, right]
            .into_iter()
            .filter(|result| matches!(result, Ok(Some(_))))
            .count(),
        1
    );

    let invitation = accounts::create_invitation(&repo, 10).await.unwrap();
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
    let left_file = [("left.txt".to_string(), "left-store".to_string(), 1)];
    let right_file = [("right.txt".to_string(), "right-store".to_string(), 1)];
    let (left, right) = futures::join!(
        services.add_attachments(&admin, &paste.id, &left_file),
        services.add_attachments(&admin, &paste.id, &right_file)
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
}
