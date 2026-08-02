use super::*;
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::http::pastes::list_pastes,
        crate::http::pastes::create_paste,
        crate::http::pastes::get_paste,
        crate::http::pastes::get_paste_source,
        crate::http::pastes::read_paste,
        crate::http::pastes::update_paste,
        crate::http::pastes::delete_paste,
        crate::http::pastes::convert_paste_content,
        crate::http::attachments::upload_attachments,
        crate::http::attachments::get_attachment,
        crate::http::attachments::delete_attachment,
        crate::http::attachments::get_archive,
        crate::http::attachments::get_qr,
        crate::http::folders::list_folders,
        crate::http::folders::create_folder,
        crate::http::folders::rename_folder,
        crate::http::folders::delete_folder,
        crate::http::folders::move_pastes,
        crate::http::account::get_session,
        crate::http::account::login,
        crate::http::account::logout,
        crate::http::account::change_password,
        crate::http::account::reset_password,
        crate::http::account::redeem_invitation,
        crate::http::keys::list_keys,
        crate::http::keys::create_key,
        crate::http::keys::update_key,
        crate::http::keys::delete_key,
        crate::http::admin::admin_users,
        crate::http::admin::admin_user,
        crate::http::admin::admin_pastes,
        crate::http::admin::admin_update_user,
        crate::http::admin::admin_create_password_reset,
        crate::http::admin::admin_revoke_user_sessions,
        crate::http::admin::admin_revoke_user_keys,
        crate::http::admin::admin_invitations,
        crate::http::admin::admin_create_invitation,
        crate::http::admin::admin_revoke_invitation,
        crate::http::admin::admin_keys,
        crate::http::admin::admin_update_key,
        crate::http::admin::admin_delete_key,
        api_root,
        get_openapi,
        get_capabilities,
        get_languages,
        get_health,
        get_readiness
    ),
    components(schemas(
        crate::http::dto::BodyInput,
        crate::http::dto::BodyOutput,
        crate::http::dto::CreatePasteRequest,
        crate::http::dto::UpdatePasteRequest,
        crate::http::dto::PasteResource,
        crate::http::dto::PasteSummary,
        crate::http::dto::PastePage,
        crate::http::dto::Pagination,
        crate::http::dto::AttachmentResource,
        crate::http::errors::ProblemDetails
    )),
    modifiers(&Security),
    tags(
        (name = "pastes"),
        (name = "attachments"),
        (name = "folders"),
        (name = "account"),
        (name = "api keys"),
        (name = "administration"),
        (name = "discovery")
    )
)]
struct ApiDoc;

struct Security;

impl Modify for Security {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{
            ApiKey, ApiKeyValue, Http, HttpAuthScheme, SecurityScheme,
        };
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        components.add_security_scheme(
            "sessionCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("racebin_session"))),
        );
        openapi.info.title = "Racebin API".into();
        openapi.info.version = env!("CARGO_PKG_VERSION").into();
        openapi.servers = Some(vec![utoipa::openapi::Server::new("/api/v1")]);
        normalize_problem_media_types(openapi);
        add_scope_extensions(openapi);
    }
}

fn normalize_problem_media_types(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::RefOr;

    for item in openapi.paths.paths.values_mut() {
        for operation in [
            &mut item.get,
            &mut item.post,
            &mut item.patch,
            &mut item.delete,
            &mut item.put,
        ]
        .into_iter()
        .flatten()
        {
            for response in operation.responses.responses.values_mut() {
                let RefOr::T(response) = response else {
                    continue;
                };
                let is_problem = response
                    .content
                    .get("application/json")
                    .and_then(|content| content.schema.as_ref())
                    .is_some_and(|schema| {
                        matches!(schema, RefOr::Ref(reference) if reference.ref_location == "#/components/schemas/ProblemDetails")
                    });
                if is_problem {
                    let content = response.content.shift_remove("application/json").unwrap();
                    response
                        .content
                        .insert("application/problem+json".into(), content);
                }
            }
        }
    }
}

