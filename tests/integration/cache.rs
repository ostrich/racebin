use super::*;
use actix_web::{test, web, App};

#[actix_web::test]
async fn api_responses_are_not_cacheable() {
    let (repo, data_dir) = sqlite_repository("api-cache-policy").await;
    insert_user(&repo, 1, "cache-owner", "user").await;
    let services = PasteService::new(repo.clone());
    let owner = principal(1, "cache-owner", "user");
    let paste = services
        .create_paste(&owner, &paste_input("private cache fixture", "private"))
        .await
        .unwrap();
    let (_, token) = api_keys::create(&repo, Some(1), "reader", &["paste:read".into()])
        .await
        .unwrap();
    let attachment = services
        .add_attachments(
            &owner,
            &paste.id,
            &[attachment("file.txt", "fixture-file", 1)],
            None,
        )
        .await
        .unwrap()
        .remove(0);
    let directory = data_dir.join("attachments").join(&paste.id);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("fixture-file"), b"x").unwrap();
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(services))
            .configure(racebin::http::configure),
    )
    .await;
    // Cover discovery, account state, a denied action and an invalid bearer.
    for (path, bearer) in [
        ("/api/v1/openapi.json", None),
        ("/api/v1/session", None),
        ("/api/v1/account/api-keys", None),
        ("/api/v1/session", Some("rbk_invalid")),
    ] {
        let mut request = test::TestRequest::get().uri(path);
        if let Some(token) = bearer {
            request = request.insert_header(("Authorization", format!("Bearer {token}")));
        }
        let response = test::call_service(&app, request.to_request()).await;
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "private, no-store",
            "{path}"
        );
    }
    // NamedFile must not replace the policy with a cacheable download response.
    for path in [
        format!("/api/v1/pastes/{}/source", paste.id),
        format!("/api/v1/pastes/{}/attachments/{}", paste.id, attachment.id),
    ] {
        let response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&path)
                .insert_header(("Authorization", format!("Bearer {token}")))
                .to_request(),
        )
        .await;
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("cache-control").unwrap(),
            "private, no-store"
        );
    }
    let verified = accounts::verify_user(&repo, "cache-owner", "correct horse battery staple")
        .await
        .unwrap()
        .unwrap();
    let (session, csrf, _) = accounts::create_session(&repo, &verified, false)
        .await
        .unwrap();
    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/api/v1/account/api-keys")
            .cookie(actix_web::cookie::Cookie::new(
                accounts::SESSION_COOKIE,
                session,
            ))
            .insert_header(("X-CSRF-Token", csrf))
            .set_json(serde_json::json!({"name":"one-time secret","scopes":["paste:read"]}))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), 201);
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "private, no-store"
    );
    drop(app);
    drop(repo);
    std::fs::remove_dir_all(data_dir).unwrap();
}
