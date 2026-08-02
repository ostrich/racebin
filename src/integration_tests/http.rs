#[cfg(test)]
mod tests {
    use crate::account::{self as accounts, api_keys};
    use crate::http::attachments::{attachment_path, sanitize_upload_filename};
    use crate::http::configure;
    use crate::repository::Repository;
    use crate::services::{PasteInput, PasteService, Principal};
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::{json, Value};
    use std::path::Path;

    #[actix_web::test]
    async fn attachment_paths_reject_traversal_and_absolute_components() {
        let root = Path::new("/tmp/racebin-test");
        assert!(attachment_path(root, "safe-id", "safe-name").is_ok());
        assert!(attachment_path(root, "..", "safe-name").is_err());
        assert!(attachment_path(root, "safe-id", "../secret").is_err());
        assert!(attachment_path(root, "safe-id", "/etc/passwd").is_err());
        assert!(attachment_path(root, "safe-id", ".hidden").is_err());
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
    async fn http_auth_authorization_visibility_and_attachment_lifecycle() {
        let data_dir = std::env::temp_dir().join(format!("racebin-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &data_dir).await.unwrap();
        repository.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO users(id,username,password_hash,role,enabled,password_change_required,created_at)
             VALUES(1,'http-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("correct horse battery staple").unwrap())
        .bind(crate::time::unix_timestamp())
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO users(id,username,password_hash,role,enabled,password_change_required,created_at)
             VALUES(2,'other-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("another correct horse battery staple").unwrap())
        .bind(crate::time::unix_timestamp())
        .execute(repository.pool())
        .await
        .unwrap();
        let user = accounts::User {
            id: 1,
            username: "http-user".to_string(),
            role: "user".to_string(),
            enabled: true,
            password_change_required: false,
        };
        let principal = Principal::Session(accounts::SessionUser {
            user,
            csrf_token: "direct".to_string(),
        });
        let services = PasteService::new(repository.clone());
        let input = |title: &str, visibility: &str| PasteInput {
            title: Some(title.to_string()),
            content: Some(format!("{title} content")),
            document: None,
            content_kind: Some("text".to_string()),
            language: Some("plaintext".to_string()),
            visibility: Some(visibility.to_string()),
            expires_at: None,
            read_limit: None,
            folder_id: None,
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
            .create_paste(&principal, &input("private", "private"))
            .await
            .unwrap();
        let other_principal = Principal::Session(accounts::SessionUser {
            user: accounts::User {
                id: 2,
                username: "other-user".to_string(),
                role: "user".to_string(),
                enabled: true,
                password_change_required: false,
            },
            csrf_token: "other".to_string(),
        });
        let other_owner = services
            .create_paste(&other_principal, &input("other owner", "private"))
            .await
            .unwrap();
        let mut final_read_input = input("final read", "unlisted");
        final_read_input.read_limit = Some(Some(1));
        let final_read = services
            .create_paste(&principal, &final_read_input)
            .await
            .unwrap();
        let final_read_directory = data_dir.join("attachments").join(&final_read.id);
        std::fs::create_dir_all(&final_read_directory).unwrap();
        std::fs::write(
            final_read_directory.join("final-read-file"),
            b"downloadable",
        )
        .unwrap();
        let final_read_attachment = services
            .add_attachments(
                &principal,
                &final_read.id,
                &[("download.txt".into(), "final-read-file".into(), 12)],
                Some(1),
            )
            .await
            .unwrap()
            .remove(0);

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(services.clone()))
                .configure(configure),
        )
        .await;

        let consumed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/pastes/{}/reads", final_read.id))
                .insert_header(("Accept", "text/plain"))
                .to_request(),
        )
        .await;
        assert_eq!(consumed.status(), StatusCode::OK);
        let grant = consumed
            .headers()
            .get("Read-Token")
            .expect("final read returns an attachment grant")
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(
            test::read_body(consumed).await.as_ref(),
            b"final read content"
        );
        let granted_download = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/pastes/{}/attachments/{}?read_token={grant}",
                    final_read.id, final_read_attachment.id
                ))
                .to_request(),
        )
        .await;
        assert_eq!(granted_download.status(), StatusCode::OK);
        assert_eq!(
            test::read_body(granted_download).await.as_ref(),
            b"downloadable"
        );

        let invalid_page_size = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/pastes?page_size=0")
                .to_request(),
        )
        .await;
        assert_eq!(invalid_page_size.status(), StatusCode::BAD_REQUEST);

