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
        .service(get_languages)
        .service(get_health)
        .service(get_readiness);
}

#[utoipa::path(get, path = "/", tag = "discovery")]
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

#[utoipa::path(get, path = "/openapi.json", tag = "discovery")]
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

#[utoipa::path(get, path = "/health", tag = "discovery", responses((status = 204)))]
#[get("/health")]
async fn get_health() -> HttpResponse {
    health().await
}

#[utoipa::path(get, path = "/readiness", tag = "discovery", responses((status = 204), (status = 503)))]
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
}
