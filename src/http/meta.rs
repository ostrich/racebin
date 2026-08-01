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
        get_capabilities,
        get_languages
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
    tags((name = "pastes"), (name = "discovery"))
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
    }
}

#[derive(Serialize, utoipa::ToSchema)]
struct Capabilities {
    site_name: String,
    plain_home_enabled: bool,
    max_attachment_size_bytes: usize,
    max_attachments_per_paste: usize,
    attachments_enabled: bool,
    qr_codes_enabled: bool,
    formats: [&'static str; 2],
    visibility_modes: [&'static str; 3],
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
        .service(get_languages);
}

#[get("")]
async fn api_root() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "name": "Racebin API",
        "version": env!("CARGO_PKG_VERSION"),
        "openapi_url": "/api/v1/openapi.json",
        "capabilities_url": "/api/v1/capabilities",
        "languages_url": "/api/v1/languages"
    }))
}

#[get("/openapi.json")]
async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(ApiDoc::openapi())
}

#[utoipa::path(get, path = "/capabilities", tag = "discovery", responses((status = 200, body = Capabilities)))]
#[get("/capabilities")]
async fn get_capabilities() -> impl Responder {
    HttpResponse::Ok().json(Capabilities {
        site_name: ARGS.site_name.clone().unwrap_or_else(|| "Racebin".into()),
        plain_home_enabled: ARGS.plain_home,
        max_attachment_size_bytes: ARGS.max_attachment_size_mb * 1024 * 1024,
        max_attachments_per_paste: 32,
        attachments_enabled: ARGS.attachments_enabled,
        qr_codes_enabled: ARGS.qr_codes,
        formats: ["text", "rich_text"],
        visibility_modes: ["public", "unlisted", "private"],
    })
}

#[utoipa::path(get, path = "/languages", tag = "discovery", responses((status = 200, body = [Language])))]
#[get("/languages")]
async fn get_languages() -> impl Responder {
    HttpResponse::Ok().json(LANGUAGES)
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
    fn generated_contract_contains_canonical_paste_lifecycle() {
        let document = ApiDoc::openapi();
        let paths = document.paths.paths;
        for path in [
            "/pastes",
            "/pastes/{paste_id}",
            "/pastes/{paste_id}/source",
            "/pastes/{paste_id}/reads",
            "/content-conversions",
            "/capabilities",
            "/languages",
        ] {
            assert!(paths.contains_key(path), "missing OpenAPI path {path}");
        }
        assert!(!paths.contains_key("/pastes/{paste_id}/consume"));
        assert!(!paths.contains_key("/pastes/{paste_id}/raw"));
    }
}