fn add_scope_extensions(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::extensions::Extensions;
    for item in openapi.paths.paths.values_mut() {
        for operation in [
            &mut item.get,
            &mut item.post,
            &mut item.patch,
            &mut item.delete,
            &mut item.put,
        ]
        .into_iter()
        .flatten()
        {
            let scopes = operation_scopes(operation.operation_id.as_deref().unwrap_or(""));
            if !scopes.is_empty() {
                operation.extensions = Some(Extensions::from_iter([(
                    "x-racebin-scopes",
                    serde_json::json!(scopes),
                )]));
            }
        }
    }
}

fn operation_scopes(operation_id: &str) -> &'static [&'static str] {
    match operation_id {
        "list_pastes" | "list_folders" => &["paste:list"],
        "get_paste" | "get_paste_source" | "read_paste" | "get_attachment" | "get_archive"
        | "get_qr" => &["paste:read"],
        "create_paste"
        | "update_paste"
        | "convert_paste_content"
        | "upload_attachments"
        | "create_folder"
        | "rename_folder"
        | "delete_folder"
        | "move_pastes"
        | "delete_attachment" => &["paste:write"],
        "delete_paste" => &["paste:delete"],
        "list_keys" | "create_key" | "update_key" | "delete_key" => &["api_key:manage"],
        "admin_pastes" => &["paste:manage"],
        "admin_users"
        | "admin_user"
        | "admin_update_user"
        | "admin_create_password_reset"
        | "admin_revoke_user_sessions" => &["user:manage"],
        "admin_invitations" | "admin_create_invitation" | "admin_revoke_invitation" => {
            &["invitation:manage"]
        }
        "admin_keys" | "admin_update_key" | "admin_delete_key" | "admin_revoke_user_keys" => {
            &["api_key:manage"]
        }
        _ => &[],
    }
}

#[derive(Serialize, utoipa::ToSchema)]
struct Capabilities {
    site_name: String,
    server_version: &'static str,
    api_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    web_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_base_url: Option<String>,
    plain_home_enabled: bool,
    max_attachment_size_bytes: usize,
    max_attachments_per_paste: usize,
    attachments_enabled: bool,
    qr_codes_enabled: bool,
    formats: [&'static str; 2],
    visibility_modes: [&'static str; 3],
    authentication_methods: [&'static str; 2],
    paste_create_media_types: [&'static str; 6],
    attachment_upload_media_types: [&'static str; 1],
    scopes: Vec<ScopeDescription>,
    max_title_characters: usize,
    max_content_size_bytes: usize,
    max_page_size: u32,
    minimum_password_characters: usize,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ScopeDescription {
    id: &'static str,
    description: &'static str,
}

#[derive(Serialize, utoipa::ToSchema)]
struct Language {
    id: &'static str,
    label: &'static str,
    aliases: &'static [&'static str],
}

const LANGUAGES: &[Language] = &[
    Language {
        id: "plaintext",
        label: "Plain text",
        aliases: &["text", "txt"],
    },
    Language {
        id: "bash",
        label: "Bash / Shell",
        aliases: &["sh", "shell", "zsh"],
    },
    Language {
        id: "c",
        label: "C",
        aliases: &[],
    },
    Language {
        id: "cpp",
        label: "C++",
        aliases: &["c++"],
    },
    Language {
        id: "csharp",
        label: "C#",
        aliases: &["cs", "c#"],
    },
    Language {
        id: "css",
        label: "CSS",
        aliases: &[],
    },
    Language {
        id: "go",
        label: "Go",
        aliases: &["golang"],
    },
    Language {
        id: "html",
        label: "HTML",
        aliases: &["htm"],
    },
    Language {
        id: "java",
        label: "Java",
        aliases: &[],
    },
    Language {
        id: "javascript",
        label: "JavaScript",
        aliases: &["js", "jsx"],
    },
    Language {
        id: "json",
        label: "JSON",
        aliases: &[],
    },
    Language {
        id: "markdown",
        label: "Markdown",
        aliases: &["md"],
    },
    Language {
        id: "python",
        label: "Python",
        aliases: &["py"],
    },
    Language {
        id: "ruby",
        label: "Ruby",
        aliases: &["rb"],
    },
    Language {
        id: "rust",
        label: "Rust",
        aliases: &["rs"],
    },
    Language {
        id: "sql",
        label: "SQL",
        aliases: &[],
    },
    Language {
        id: "typescript",
        label: "TypeScript",
        aliases: &["ts", "tsx"],
    },
    Language {
        id: "xml",
        label: "XML",
        aliases: &["svg"],
    },
    Language {
        id: "yaml",
        label: "YAML",
        aliases: &["yml"],
    },
];

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(api_root)
        .service(get_openapi)
        .service(get_capabilities)
        .service(get_languages)
        .service(get_health)
        .service(get_readiness);
}

#[utoipa::path(
    get, path = "/", tag = "discovery",
    responses((status = 200, description = "API discovery links", body = crate::http::contract::ApiRootResponse)),
    security(())
)]
#[get("")]
async fn api_root() -> impl Responder {
    HttpResponse::Ok().json(contract::ApiRootResponse {
        name: "Racebin API",
        version: env!("CARGO_PKG_VERSION"),
        openapi_url: "/api/v1/openapi.json",
        capabilities_url: "/api/v1/capabilities",
        languages_url: "/api/v1/languages",
    })
}

#[utoipa::path(
    get, path = "/openapi.json", tag = "discovery",
    responses((status = 200, description = "OpenAPI 3.1 contract", body = Object)),
    security(())
)]
#[get("/openapi.json")]
async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(ApiDoc::openapi())
}

