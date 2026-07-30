#[cfg(test)]
mod tests {
    use crate::account::{self as accounts, api_keys};
    use crate::http::configure;
    use crate::http::files::{attachment_path, sanitize_upload_filename};
    use crate::repository::Repository;
    use crate::services::{now, PasteInput, Principal, Services};
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::{json, Value};
    use std::path::Path;

    #[actix_web::test]
    async fn attachment_paths_reject_traversal_and_absolute_components() {
        let root = Path::new("/tmp/racebin-test");
        assert!(attachment_path(root, "safe-slug", "safe-name").is_ok());
        assert!(attachment_path(root, "..", "safe-name").is_err());
        assert!(attachment_path(root, "safe-slug", "../secret").is_err());
        assert!(attachment_path(root, "safe-slug", "/etc/passwd").is_err());
        assert!(attachment_path(root, "safe-slug", ".hidden").is_err());
    }

    #[actix_web::test]
    async fn upload_filenames_are_reduced_to_safe_components() {
        assert_eq!(sanitize_upload_filename("hello.txt"), "hello.txt");
        assert_eq!(sanitize_upload_filename("../hello.txt"), "hello.txt");
        assert_eq!(
            sanitize_upload_filename(r"C:\Users\someone\hello.txt"),
            "hello.txt"
        );
        assert_eq!(
            sanitize_upload_filename(" bad:<name>?.txt "),
            "bad__name__.txt"
        );
        assert_eq!(sanitize_upload_filename("..."), "");

        let long = sanitize_upload_filename(&"é".repeat(200));
        assert!(long.len() <= 255);
        assert!(long.is_char_boundary(long.len()));
    }

