use super::*;

pub(super) async fn concurrency_contract(repo: Repository) {
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
