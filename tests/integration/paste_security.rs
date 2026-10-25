use super::*;
use actix_web::{test, web, App};
use serde_json::{json, Value};

pub(super) async fn paste_content_contract(repo: &Database) {
    racebin::instance::settings::initialize(repo, &racebin::args::ARGS)
        .await
        .unwrap();
    insert_user(repo, 201, "content-owner", "user").await;
    let owner = principal(201, "content-owner", "user");
    let services = PasteService::new(repo.clone());
    let private = services
        .create_paste(
            &owner,
            &PasteInput {
                content: Some("private-body-marker".into()),
                ..paste_input("private metadata", "private")
            },
        )
        .await
        .unwrap();
    let (list_key, list_token) =
        api_keys::create(repo, Some(201), "metadata only", &["paste:list".into()])
            .await
            .unwrap();
    let list_principal = Principal::ApiKey(list_key);
    let mine = PasteQuery {
        mine: Some(true),
        ..Default::default()
    };
    let listed = services
        .list_pastes(&list_principal, &mine, false)
        .await
        .unwrap();
    assert!(listed
        .items
        .iter()
        .find(|p| p.id == private.id)
        .unwrap()
        .content
        .is_empty());
    assert_eq!(
        services
            .list_pastes(
                &list_principal,
                &PasteQuery {
                    search: Some("private-body-marker".into()),
                    mine: Some(true),
                    ..Default::default()
                },
                false
            )
            .await
            .unwrap()
            .total_items,
        0
    );

    let limited = services
        .create_paste(
            &owner,
            &PasteInput {
                content: Some("limited-body-marker".into()),
                read_limit: Some(Some(3)),
                ..paste_input("limited metadata", "public")
            },
        )
        .await
        .unwrap();
    let search = PasteQuery {
        search: Some("limited-body-marker".into()),
        ..Default::default()
    };
    assert_eq!(
        services
            .list_pastes(&Principal::Anonymous, &search, false)
            .await
            .unwrap()
            .total_items,
        0
    );
    assert_eq!(
        services
            .list_pastes(&owner, &search, false)
            .await
            .unwrap()
            .total_items,
        1
    );
    assert_eq!(
        services
            .get_paste(&owner, &limited.id)
            .await
            .unwrap()
            .unwrap()
            .read_count,
        0
    );
    let read = services
        .read_paste(&Principal::Anonymous, &limited.id, Some("logical-read"))
        .await
        .unwrap()
        .unwrap();
    let grant = read.grant_token.unwrap();
    assert!(services
        .update_paste(
            &owner,
            &limited.id,
            &PasteInput {
                title: Some("stale mutation".into()),
                ..Default::default()
            },
            Some(i64::MAX)
        )
        .await
        .is_err());
    assert!(services
        .get_paste_with_grant(&limited.id, &grant)
        .await
        .unwrap()
        .is_some());
    services
        .update_paste(
            &owner,
            &limited.id,
            &PasteInput {
                content: Some("replacement-private-body".into()),
                visibility: Some("private".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    let capabilities: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM paste_read_grants WHERE paste_id=$1)
              + (SELECT count(*) FROM paste_read_receipts WHERE paste_id=$1)",
    )
    .bind(&limited.id)
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(capabilities, 0);
    assert!(services
        .get_paste_with_grant(&limited.id, &grant)
        .await
        .unwrap()
        .is_none());
    assert!(services
        .read_paste(&Principal::Anonymous, &limited.id, Some("logical-read"))
        .await
        .unwrap()
        .is_none());

    let files = services
        .create_paste(
            &owner,
            &PasteInput {
                read_limit: Some(Some(3)),
                ..paste_input("attachment grants", "public")
            },
        )
        .await
        .unwrap();
    let grant = services
        .read_paste(&Principal::Anonymous, &files.id, None)
        .await
        .unwrap()
        .unwrap()
        .grant_token
        .unwrap();
    let attachment = services
        .add_attachments(
            &owner,
            &files.id,
            &[attachment("test.txt", "test-file", 1)],
            None,
        )
        .await
        .unwrap()
        .remove(0);
    assert!(services
        .get_paste_with_grant(&files.id, &grant)
        .await
        .unwrap()
        .is_none());
    let grant = services
        .read_paste(&Principal::Anonymous, &files.id, None)
        .await
        .unwrap()
        .unwrap()
        .grant_token
        .unwrap();
    services
        .delete_attachment(&owner, &files.id, attachment.id, None)
        .await
        .unwrap();
    assert!(services
        .get_paste_with_grant(&files.id, &grant)
        .await
        .unwrap()
        .is_none());

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(services.clone()))
            .configure(racebin::http::configure),
    )
    .await;
    let listed = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/pastes?owner=me")
            .insert_header(("Authorization", format!("Bearer {list_token}")))
            .to_request(),
    )
    .await;
    assert_eq!(listed.status(), 200);
    let listed: Value = test::read_body_json(listed).await;
    let row = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == private.id)
        .unwrap();
    assert!(row["excerpt"].is_null());

    let (_, write_token) = api_keys::create(repo, Some(201), "uploader", &["paste:write".into()])
        .await
        .unwrap();
    let body = json!({"visibility":"private","body":{"format":"text","content":"original body"}});
    let request = || {
        test::TestRequest::post()
            .uri("/api/v1/pastes")
            .insert_header(("Authorization", format!("Bearer {write_token}")))
            .insert_header(("Idempotency-Key", "write-only-replay"))
            .set_json(&body)
            .to_request()
    };
    let created = test::call_service(&app, request()).await;
    assert_eq!(created.status(), 201);
    let created: Value = test::read_body_json(created).await;
    services
        .update_paste(
            &owner,
            created["id"].as_str().unwrap(),
            &PasteInput {
                content: Some("later owner secret".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
    let replayed = test::call_service(&app, request()).await;
    assert_eq!(replayed.status(), 403);
    let replayed: Value = test::read_body_json(replayed).await;
    assert!(!replayed.to_string().contains("later owner secret"));
}

#[actix_web::test]
async fn sqlite_paste_content_contract() {
    let (repo, data_dir) = sqlite_repository("content-security").await;
    paste_content_contract(&repo).await;
    drop(repo);
    std::fs::remove_dir_all(data_dir).unwrap();
}
