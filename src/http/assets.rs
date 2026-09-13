use super::errors::error;
use actix_web::http::{header, Method, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::sync::OnceLock;

const SPA_INDEX: &[u8] = include_bytes!("../../web/dist/index.html");
include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));

pub(super) async fn asset(path: web::Path<String>) -> HttpResponse {
    match embedded_asset(path.as_str()) {
        Some((bytes, content_type)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, content_type))
            .insert_header((header::CACHE_CONTROL, "no-cache"))
            .body(bytes),
        None => HttpResponse::NotFound().finish(),
    }
}

pub(super) async fn spa(request: HttpRequest) -> HttpResponse {
    if request.method() != Method::GET
        || request.path().starts_with("/api/")
        || !spa_route(request.path())
    {
        return error(StatusCode::NOT_FOUND, "not_found", "Route not found");
    }
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .body(SPA_INDEX)
}

fn spa_route(path: &str) -> bool {
    #[derive(Deserialize)]
    struct RouteDefinition {
        path: String,
    }
    static ROUTES: OnceLock<Vec<RouteDefinition>> = OnceLock::new();
    ROUTES
        .get_or_init(|| {
            serde_json::from_str(include_str!("../../web/src/navigation/routes.json"))
                .expect("frontend route manifest must be valid")
        })
        .iter()
        .any(|route| template_matches(&route.path, path))
}

fn template_matches(template: &str, path: &str) -> bool {
    let expected = template.split('/').collect::<Vec<_>>();
    let actual = path.split('/').collect::<Vec<_>>();
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(expected, actual)| {
            if *expected == ":userId:int" {
                !actual.is_empty() && actual.bytes().all(|byte| byte.is_ascii_digit())
            } else if expected.starts_with(':') {
                !actual.is_empty()
            } else {
                *expected == actual
            }
        })
}

#[cfg(test)]
mod tests {
    use super::{embedded_asset, spa_route, EMBEDDED_ASSET_PATHS};

    #[test]
    fn only_known_spa_routes_are_accepted() {
        assert!(spa_route("/"));
        assert!(spa_route("/pastes/example"));
        assert!(spa_route("/pastes/example/edit"));
        assert!(spa_route("/invitations/token"));
        assert!(spa_route("/help"));
        assert!(spa_route("/admin/users/42"));
        assert!(spa_route("/admin/invitations"));
        assert!(spa_route("/admin/api-keys"));
        assert!(spa_route("/admin/settings"));
        assert!(spa_route("/admin/audit"));
        assert!(spa_route("/password-reset/token"));
        assert!(!spa_route("/api/v1/pastes"));
        assert!(!spa_route("/pastes/example/unknown"));
        assert!(!spa_route("/invitations/token/nested"));
        assert!(!spa_route("/admin/users/not-a-number"));
        assert!(!spa_route("/admin/settings/nested"));
    }

    #[test]
    fn every_built_frontend_asset_is_embedded() {
        assert!(EMBEDDED_ASSET_PATHS.contains(&"app.js"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"app.css"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"theme-init.js"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"favicon-32x32.png"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"apple-touch-icon.png"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"InterVariable.woff2"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"InterVariable-Italic.woff2"));
        assert!(
            EMBEDDED_ASSET_PATHS
                .iter()
                .any(|path| path.ends_with(".js") && *path != "app.js"),
            "the frontend build should include at least one lazy-loaded JavaScript chunk"
        );
        for path in EMBEDDED_ASSET_PATHS {
            let (contents, content_type) =
                embedded_asset(path).unwrap_or_else(|| panic!("{path} is not embedded"));
            assert!(!contents.is_empty(), "{path} is empty");
            assert!(!content_type.is_empty(), "{path} has no content type");
        }
    }
}