        for (id, expected) in [
            (&public.id, StatusCode::OK),
            (&unlisted.id, StatusCode::OK),
            (&owner.id, StatusCode::NOT_FOUND),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v1/pastes/{id}"))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
        }

        let login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/session")
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

        let converted = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/content-conversions")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({
                    "source":{"format":"text","content":"INT. LAB - NIGHT\n\nADA\nWe should go."},
                    "target_format":"rich_text"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(converted.status(), StatusCode::OK);
        let converted: Value = test::read_body_json(converted).await;
        assert_eq!(converted["body"]["format"], "rich_text");
        assert!(converted["body"]["content"]
            .as_str()
            .unwrap()
            .contains("INT. LAB"));

        let unsafe_document = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({
                    "body":{"format":"rich_text","content":"<script>alert(1)</script>"}
                }))
                .to_request(),
        )
        .await;
        assert_eq!(unsafe_document.status(), StatusCode::CREATED);
        let sanitized: Value = test::read_body_json(unsafe_document).await;
        assert!(!sanitized["body"]["content"]
            .as_str()
            .unwrap()
            .contains("script"));

        let created_folder = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/folders")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"name":"Scripts"}))
                .to_request(),
        )
        .await;
        assert_eq!(created_folder.status(), StatusCode::CREATED);
        let created_folder: Value = test::read_body_json(created_folder).await;
        assert!(created_folder["created_at"].as_str().is_some());
        let folder_id = created_folder["id"].as_i64().unwrap();
        let moved = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri("/api/v1/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"ids":[public.id],"folder_id":folder_id}))
                .to_request(),
        )
        .await;
        assert_eq!(moved.status(), StatusCode::OK);
        let moved: Value = test::read_body_json(moved).await;
        assert_eq!(moved["pastes"][0]["id"], public.id);
        let moved_etag = moved["pastes"][0]["etag"].as_str().unwrap().to_string();
        let owner_view = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", public.id))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(
            owner_view.headers().get("ETag").unwrap().to_str().unwrap(),
            moved_etag
        );
        let owner_view: Value = test::read_body_json(owner_view).await;
        assert_eq!(owner_view["folder_id"], folder_id);
        let public_view = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", public.id))
                .to_request(),
        )
        .await;
        let public_view: Value = test::read_body_json(public_view).await;
        assert!(public_view["folder_id"].is_null());

        let without_csrf = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .cookie(cookie.clone())
                .set_json(json!({"title":"blocked","body":{"format":"text","content":"body"}}))
                .to_request(),
        )
        .await;
        assert_eq!(without_csrf.status(), StatusCode::FORBIDDEN);

        let paste_count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM pastes")
            .fetch_one(repository.pool())
            .await
            .unwrap();
        let expired = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({
                    "title":"expired",
                    "body":{"format":"text","content":"body"},
                    "expires_at":"1970-01-01T00:00:00Z"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(expired.status(), StatusCode::BAD_REQUEST);
        let empty_update = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/pastes/{}", owner.id))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header(("If-Match", "*"))
                .set_json(json!({}))
                .to_request(),
        )
        .await;
        assert_eq!(empty_update.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let paste_count_after: i64 = sqlx::query_scalar("SELECT count(*) FROM pastes")
            .fetch_one(repository.pool())
            .await
            .unwrap();
        assert_eq!(paste_count_after, paste_count_before);

        let created_at = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"title":"files","body":{"format":"text","content":"body"},"visibility":"private"}))
                .to_request(),
        )
        .await;
        assert_eq!(created_at.status(), StatusCode::CREATED);
        let created_at: Value = test::read_body_json(created_at).await;
        let id = created_at["id"].as_str().unwrap();

        let boundary = "racebin-test-boundary";
        let multipart = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let uploaded = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/pastes/{id}/attachments"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header(("If-Match", "*"))
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
        let attachment_id = uploaded["items"][0]["id"].as_i64().unwrap();

        let downloaded = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{id}/attachments/{attachment_id}"))
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
                .uri(&format!("/api/v1/pastes/{}", owner.id))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(key_read.status(), StatusCode::OK);
        let key_write = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .set_json(json!({"title":"forbidden","body":{"format":"text","content":"body"}}))
                .to_request(),
        )
        .await;
        assert_eq!(key_write.status(), StatusCode::FORBIDDEN);
        let key_folders = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/folders")
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(key_folders.status(), StatusCode::FORBIDDEN);

        let positive_get_scopes = [
            (
                "paste:read",
                format!("/api/v1/pastes/{}", owner.id),
                "/api/v1/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "paste:list",
                "/api/v1/pastes?owner=me".to_string(),
                owner.id.as_str(),
                StatusCode::NOT_FOUND,
            ),
            (
                "paste:manage",
                "/api/v1/admin/pastes".to_string(),
                "/api/v1/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "user:manage",
                "/api/v1/admin/users".to_string(),
                "/api/v1/admin/invitations",
                StatusCode::FORBIDDEN,
            ),
            (
                "invitation:manage",
                "/api/v1/admin/invitations".to_string(),
                "/api/v1/admin/api-keys",
                StatusCode::FORBIDDEN,
            ),
            (
                "api_key:manage",
                "/api/v1/admin/api-keys".to_string(),
                "/api/v1/admin/users",
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
                format!("/api/v1/pastes/{denied_uri}")
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
            "/api/v1/pastes?owner=me",
            "/api/v1/admin/pastes",
            "/api/v1/admin/users",
            "/api/v1/admin/invitations",
            "/api/v1/admin/api-keys",
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
        let raw_created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes?title=Raw%20upload&visibility=unlisted&language=js")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/plain"))
                .insert_header(("Accept", "text/plain"))
                .insert_header(("Idempotency-Key", "raw-upload-contract"))
                .set_payload("const answer = 42;")
                .to_request(),
        )
        .await;
        assert_eq!(raw_created.status(), StatusCode::CREATED);
        assert!(raw_created.headers().contains_key("Location"));
        assert!(raw_created.headers().contains_key("ETag"));
        let raw_url = String::from_utf8(test::read_body(raw_created).await.to_vec()).unwrap();
        assert!(raw_url.starts_with("/pastes/"));

        for (content_type, body) in [
            (
                "application/json",
                r#"{"body":{"format":"text","content":"body"}}"#,
            ),
            ("application/x-www-form-urlencoded", "content=body"),
            ("multipart/form-data; boundary=empty", "--empty--\r\n"),
        ] {
            let rejected_query = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/v1/pastes?title=query-title")
                    .insert_header(("Authorization", format!("Bearer {write_token}")))
                    .insert_header(("Content-Type", content_type))
                    .set_payload(body)
                    .to_request(),
            )
            .await;
            assert_eq!(rejected_query.status(), StatusCode::BAD_REQUEST);
        }

        let ambiguous_raw = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes?content=query-content")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/plain"))
                .set_payload("body content")
                .to_request(),
        )
        .await;
        assert_eq!(ambiguous_raw.status(), StatusCode::BAD_REQUEST);

        let markdown = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/markdown"))
                .set_payload("# Heading")
                .to_request(),
        )
        .await;
        assert_eq!(markdown.status(), StatusCode::CREATED);
        let markdown: Value = test::read_body_json(markdown).await;
        assert_eq!(markdown["body"]["format"], "text");
        assert_eq!(markdown["body"]["language"], "markdown");

        let contradictory_html = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes?language=javascript")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/html"))
                .set_payload("<p>HTML</p>")
                .to_request(),
        )
        .await;
        assert_eq!(
            contradictory_html.status(),
            StatusCode::UNPROCESSABLE_ENTITY
        );

        for body in [
            json!({ "title": null }),
            json!({
                "expires_at": "2030-01-01T00:00:00Z",
                "expires_in": 60
            }),
        ] {
            let invalid_create = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/v1/pastes")
                    .insert_header(("Authorization", format!("Bearer {write_token}")))
                    .set_json(body)
                    .to_request(),
            )
            .await;
            assert!(
                matches!(
                    invalid_create.status(),
                    StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
                ),
                "unexpected create status {}",
                invalid_create.status()
            );
        }

        let raw_replay = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes?title=Raw%20upload&visibility=unlisted&language=js")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/plain"))
                .insert_header(("Accept", "text/plain"))
                .insert_header(("Idempotency-Key", "raw-upload-contract"))
                .set_payload("const answer = 42;")
                .to_request(),
        )
        .await;
        assert_eq!(raw_replay.status(), StatusCode::CREATED);
        assert_eq!(
            raw_replay.headers().get("Idempotency-Replayed").unwrap(),
            "true"
        );
        assert_eq!(
            String::from_utf8(test::read_body(raw_replay).await.to_vec()).unwrap(),
            raw_url
        );
        let idempotency_conflict = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes?title=Different")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Content-Type", "text/plain"))
                .insert_header(("Idempotency-Key", "raw-upload-contract"))
                .set_payload("different content")
                .to_request(),
        )
        .await;
        assert_eq!(idempotency_conflict.status(), StatusCode::CONFLICT);

        let recovery_boundary = "racebin-recovery-boundary";
        let recovery_body = format!(
            "--{recovery_boundary}\r\nContent-Disposition: form-data; name=\"content\"\r\n\r\nrecovery body\r\n--{recovery_boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"recovery.txt\"\r\nContent-Type: text/plain\r\n\r\nrecovery file\r\n--{recovery_boundary}--\r\n"
        );
        let create_recovery = || {
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("Idempotency-Key", "multipart-recovery-contract"))
                .insert_header((
                    "Content-Type",
                    format!("multipart/form-data; boundary={recovery_boundary}"),
                ))
                .set_payload(recovery_body.clone())
                .to_request()
        };
        let recovery_created = test::call_service(&app, create_recovery()).await;
        if recovery_created.status() != StatusCode::CREATED {
            let status = recovery_created.status();
            let body = test::read_body(recovery_created).await;
            let latest: Option<(String, Option<i64>)> = sqlx::query_as(
                "SELECT id,owner_id FROM pastes ORDER BY created_at DESC,id DESC LIMIT 1",
            )
            .fetch_optional(repository.pool())
            .await
            .unwrap();
            panic!(
                "multipart recovery create returned {status}: {}; latest={latest:?}",
                String::from_utf8_lossy(&body)
            );
        }
        let recovery_created: Value = test::read_body_json(recovery_created).await;
        let recovery_id = recovery_created["id"].as_str().unwrap();
        sqlx::query("DELETE FROM attachments WHERE paste_id=$1")
            .bind(recovery_id)
            .execute(repository.pool())
            .await
            .unwrap();
        let recovery_directory = data_dir.join("attachments").join(recovery_id);
        std::fs::write(recovery_directory.join("orphaned-crash-file"), b"orphan").unwrap();
        let recovered = test::call_service(&app, create_recovery()).await;
        assert_eq!(recovered.status(), StatusCode::CREATED);
        assert_eq!(
            recovered.headers().get("Idempotency-Replayed").unwrap(),
            "true"
        );
        let recovered: Value = test::read_body_json(recovered).await;
        assert_eq!(recovered["attachments"].as_array().unwrap().len(), 1);
        assert!(!recovery_directory.join("orphaned-crash-file").exists());

        let write_allowed = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/pastes/{}", owner.id))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .insert_header(("If-Match", "*"))
                .set_json(json!({"title":"written by key"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_allowed.status(), StatusCode::OK);
        let write_create_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/pastes")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"key-created","body":{"format":"text","content":"body"}}))
                .to_request(),
        )
        .await;
        assert_eq!(write_create_allowed.status(), StatusCode::CREATED);
        let write_folder_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/folders")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"name":"API folder"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_folder_allowed.status(), StatusCode::CREATED);
        let write_folder_list_denied = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/folders")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(write_folder_list_denied.status(), StatusCode::FORBIDDEN);
        let (_, list_token) =
            api_keys::create(&repository, Some(1), "list", &["paste:list".to_string()])
                .await
                .unwrap();
        let list_folder_allowed = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/folders")
                .insert_header(("Authorization", format!("Bearer {list_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(list_folder_allowed.status(), StatusCode::OK);
        let read_conversion_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/content-conversions")
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .set_json(json!({
                    "source":{"format":"text","content":"example"},
                    "target_format":"rich_text"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(read_conversion_denied.status(), StatusCode::FORBIDDEN);
        let write_conversion_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/content-conversions")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({
                    "source":{"format":"text","content":"example"},
                    "target_format":"rich_text"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(write_conversion_allowed.status(), StatusCode::OK);
        let cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"not allowed"}))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_write.status(), StatusCode::FORBIDDEN);
        let write_cannot_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", owner.id))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(write_cannot_read.status(), StatusCode::NOT_FOUND);

        let disposable = services
            .create_paste(&principal, &input("delete target", "private"))
            .await
            .unwrap();
        let read_cannot_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/pastes/{}", disposable.id))
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
                .uri(&format!("/api/v1/pastes/{}", disposable.id))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .insert_header(("If-Match", "*"))
                .to_request(),
        )
        .await;
        assert_eq!(delete_allowed.status(), StatusCode::NO_CONTENT);
        let delete_other_denied = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(delete_other_denied.status(), StatusCode::FORBIDDEN);

        let (_, paste_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "paste admin",
            &["paste:manage".to_string()],
        )
        .await
        .unwrap();
        let cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_read.status(), StatusCode::NOT_FOUND);
        let admin_cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .insert_header(("If-Match", "*"))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_read.status(), StatusCode::OK);
        let admin_cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .insert_header(("If-Match", "*"))
                .set_json(json!({"title":"administered"}))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_write.status(), StatusCode::OK);

        let (_, delegating_token) = api_keys::create(
            &repository,
            Some(1),
            "delegator",
            &["api_key:manage".to_string(), "paste:read".to_string()],
        )
        .await
        .unwrap();
        let delegation_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated read","scopes":["paste:read"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_allowed.status(), StatusCode::CREATED);
        let delegation_allowed: Value = test::read_body_json(delegation_allowed).await;
        assert!(delegation_allowed["key"]["created_at"].as_str().is_some());
        let delegation_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated write","scopes":["paste:write"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_denied.status(), StatusCode::FORBIDDEN);
        let browser_admin_scope_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/account/api-keys")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header(("If-Match", "*"))
                .set_json(json!({"name":"admin attempt","scopes":["user:manage"]}))
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
                .uri(&format!("/api/v1/pastes/{}", owner.id))
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
        sqlx::query("UPDATE users SET enabled=0 WHERE id=2")
            .execute(repository.pool())
            .await
            .unwrap();
        let disabled_owner_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {owner_disabled_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(disabled_owner_response.status(), StatusCode::UNAUTHORIZED);
        sqlx::query("UPDATE users SET enabled=1 WHERE id=2")
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
            &["api_key:manage".to_string()],
        )
        .await
        .unwrap();
        let account_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/account/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(account_delete_other.status(), StatusCode::NOT_FOUND);
        let admin_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/admin/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_delete_other.status(), StatusCode::NO_CONTENT);
        let admin_cross_owner_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/pastes/{}", other_owner.id))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .insert_header(("If-Match", "*"))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_delete.status(), StatusCode::NO_CONTENT);

        let deleted = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v1/pastes/{id}/attachments/{attachment_id}"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header(("If-Match", "*"))
                .to_request(),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        assert!(deleted.headers().contains_key("ETag"));
        let missing = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/pastes/{id}/attachments/{attachment_id}"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let (_, invitation_token) = api_keys::create(
            &repository,
            Some(1),
            "invitation administration",
            &["invitation:manage".to_string()],
        )
        .await
        .unwrap();
        let invitation = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/admin/invitations")
                .insert_header(("Authorization", format!("Bearer {invitation_token}")))
                .insert_header(("Host", "attacker.example"))
                .to_request(),
        )
        .await;
        assert_eq!(invitation.status(), StatusCode::CREATED);
        let invitation: Value = test::read_body_json(invitation).await;
        assert!(invitation["url"]
            .as_str()
            .unwrap()
            .starts_with("/invitations/"));
        assert!(!invitation["url"]
            .as_str()
            .unwrap()
            .contains("attacker.example"));
        let invitation_list = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/admin/invitations")
                .insert_header(("Authorization", format!("Bearer {invitation_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(invitation_list.status(), StatusCode::OK);
        let invitation_list: Value = test::read_body_json(invitation_list).await;
        assert!(invitation_list[0]["expires_at"].as_str().is_some());

        let (_, user_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "user timestamp",
            &["user:manage".to_string()],
        )
        .await
        .unwrap();
        let admin_users = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/admin/users")
                .insert_header(("Authorization", format!("Bearer {user_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_users.status(), StatusCode::OK);
        let admin_users: Value = test::read_body_json(admin_users).await;
        assert!(admin_users[0]["created_at"].as_str().is_some());

        for _ in 0..5 {
            let failed_login = test::call_service(
                &app,
                test::TestRequest::post()
                    .uri("/api/v1/session")
                    .set_json(
                        json!({"username":"rate-limited-user","password":"incorrect password"}),
                    )
                    .to_request(),
            )
            .await;
            assert_eq!(failed_login.status(), StatusCode::UNAUTHORIZED);
        }
        let limited_login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/session")
                .set_json(json!({"username":"rate-limited-user","password":"incorrect password"}))
                .to_request(),
        )
        .await;
        assert_eq!(limited_login.status(), StatusCode::TOO_MANY_REQUESTS);
        assert!(limited_login.headers().contains_key("Retry-After"));

        let logout = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri("/api/v1/session")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        let session = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/session")
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
