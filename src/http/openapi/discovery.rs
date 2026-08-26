use super::*;

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

pub(crate) fn configure(config: &mut web::ServiceConfig) {
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
pub(super) async fn api_root() -> impl Responder {
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
pub(super) async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(openapi_document())
}

#[utoipa::path(get, path = "/capabilities", tag = "discovery", responses((status = 200, description = "Runtime features, limits, and authorization scopes", body = Capabilities)), security(()))]
#[get("/capabilities")]
pub(super) async fn get_capabilities(services: web::Data<PasteService>) -> impl Responder {
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

pub(super) fn canonical_base_urls(
    public_url: Option<&url::Url>,
) -> (Option<String>, Option<String>) {
    let Some(public_url) = public_url else {
        return (None, None);
    };
    let web_base_url = public_url.as_str().trim_end_matches('/').to_string();
    let api_base_url = format!("{web_base_url}/api/v1");
    (Some(web_base_url), Some(api_base_url))
}

#[utoipa::path(get, path = "/languages", tag = "discovery", responses((status = 200, description = "Accepted syntax identifiers and aliases", body = [Language])), security(()))]
#[get("/languages")]
pub(super) async fn get_languages() -> impl Responder {
    HttpResponse::Ok().json(LANGUAGES)
}

#[utoipa::path(get, path = "/health", tag = "discovery", responses((status = 204, description = "Process is running")), security(()))]
#[get("/health")]
pub(super) async fn get_health() -> HttpResponse {
    health().await
}

#[utoipa::path(get, path = "/readiness", tag = "discovery", responses((status = 204, description = "Database is available"), (status = 503, description = "Database is unavailable")), security(()))]
#[get("/readiness")]
pub(super) async fn get_readiness(services: web::Data<PasteService>) -> HttpResponse {
    ready(services).await
}

pub(crate) async fn health() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

pub(crate) async fn ready(services: web::Data<PasteService>) -> HttpResponse {
    match sqlx::query("SELECT 1")
        .execute(services.storage.pool())
        .await
    {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(_) => HttpResponse::ServiceUnavailable().finish(),
    }
}
