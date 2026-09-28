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
            .insert_header((
                header::CACHE_CONTROL,
                if is_fingerprinted(path.as_str()) {
                    "public, max-age=31536000, immutable"
                } else {
                    "public, max-age=3600"
                },
            ))
            .body(bytes),
        None => HttpResponse::NotFound().finish(),
    }
}

fn is_fingerprinted(path: &str) -> bool {
    let Some((stem, _)) = path.rsplit_once('.') else {
        return false;
    };
    let Some(hash) = stem.rsplit('-').next() else {
        return false;
    };
    hash.len() >= 8
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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
    use super::{embedded_asset, is_fingerprinted, spa_route, EMBEDDED_ASSET_PATHS};
    use actix_web::http::header;
    use actix_web::{middleware, test as awtest, web, App};

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
        assert!(EMBEDDED_ASSET_PATHS.contains(&"theme-init.js"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"favicon-32x32.png"));
        assert!(EMBEDDED_ASSET_PATHS.contains(&"apple-touch-icon.png"));
        assert!(EMBEDDED_ASSET_PATHS
            .iter()
            .any(|path| path.starts_with("InterVariable-") && path.ends_with(".woff2")));
        assert!(EMBEDDED_ASSET_PATHS
            .iter()
            .any(|path| path.starts_with("InterVariable-Italic-") && path.ends_with(".woff2")));
        assert!(
            EMBEDDED_ASSET_PATHS
                .iter()
                .any(|path| path.starts_with("index-") && path.ends_with(".js")),
            "the frontend entry point should be content-hashed"
        );
        assert!(
            EMBEDDED_ASSET_PATHS
                .iter()
                .any(|path| path.ends_with(".css") && is_fingerprinted(path)),
            "the frontend stylesheet should be content-hashed"
        );
        assert!(
            EMBEDDED_ASSET_PATHS
                .iter()
                .any(|path| path.ends_with(".js") && !path.starts_with("index-")),
            "the frontend build should include at least one lazy-loaded JavaScript chunk"
        );
        for path in EMBEDDED_ASSET_PATHS {
            let (contents, content_type) =
                embedded_asset(path).unwrap_or_else(|| panic!("{path} is not embedded"));
            assert!(!contents.is_empty(), "{path} is empty");
            assert!(!content_type.is_empty(), "{path} has no content type");
        }
    }

    #[test]
    fn only_content_hashed_assets_are_immutable() {
        assert!(is_fingerprinted("app-CV7z6EZ9.js"));
        assert!(is_fingerprinted("InterVariable-D7YiKFrg.woff2"));
        assert!(!is_fingerprinted("theme-init.js"));
        assert!(!is_fingerprinted("favicon-32x32.png"));
    }

    #[actix_web::test]
    async fn asset_delivery_is_cacheable_and_compressed() {
        let hashed_javascript = EMBEDDED_ASSET_PATHS
            .iter()
            .copied()
            .find(|path| path.starts_with("index-") && path.ends_with(".js"))
            .expect("frontend entry point");
        let app = awtest::init_service(
            App::new()
                .wrap(middleware::Compress::default())
                .service(web::resource("/assets/{path:.*}").route(web::get().to(super::asset))),
        )
        .await;
        let response = awtest::call_service(
            &app,
            awtest::TestRequest::get()
                .uri(&format!("/assets/{hashed_javascript}"))
                .insert_header((header::ACCEPT_ENCODING, "gzip"))
                .to_request(),
        )
        .await;
        assert!(response.status().is_success());
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING).unwrap(),
            "gzip"
        );
        assert!(response.headers().contains_key(header::VARY));

        let response = awtest::call_service(
            &app,
            awtest::TestRequest::get()
                .uri("/assets/theme-init.js")
                .to_request(),
        )
        .await;
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "public, max-age=3600"
        );
    }
}