#[utoipa::path(get, path = "/capabilities", tag = "discovery", responses((status = 200, description = "Runtime features, limits, and authorization scopes", body = Capabilities)), security(()))]
#[get("/capabilities")]
async fn get_capabilities() -> impl Responder {
    let (web_base_url, api_base_url) = canonical_base_urls(ARGS.public_url.as_ref());
    HttpResponse::Ok().json(Capabilities {
        site_name: ARGS.site_name.clone().unwrap_or_else(|| "Racebin".into()),
        server_version: env!("CARGO_PKG_VERSION"),
        api_version: "v1",
        web_base_url,
        api_base_url,
        plain_home_enabled: ARGS.plain_home,
        max_attachment_size_bytes: ARGS.max_attachment_size_mb * 1024 * 1024,
        max_attachments_per_paste: 32,
        attachments_enabled: ARGS.attachments_enabled,
        qr_codes_enabled: ARGS.qr_codes,
        formats: ["text", "rich_text"],
        visibility_modes: ["public", "unlisted", "private"],
        authentication_methods: ["browser_session", "bearer_api_key"],
        paste_create_media_types: [
            "application/json",
            "text/plain",
            "text/markdown",
            "text/html",
            "application/x-www-form-urlencoded",
            "multipart/form-data",
        ],
        attachment_upload_media_types: ["multipart/form-data"],
        scopes: vec![
            ScopeDescription {
                id: "paste:read",
                description: "Read paste content available to the key owner",
            },
            ScopeDescription {
                id: "paste:write",
                description: "Create and update pastes, folders, and attachments",
            },
            ScopeDescription {
                id: "paste:delete",
                description: "Delete owned pastes",
            },
            ScopeDescription {
                id: "paste:list",
                description: "List and search non-public pastes and folders",
            },
            ScopeDescription {
                id: "paste:manage",
                description: "Administratively inspect and manage all pastes",
            },
            ScopeDescription {
                id: "user:manage",
                description: "Administratively manage users and password recovery",
            },
            ScopeDescription {
                id: "invitation:manage",
                description: "Create, list, and revoke invitations",
            },
            ScopeDescription {
                id: "api_key:manage",
                description: "Manage API keys, subject to ownership and delegation rules",
            },
        ],
        max_title_characters: 200,
        max_content_size_bytes: 2 * 1024 * 1024,
        max_page_size: 100,
        minimum_password_characters: 12,
    })
}

fn canonical_base_urls(public_url: Option<&url::Url>) -> (Option<String>, Option<String>) {
    let Some(public_url) = public_url else {
        return (None, None);
    };
    let web_base_url = public_url.as_str().trim_end_matches('/').to_string();
    let api_base_url = format!("{web_base_url}/api/v1");
    (Some(web_base_url), Some(api_base_url))
}

#[utoipa::path(get, path = "/languages", tag = "discovery", responses((status = 200, description = "Accepted syntax identifiers and aliases", body = [Language])), security(()))]
#[get("/languages")]
async fn get_languages() -> impl Responder {
    HttpResponse::Ok().json(LANGUAGES)
}