    #[actix_web::test]
    async fn http_auth_authorization_visibility_and_file_lifecycle() {
        let data_dir = std::env::temp_dir().join(format!("racebin-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &data_dir).await.unwrap();
        repository.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES(1,'http-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("correct horse battery staple").unwrap())
        .bind(now())
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES(2,'other-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("another correct horse battery staple").unwrap())
        .bind(now())
        .execute(repository.pool())
        .await
        .unwrap();
        let user = accounts::User {
            id: 1,
            username: "http-user".to_string(),
            role: "user".to_string(),
            enabled: true,
            force_password_change: false,
        };
        let principal = Principal::User(accounts::SessionUser {
            user,
            csrf_token: "direct".to_string(),
        });
        let services = Services::new(repository.clone());
        let input = |title: &str, access: &str| PasteInput {
            title: Some(title.to_string()),
            content: Some(format!("{title} content")),
            kind: Some("text".to_string()),
            syntax: Some("none".to_string()),
            access: Some(access.to_string()),
            expiration: None,
            burn_after_reads: None,
        };
        let public = services
            .create_paste(&principal, &input("public", "public"))
            .await
            .unwrap();
        let unlisted = services
            .create_paste(&principal, &input("unlisted", "unlisted"))
            .await
            .unwrap();
        let owner = services
            .create_paste(&principal, &input("owner", "owner"))
            .await
            .unwrap();
        let other_principal = Principal::User(accounts::SessionUser {
            user: accounts::User {
                id: 2,
                username: "other-user".to_string(),
                role: "user".to_string(),
                enabled: true,
                force_password_change: false,
            },
            csrf_token: "other".to_string(),
        });
        let other_owner = services
            .create_paste(&other_principal, &input("other owner", "owner"))
            .await
            .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(services.clone()))
                .configure(configure),
        )
        .await;

        for (slug, expected) in [
            (&public.slug, StatusCode::OK),
            (&unlisted.slug, StatusCode::OK),
            (&owner.slug, StatusCode::NOT_FOUND),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v2/pastes/{slug}"))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
        }

        let login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/session")
                .set_json(json!({
                    "username": "http-user",
                    "password": "correct horse battery staple"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(login.status(), StatusCode::OK);
        let cookie = login.response().cookies().next().unwrap().into_owned();
        let login_body: Value = test::read_body_json(login).await;
        let csrf = login_body["csrf_token"].as_str().unwrap().to_string();

        let without_csrf = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .cookie(cookie.clone())
                .set_json(json!({"title":"blocked","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(without_csrf.status(), StatusCode::FORBIDDEN);

        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"title":"files","content":"body","access":"owner"}))
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created: Value = test::read_body_json(created).await;
        let slug = created["slug"].as_str().unwrap();

        let boundary = "racebin-test-boundary";
        let multipart = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let uploaded = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v2/pastes/{slug}/files"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header((
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(multipart)
                .to_request(),
        )
        .await;
        assert_eq!(uploaded.status(), StatusCode::CREATED);
        let uploaded: Value = test::read_body_json(uploaded).await;
        let file_id = uploaded["items"][0]["id"].as_i64().unwrap();

        let downloaded = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(test::read_body(downloaded).await.as_ref(), b"hello");

        let scopes = vec!["paste:read".to_string()];
        let (_, read_token) = api_keys::create(&repository, Some(1), "read only", &scopes)
            .await
            .unwrap();
        let key_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(key_read.status(), StatusCode::OK);
        let key_write = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .set_json(json!({"title":"forbidden","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(key_write.status(), StatusCode::FORBIDDEN);

        let positive_get_scopes = [
            (
                "paste:read",
                format!("/api/v2/pastes/{}", owner.slug),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "paste:list",
                "/api/v2/pastes?mine=true".to_string(),
                owner.slug.as_str(),
                StatusCode::NOT_FOUND,
            ),
            (
                "paste:admin",
                "/api/v2/admin/pastes".to_string(),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "user:admin",
                "/api/v2/admin/users".to_string(),
                "/api/v2/admin/invites",
                StatusCode::FORBIDDEN,
            ),
            (
                "invite:admin",
                "/api/v2/admin/invites".to_string(),
                "/api/v2/admin/api-keys",
                StatusCode::FORBIDDEN,
            ),
            (
                "key:admin",
                "/api/v2/admin/api-keys".to_string(),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
        ];
        for (scope, uri, denied_uri, denied_status) in positive_get_scopes {
            let (_, token) = api_keys::create(&repository, Some(1), scope, &[scope.to_string()])
                .await
                .unwrap();
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&uri)
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK, "scope {scope}");
            let denied_uri = if scope == "paste:list" {
                format!("/api/v2/pastes/{denied_uri}")
            } else {
                denied_uri.to_string()
            };
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&denied_uri)
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), denied_status, "isolated scope {scope}");
        }

        let forbidden_gets = [
            "/api/v2/pastes?mine=true",
            "/api/v2/admin/pastes",
            "/api/v2/admin/users",
            "/api/v2/admin/invites",
            "/api/v2/admin/api-keys",
        ];
        for uri in forbidden_gets {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(uri)
                    .insert_header(("Authorization", format!("Bearer {read_token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
        }

        let (_, write_token) =
            api_keys::create(&repository, Some(1), "write", &["paste:write".to_string()])
                .await
                .unwrap();
        let write_allowed = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"written by key"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_allowed.status(), StatusCode::NO_CONTENT);
        let write_create_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"key-created","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_create_allowed.status(), StatusCode::CREATED);
        let cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"not allowed"}))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_write.status(), StatusCode::FORBIDDEN);
        let write_cannot_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(write_cannot_read.status(), StatusCode::NOT_FOUND);

        let disposable = services
            .create_paste(&principal, &input("delete target", "owner"))
            .await
            .unwrap();
        let read_cannot_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", disposable.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(read_cannot_delete.status(), StatusCode::FORBIDDEN);
        let (_, delete_token) = api_keys::create(
            &repository,
            Some(1),
            "delete",
            &["paste:delete".to_string()],
        )
        .await
        .unwrap();
        let delete_allowed = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", disposable.slug))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(delete_allowed.status(), StatusCode::NO_CONTENT);
        let delete_other_denied = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(delete_other_denied.status(), StatusCode::FORBIDDEN);

        let (_, paste_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "paste admin",
            &["paste:admin".to_string()],
        )
        .await
        .unwrap();
        let cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_read.status(), StatusCode::NOT_FOUND);
        let admin_cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_read.status(), StatusCode::OK);
        let admin_cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .set_json(json!({"title":"administered"}))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_write.status(), StatusCode::OK);

        let (_, delegating_token) = api_keys::create(
            &repository,
            Some(1),
            "delegator",
            &["key:admin".to_string(), "paste:read".to_string()],
        )
        .await
        .unwrap();
        let delegation_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated read","scopes":["paste:read"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_allowed.status(), StatusCode::CREATED);
        let delegation_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated write","scopes":["paste:write"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_denied.status(), StatusCode::FORBIDDEN);
        let browser_admin_scope_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"name":"admin attempt","scopes":["user:admin"]}))
                .to_request(),
        )
        .await;
        assert_eq!(browser_admin_scope_denied.status(), StatusCode::FORBIDDEN);

        let (disabled_key, disabled_token) = api_keys::create(
            &repository,
            Some(1),
            "disabled",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        api_keys::set_enabled(&repository, disabled_key.id, false)
            .await
            .unwrap();
        let disabled_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {disabled_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(disabled_response.status(), StatusCode::UNAUTHORIZED);
        let (_, owner_disabled_token) = api_keys::create(
            &repository,
            Some(2),
            "disabled owner",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        sqlx::query("UPDATE app_user SET enabled=0 WHERE id=2")
            .execute(repository.pool())
            .await
            .unwrap();
        let disabled_owner_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {owner_disabled_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(disabled_owner_response.status(), StatusCode::UNAUTHORIZED);
        sqlx::query("UPDATE app_user SET enabled=1 WHERE id=2")
            .execute(repository.pool())
            .await
            .unwrap();

        let (other_key, _) = api_keys::create(
            &repository,
            Some(2),
            "other user's key",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        let (_, key_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "key boundary",
            &["key:admin".to_string()],
        )
        .await
        .unwrap();
        let account_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/account/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(account_delete_other.status(), StatusCode::NOT_FOUND);
        let admin_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/admin/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_delete_other.status(), StatusCode::NO_CONTENT);
        let admin_cross_owner_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_delete.status(), StatusCode::NO_CONTENT);

        let deleted = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        let missing = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let logout = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri("/api/v2/session")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        let session = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v2/session")
                .cookie(cookie)
                .to_request(),
        )
        .await;
        let session: Value = test::read_body_json(session).await;
        assert_eq!(session["authenticated"], false);

        drop(app);
        drop(repository);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
