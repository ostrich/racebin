use super::*;
use utoipa::{Modify, OpenApi};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::http::pastes::list_pastes,
        crate::http::pastes::create_paste,
        crate::http::pastes::get_paste,
        crate::http::pastes::get_paste_raw,
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
        crate::http::account::reauthenticate,
        crate::http::account::change_password,
        crate::http::account::reset_password,
        crate::http::account::redeem_invitation,
        crate::http::api_key_routes::list_keys,
        crate::http::api_key_routes::create_key,
        crate::http::api_key_routes::update_key,
        crate::http::api_key_routes::delete_key,
        crate::http::admin::admin_users,
        crate::http::admin::admin_summary,
        crate::http::admin::admin_user,
        crate::http::admin::admin_pastes,
        crate::http::admin::admin_update_user,
        crate::http::admin::admin_update_user_role,
        crate::http::admin::admin_transfer_ownership,
        crate::http::admin::admin_settings,
        crate::http::admin::admin_replace_settings,
        crate::http::admin::admin_audit_events,
        crate::http::admin::admin_create_password_reset,
        crate::http::admin::admin_revoke_user_sessions,
        crate::http::admin::admin_revoke_user_keys,
        crate::http::admin::admin_invitations,
        crate::http::admin::admin_create_invitation,
        crate::http::admin::admin_update_invitation,
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
        crate::http::contract::BodyInput,
        crate::http::contract::BodyOutput,
        crate::http::contract::CreatePasteRequest,
        crate::http::contract::UpdatePasteRequest,
        crate::http::contract::PasteMetadataResource,
        crate::http::contract::PasteResource,
        crate::http::contract::PasteSummary,
        crate::http::contract::PastePage,
        crate::http::contract::Pagination,
        crate::http::contract::AttachmentResource,
        crate::http::pastes::OwnerFilter,
        crate::http::pastes::ExpirationFilter,
        crate::http::pastes::ReadLimitFilter,
        crate::http::pastes::PasteSort,
        crate::http::pastes::SortDirection,
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

pub(crate) fn openapi_document() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

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
        normalize_optional_header_schemas(openapi);
        refine_component_schemas(openapi);
        refine_parameter_schemas(openapi);
        constrain_numeric_path_ids(openapi);
        add_scope_extensions(openapi);
    }
}

fn refine_component_schemas(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::schema::{AdditionalProperties, OneOf, Schema, SchemaType, Type};
    use utoipa::openapi::RefOr;

    let Some(components) = openapi.components.as_mut() else {
        return;
    };
    for name in [
        "CreatePasteRequest",
        "FlatCreateRequest",
        "MultipartCreateRequest",
        "UpdatePasteRequest",
    ] {
        if let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) {
            if let Some(RefOr::T(Schema::Object(title))) = schema.properties.get_mut("title") {
                title.max_length = Some(crate::limits::MAX_TITLE_CHARACTERS);
            }
        }
    }
    if let Some(RefOr::T(Schema::OneOf(body))) = components.schemas.get_mut("BodyInput") {
        for branch in &mut body.items {
            if let RefOr::T(Schema::Object(schema)) = branch {
                schema.additional_properties =
                    Some(Box::new(AdditionalProperties::FreeForm(false)));
            }
        }
    }
    if let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut("Pagination") {
        if let Some(RefOr::T(Schema::Object(page_size))) = schema.properties.get_mut("page_size") {
            page_size.maximum = Some(crate::limits::MAX_PAGE_SIZE.into());
        }
    }
    for name in ["UpdatePasteRequest", "UserUpdate"] {
        if let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) {
            schema.min_properties = Some(1);
        }
    }
    for (name, properties) in [
        (
            "CreatePasteRequest",
            &[
                "title",
                "body",
                "visibility",
                "folder_id",
                "expires_at",
                "expires_in",
                "read_limit",
            ][..],
        ),
        (
            "FlatCreateRequest",
            &[
                "title",
                "format",
                "content",
                "language",
                "visibility",
                "folder_id",
                "expires_at",
                "expires_in",
                "read_limit",
            ][..],
        ),
        (
            "MultipartCreateRequest",
            &[
                "title",
                "format",
                "content",
                "language",
                "visibility",
                "folder_id",
                "expires_at",
                "expires_in",
                "read_limit",
            ][..],
        ),
        ("UpdatePasteRequest", &["title", "body", "visibility"][..]),
        ("UserUpdate", &["enabled", "role"][..]),
    ] {
        let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) else {
            continue;
        };
        for property in properties {
            if let Some(value) = schema.properties.get_mut(*property) {
                remove_null(value);
            }
        }
    }
    for name in [
        "CreatePasteRequest",
        "FlatCreateRequest",
        "MultipartCreateRequest",
    ] {
        let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) else {
            continue;
        };
        schema.additional_properties = Some(Box::new(AdditionalProperties::FreeForm(false)));
        let mut neither = schema.clone();
        neither.properties.remove("expires_at");
        neither.properties.remove("expires_in");
        let mut absolute = schema.clone();
        absolute.properties.remove("expires_in");
        absolute.required.push("expires_at".into());
        let mut relative = schema.clone();
        relative.properties.remove("expires_at");
        relative.required.push("expires_in".into());
        *components.schemas.get_mut(name).unwrap() = RefOr::T(Schema::OneOf(OneOf {
            items: vec![
                RefOr::T(Schema::Object(neither)),
                RefOr::T(Schema::Object(absolute)),
                RefOr::T(Schema::Object(relative)),
            ],
            ..OneOf::default()
        }));
    }
    for name in [
        "AttachmentUploadRequest",
        "BrowserSessionResponse",
        "BearerSessionResponse",
        "AnonymousSessionResponse",
    ] {
        if let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) {
            schema.additional_properties = Some(Box::new(AdditionalProperties::FreeForm(false)));
        }
    }
    for (name, authenticated) in [
        ("BrowserSessionResponse", true),
        ("BearerSessionResponse", true),
        ("AnonymousSessionResponse", false),
    ] {
        let Some(RefOr::T(Schema::Object(schema))) = components.schemas.get_mut(name) else {
            continue;
        };
        let Some(RefOr::T(Schema::Object(authenticated_schema))) =
            schema.properties.get_mut("authenticated")
        else {
            continue;
        };
        authenticated_schema.enum_values = Some(vec![serde_json::json!(authenticated)]);
    }

    fn remove_null(value: &mut RefOr<Schema>) {
        let RefOr::T(schema) = value else {
            return;
        };
        match schema {
            Schema::Object(object) => {
                if let SchemaType::Array(types) = &mut object.schema_type {
                    types.retain(|value| *value != Type::Null);
                    if types.len() == 1 {
                        object.schema_type = SchemaType::Type(types[0].clone());
                    }
                }
            }
            Schema::OneOf(one_of) => {
                one_of.items.retain(|item| {
                    !matches!(item, RefOr::T(Schema::Object(object)) if object.schema_type == SchemaType::Type(Type::Null))
                });
                if one_of.items.len() == 1 {
                    *value = one_of.items[0].clone();
                }
            }
            _ => {}
        }
    }
}

