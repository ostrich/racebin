use super::errors::error;
use actix_web::http::{header, Method, StatusCode};
use actix_web::{web, HttpRequest, HttpResponse};

const SPA_INDEX: &[u8] = include_bytes!("../../web/dist/index.html");
const SPA_SCRIPT: &[u8] = include_bytes!("../../web/dist/assets/app.js");
const SPA_STYLE: &[u8] = include_bytes!("../../web/dist/assets/app.css");
const RICH_TEXT_SCRIPT: &[u8] = include_bytes!("../../web/dist/assets/rich_text_editor.js");

pub(super) async fn asset(path: web::Path<String>) -> HttpResponse {
    let asset = match path.as_str() {
        "app.js" => Some((SPA_SCRIPT, "text/javascript; charset=utf-8")),
        "app.css" => Some((SPA_STYLE, "text/css; charset=utf-8")),
        "rich_text_editor.js" => Some((RICH_TEXT_SCRIPT, "text/javascript; charset=utf-8")),
        _ => None,
    };
    match asset {
        Some((bytes, content_type)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, content_type))
            .insert_header((header::CACHE_CONTROL, "public, max-age=31536000, immutable"))
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
    path == "/"
        || matches!(
            path,
            "/explore"
                | "/login"
                | "/new"
                | "/pastes"
                | "/account"
                | "/account/password"
                | "/admin"
                | "/admin/pastes"
                | "/guide"
        )
        || path
            .strip_prefix("/invitations/")
            .is_some_and(|value| !value.is_empty() && !value.contains('/'))
        || path.strip_prefix("/pastes/").is_some_and(|value| {
            let pieces: Vec<_> = value.split('/').collect();
            pieces.len() == 1 || (pieces.len() == 2 && pieces[1] == "edit")
        })
}

#[cfg(test)]
mod tests {
    use super::spa_route;

    #[test]
    fn only_known_spa_routes_are_accepted() {
        assert!(spa_route("/"));
        assert!(spa_route("/pastes/example"));
        assert!(spa_route("/pastes/example/edit"));
        assert!(spa_route("/invitations/token"));
        assert!(!spa_route("/api/v2/pastes"));
        assert!(!spa_route("/pastes/example/unknown"));
        assert!(!spa_route("/invitations/token/nested"));
    }
}