#[utoipa::path(get, path = "/health", tag = "discovery", responses((status = 204, description = "Process is running")), security(()))]
#[get("/health")]
async fn get_health() -> HttpResponse {
    health().await
}

#[utoipa::path(get, path = "/readiness", tag = "discovery", responses((status = 204, description = "Database is available"), (status = 503, description = "Database is unavailable")), security(()))]
#[get("/readiness")]
async fn get_readiness(services: web::Data<PasteService>) -> HttpResponse {
    ready(services).await
}

pub(super) async fn health() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

pub(super) async fn ready(services: web::Data<PasteService>) -> HttpResponse {
    match sqlx::query("SELECT 1")
        .execute(services.storage.pool())
        .await
    {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_contract_contains_every_supported_route() {
        let document = ApiDoc::openapi();
        let paths = document.paths.paths;
        for path in [
            "/",
            "/openapi.json",
            "/pastes",
            "/pastes/{paste_id}",
            "/pastes/{paste_id}/source",
            "/pastes/{paste_id}/reads",
            "/content-conversions",
            "/capabilities",
            "/languages",
            "/health",
            "/readiness",
            "/session",
            "/account/password",
            "/password-resets/{token}",
            "/invitations/{token}/redeem",
            "/account/api-keys",
            "/account/api-keys/{id}",
            "/folders",
            "/folders/{folder_id}",
            "/pastes/{paste_id}/attachments",
            "/pastes/{paste_id}/attachments/{attachment_id}",
            "/pastes/{paste_id}/archive",
            "/pastes/{paste_id}/qr",
            "/admin/users",
            "/admin/users/{id}",
            "/admin/users/{id}/password-reset",
            "/admin/users/{id}/sessions",
            "/admin/users/{id}/api-keys",
            "/admin/pastes",
            "/admin/invitations",
            "/admin/invitations/{id}",
            "/admin/api-keys",
            "/admin/api-keys/{id}",
        ] {
            assert!(paths.contains_key(path), "missing OpenAPI path {path}");
        }
        assert!(!paths.contains_key("/pastes/{paste_id}/consume"));
        assert!(!paths.contains_key("/pastes/{paste_id}/raw"));
        let actual_operation_ids = paths
            .values()
            .flat_map(|item| {
                [
                    item.get.as_ref(),
                    item.post.as_ref(),
                    item.patch.as_ref(),
                    item.delete.as_ref(),
                    item.put.as_ref(),
                ]
                .into_iter()
                .flatten()
                .filter_map(|operation| operation.operation_id.as_deref())
            })
            .collect::<std::collections::HashSet<_>>();
        let expected_operation_ids = [
            "list_pastes",
            "create_paste",
            "get_paste",
            "get_paste_source",
            "read_paste",
            "update_paste",
            "delete_paste",
            "convert_paste_content",
            "upload_attachments",
            "get_attachment",
            "delete_attachment",
            "get_archive",
            "get_qr",
            "list_folders",
            "create_folder",
            "rename_folder",
            "delete_folder",
            "move_pastes",
            "get_session",
            "login",
            "logout",
            "change_password",
            "reset_password",
            "redeem_invitation",
            "list_keys",
            "create_key",
            "update_key",
            "delete_key",
            "admin_users",
            "admin_user",
            "admin_pastes",
            "admin_update_user",
            "admin_create_password_reset",
            "admin_revoke_user_sessions",
            "admin_revoke_user_keys",
            "admin_invitations",
            "admin_create_invitation",
            "admin_revoke_invitation",
            "admin_keys",
            "admin_update_key",
            "admin_delete_key",
            "api_root",
            "get_openapi",
            "get_capabilities",
            "get_languages",
            "get_health",
            "get_readiness",
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
        assert_eq!(actual_operation_ids, expected_operation_ids);
        for (method, path) in [
            ("get", "/session"),
            ("post", "/session"),
            ("delete", "/session"),
            ("get", "/folders"),
            ("post", "/folders"),
            ("patch", "/folders/{folder_id}"),
            ("delete", "/folders/{folder_id}"),
            ("get", "/account/api-keys"),
            ("post", "/account/api-keys"),
            ("patch", "/account/api-keys/{id}"),
            ("delete", "/account/api-keys/{id}"),
            ("post", "/pastes/{paste_id}/attachments"),
            ("get", "/pastes/{paste_id}/attachments/{attachment_id}"),
            ("delete", "/pastes/{paste_id}/attachments/{attachment_id}"),
            ("get", "/admin/users"),
            ("patch", "/admin/users/{id}"),
            ("get", "/admin/invitations"),
            ("post", "/admin/invitations"),
            ("get", "/health"),
            ("get", "/readiness"),
        ] {
            let item = paths.get(path).unwrap();
            let present = match method {
                "get" => item.get.is_some(),
                "post" => item.post.is_some(),
                "patch" => item.patch.is_some(),
                "delete" => item.delete.is_some(),
                _ => false,
            };
            assert!(present, "missing OpenAPI operation {method} {path}");
        }
    }

    #[test]
    fn every_operation_has_responses_and_resolvable_schema_references() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let paths = value["paths"].as_object().unwrap();
        let mut operation_ids = std::collections::HashSet::new();
        for (path, item) in paths {
            for method in ["get", "post", "patch", "delete", "put"] {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let responses = operation["responses"]
                    .as_object()
                    .unwrap_or_else(|| panic!("{method} {path} has no response object"));
                assert!(!responses.is_empty(), "{method} {path} has no responses");
                let operation_id = operation["operationId"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{method} {path} has no operationId"));
                assert!(
                    operation_ids.insert(operation_id),
                    "duplicate operationId {operation_id}"
                );
                for (status, response) in responses {
                    let Some(content) = response["content"].as_object() else {
                        continue;
                    };
                    let has_problem_schema = content.values().any(|media| {
                        media["schema"]["$ref"].as_str()
                            == Some("#/components/schemas/ProblemDetails")
                    });
                    if has_problem_schema {
                        assert_eq!(
                            content.len(),
                            1,
                            "{method} {path} response {status} mixes problem details with another representation"
                        );
                        assert!(
                            content.contains_key("application/problem+json"),
                            "{method} {path} response {status} has the wrong problem media type"
                        );
                    }
                }
            }
        }

        let schemas = value["components"]["schemas"].as_object().unwrap();
        fn check_refs(
            value: &serde_json::Value,
            schemas: &serde_json::Map<String, serde_json::Value>,
        ) {
            match value {
                serde_json::Value::Object(object) => {
                    if let Some(reference) = object.get("$ref").and_then(|value| value.as_str()) {
                        if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                            assert!(
                                schemas.contains_key(name),
                                "unresolved schema reference {reference}"
                            );
                        }
                    }
                    for value in object.values() {
                        check_refs(value, schemas);
                    }
                }
                serde_json::Value::Array(array) => {
                    for value in array {
                        check_refs(value, schemas);
                    }
                }
                _ => {}
            }
        }
        check_refs(&value, schemas);
    }

    #[test]
    fn protected_operations_and_protocol_headers_are_explicit() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let operation =
            |method: &str, path: &str| -> &serde_json::Value { &value["paths"][path][method] };
        let anonymous_allowed = [
            "list_pastes",
            "get_paste",
            "read_paste",
            "get_attachment",
            "get_archive",
            "get_qr",
            "get_session",
            "login",
            "reset_password",
            "redeem_invitation",
            "api_root",
            "get_openapi",
            "get_capabilities",
            "get_languages",
            "get_health",
            "get_readiness",
        ]
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
        for (path, item) in value["paths"].as_object().unwrap() {
            for method in ["get", "post", "patch", "delete", "put"] {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let operation_id = operation["operationId"].as_str().unwrap();
                let security = operation["security"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{method} {path} does not declare security"));
                assert!(!security.is_empty(), "{method} {path} has empty security");
                if !anonymous_allowed.contains(operation_id) {
                    assert!(
                        security.iter().all(|requirement| {
                            requirement
                                .as_object()
                                .is_some_and(|requirement| !requirement.is_empty())
                        }),
                        "protected operation {method} {path} permits anonymous access"
                    );
                }
            }
        }

        let parameter_names = |method: &str, path: &str| {
            operation(method, path)["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|parameter| parameter["name"].as_str())
                .collect::<std::collections::HashSet<_>>()
        };
        let list_parameters = parameter_names("get", "/pastes");
        for expected in [
            "q",
            "page",
            "page_size",
            "owner",
            "visibility",
            "format",
            "folder_id",
            "created_after",
            "min_size_bytes",
            "sort",
            "direction",
        ] {
            assert!(
                list_parameters.contains(expected),
                "missing list parameter {expected}"
            );
        }
        assert!(parameter_names("post", "/pastes").contains("Idempotency-Key"));
        assert!(parameter_names("post", "/pastes/{paste_id}/reads").contains("Idempotency-Key"));
        for (method, path) in [
            ("patch", "/pastes/{paste_id}"),
            ("delete", "/pastes/{paste_id}"),
            ("post", "/pastes/{paste_id}/attachments"),
            ("delete", "/pastes/{paste_id}/attachments/{attachment_id}"),
        ] {
            assert!(parameter_names(method, path).contains("If-Match"));
        }
        assert!(operation("post", "/pastes")["x-racebin-scopes"].is_array());
        assert!(operation("get", "/admin/users")["x-racebin-scopes"].is_array());
    }

    #[test]
    fn every_operation_advertises_its_authoritative_api_key_scopes() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for (path, item) in value["paths"].as_object().unwrap() {
            for method in ["get", "post", "patch", "delete", "put"] {
                let Some(operation) = item.get(method) else {
                    continue;
                };
                let operation_id = operation["operationId"].as_str().unwrap();
                let expected = operation_scopes(operation_id);
                let actual = operation["x-racebin-scopes"]
                    .as_array()
                    .map(|scopes| {
                        scopes
                            .iter()
                            .map(|scope| scope.as_str().unwrap())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                assert_eq!(
                    actual, expected,
                    "incorrect API-key scopes for {method} {path} ({operation_id})"
                );
            }
        }
    }

    #[test]
    fn every_timestamp_schema_uses_rfc3339_date_time_strings() {
        fn contains_date_time(schema: &serde_json::Value) -> bool {
            schema["format"].as_str() == Some("date-time")
                || schema
                    .as_object()
                    .into_iter()
                    .flat_map(|object| object.values())
                    .any(contains_date_time)
                || schema
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(contains_date_time)
        }

        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let schemas = value["components"]["schemas"].as_object().unwrap();
        let mut timestamps = 0;
        for (schema_name, schema) in schemas {
            let Some(properties) = schema["properties"].as_object() else {
                continue;
            };
            for (property_name, property) in properties {
                if property_name.ends_with("_at") {
                    timestamps += 1;
                    assert!(
                        contains_date_time(property),
                        "{schema_name}.{property_name} is not an RFC 3339 date-time string"
                    );
                }
            }
        }
        assert!(timestamps > 0, "no timestamp properties were checked");
    }

    #[test]
    fn rich_text_sanitization_is_part_of_the_generated_contract() {
        let value = serde_json::to_value(ApiDoc::openapi()).unwrap();
        for schema_name in ["BodyInput", "BodyOutput"] {
            let schema = serde_json::to_string(&value["components"]["schemas"][schema_name])
                .unwrap()
                .to_ascii_lowercase();
            assert!(
                schema.contains("sanitiz") && schema.contains("html"),
                "{schema_name} does not explain the rich-text sanitization contract"
            );
        }
    }

    #[test]
    fn discovery_advertises_stable_versions_media_types_and_canonical_urls() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        let properties = document["components"]["schemas"]["Capabilities"]["properties"]
            .as_object()
            .unwrap();
        for field in [
            "server_version",
            "api_version",
            "web_base_url",
            "api_base_url",
            "paste_create_media_types",
            "attachment_upload_media_types",
        ] {
            assert!(properties.contains_key(field), "missing capability {field}");
        }

        let public_url = url::Url::parse("https://example.com/racebin/").unwrap();
        let (web, api) = canonical_base_urls(Some(&public_url));
        assert_eq!(web.as_deref(), Some("https://example.com/racebin"));
        assert_eq!(api.as_deref(), Some("https://example.com/racebin/api/v1"));
        assert_eq!(canonical_base_urls(None), (None, None));
    }
}