fn refine_parameter_schemas(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::schema::Schema;
    use utoipa::openapi::RefOr;

    let enum_schemas = openapi
        .components
        .as_ref()
        .map(|components| {
            ["PasteSort", "SortDirection"]
                .into_iter()
                .filter_map(|name| {
                    components
                        .schemas
                        .get(name)
                        .and_then(|schema| match schema {
                            RefOr::T(schema) => Some((name, schema.clone())),
                            RefOr::Ref(_) => None,
                        })
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();
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
            for parameter in operation.parameters.iter_mut().flatten() {
                match parameter.name.as_str() {
                    "page_size" => {
                        if let Some(RefOr::T(Schema::Object(schema))) = parameter.schema.as_mut() {
                            schema.maximum = Some(crate::limits::MAX_PAGE_SIZE.into());
                            schema.default = Some(crate::limits::DEFAULT_PAGE_SIZE.into());
                        }
                    }
                    "title" => {
                        if let Some(RefOr::T(Schema::Object(schema))) = parameter.schema.as_mut() {
                            schema.max_length = Some(crate::limits::MAX_TITLE_CHARACTERS);
                        }
                    }
                    "sort" | "direction" => {
                        let (component, default) = if parameter.name == "sort" {
                            ("PasteSort", "created")
                        } else {
                            ("SortDirection", "desc")
                        };
                        if let Some(schema) = enum_schemas.get(component) {
                            let mut schema = schema.clone();
                            if let Schema::Object(object) = &mut schema {
                                object.default = Some(default.into());
                            }
                            parameter.schema = Some(RefOr::T(schema));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(components) = openapi.components.as_mut() {
        components.schemas.remove("PasteSort");
        components.schemas.remove("SortDirection");
    }
}

fn constrain_numeric_path_ids(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::path::ParameterIn;
    use utoipa::openapi::schema::Schema;
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
            for parameter in operation.parameters.iter_mut().flatten() {
                if parameter.parameter_in != ParameterIn::Path
                    || !matches!(
                        parameter.name.as_str(),
                        "id" | "attachment_id" | "folder_id"
                    )
                {
                    continue;
                }
                if let Some(RefOr::T(Schema::Object(schema))) = parameter.schema.as_mut() {
                    schema.minimum = Some(1.into());
                }
            }
        }
    }
}

fn normalize_optional_header_schemas(openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::path::ParameterIn;
    use utoipa::openapi::schema::{Schema, SchemaType, Type};
    use utoipa::openapi::{RefOr, Required};

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
            for parameter in operation.parameters.iter_mut().flatten() {
                if parameter.parameter_in != ParameterIn::Header
                    || parameter.required != Required::False
                {
                    continue;
                }
                let Some(RefOr::T(Schema::Object(schema))) = parameter.schema.as_mut() else {
                    continue;
                };
                let SchemaType::Array(types) = &mut schema.schema_type else {
                    continue;
                };
                types.retain(|value| *value != Type::Null);
                if types.len() == 1 {
                    schema.schema_type = SchemaType::Type(types[0].clone());
                }
            }
        }
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
                let mut extensions =
                    Extensions::from_iter([("x-racebin-scopes", serde_json::json!(scopes))]);
                if operation.operation_id.as_deref() == Some("get_paste_source") {
                    extensions.insert(
                        "x-racebin-authorization".into(),
                        serde_json::json!({
                            "anyOf": [
                                {"scope": "paste:read", "relationship": "owner"},
                                {"scope": "paste:manage"}
                            ]
                        }),
                    );
                }
                operation.extensions = Some(extensions);
            }
        }
    }
}

fn operation_scopes(operation_id: &str) -> &'static [&'static str] {
    match operation_id {
        "list_pastes" | "list_folders" => &["paste:list"],
        "get_paste" | "get_paste_raw" | "get_paste_source" | "read_paste" | "get_attachment"
        | "get_archive" | "get_qr" => &["paste:read"],
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
        | "admin_summary"
        | "admin_user"
        | "admin_update_user"
        | "admin_create_password_reset"
        | "admin_revoke_user_sessions" => &["user:manage"],
        "admin_invitations"
        | "admin_create_invitation"
        | "admin_update_invitation"
        | "admin_revoke_invitation" => &["invitation:manage"],
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
    #[schema(format = "uri-reference")]
    web_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(format = "uri-reference")]
    api_base_url: Option<String>,
    plain_home_enabled: bool,
    public_explore_enabled: bool,
    invitations_enabled: bool,
    default_format: String,
    default_language: String,
    default_visibility: String,
    default_expiration_seconds: Option<i64>,
    max_attachment_size_bytes: usize,
    max_attachments_per_paste: usize,
    max_folders_per_user: usize,
    attachments_enabled: bool,
    qr_codes_enabled: bool,
    formats: [&'static str; 2],
    markdown_dialect: &'static str,
    markdown_extensions: [&'static str; 4],
    visibility_modes: [&'static str; 3],
    authentication_methods: [&'static str; 2],
    paste_create_media_types: [&'static str; 6],
    attachment_upload_media_types: [&'static str; 1],
    scopes: Vec<ScopeDescription>,
    max_title_characters: usize,
    max_content_size_bytes: usize,
    #[schema(minimum = 1)]
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
    HttpResponse::Ok().json(openapi_document())
}

#[utoipa::path(get, path = "/capabilities", tag = "discovery", responses((status = 200, description = "Runtime features, limits, and authorization scopes", body = Capabilities)), security(()))]
#[get("/capabilities")]
async fn get_capabilities(services: web::Data<PasteService>) -> impl Responder {
    let settings = match crate::instance::settings::get(&services.storage).await {
        Ok(settings) => settings,
        Err(error) => return domain_error(error),
    };
    let (web_base_url, api_base_url) = canonical_base_urls(ARGS.public_url.as_ref());
    HttpResponse::Ok().json(Capabilities {
        site_name: settings.site_name,
        server_version: env!("CARGO_PKG_VERSION"),
        api_version: "v1",
        web_base_url,
        api_base_url,
        plain_home_enabled: settings.home_mode == "plain",
        public_explore_enabled: settings.public_explore_enabled,
        invitations_enabled: settings.invitations_enabled,
        default_format: settings.default_format,
        default_language: settings.default_language,
        default_visibility: settings.default_visibility,
        default_expiration_seconds: settings.default_expiration_seconds,
        max_attachment_size_bytes: ARGS
            .attachment_size_limit_bytes()
            .expect("attachment limit is validated at startup"),
        max_attachments_per_paste: crate::limits::MAX_ATTACHMENTS_PER_PASTE,
        max_folders_per_user: crate::limits::MAX_FOLDERS_PER_USER,
        attachments_enabled: settings.attachments_enabled,
        qr_codes_enabled: settings.qr_codes_enabled,
        formats: ["text", "markdown"],
        markdown_dialect: "CommonMark with GitHub-Flavored Markdown extensions",
        markdown_extensions: ["tables", "task_lists", "autolinks", "strikethrough"],
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
                description: "Create, list, annotate, and revoke invitations",
            },
            ScopeDescription {
                id: "api_key:manage",
                description: "Manage API keys, subject to ownership and delegation rules",
            },
        ],
        max_title_characters: crate::limits::MAX_TITLE_CHARACTERS,
        max_content_size_bytes: crate::limits::MAX_CONTENT_SIZE_BYTES,
        max_page_size: crate::limits::MAX_PAGE_SIZE,
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
mod tests;
