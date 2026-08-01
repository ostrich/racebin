use super::errors::{error, internal};
use crate::account::{self as accounts, api_keys};
use crate::args::ARGS;
use crate::services::{PasteService, Principal};

pub(super) fn client_address(req: &HttpRequest) -> String {
    let Some(peer) = req.peer_addr() else {
        return "unknown".to_string();
    };
    if ARGS.trusted_proxies.contains(&peer.ip()) {
        if let Some(forwarded) = req
            .headers()
            .get("X-Forwarded-For")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(',').next())
            .map(str::trim)
            .filter(|value| value.parse::<std::net::IpAddr>().is_ok())
        {
            return forwarded.to_string();
        }
    }
    peer.ip().to_string()
}

use actix_web::http::{header, StatusCode};
use actix_web::{HttpRequest, HttpResponse};

pub(super) async fn principal(
    services: &PasteService,
    request: &HttpRequest,
) -> Result<Principal, HttpResponse> {
    resolve_principal(services, request)
        .await
        .map_err(|message| {
            if message == "Password change required" {
                error(StatusCode::FORBIDDEN, "password_change_required", message)
            } else if message.contains("authorization") || message.contains("bearer") {
                unauthorized("invalid_token", message)
            } else {
                internal(message)
            }
        })
}

async fn resolve_principal(
    services: &PasteService,
    request: &HttpRequest,
) -> Result<Principal, String> {
    if let Some(header) = request.headers().get("Authorization") {
        let header = header
            .to_str()
            .map_err(|_| "Invalid authorization header")?;
        let value = header
            .strip_prefix("Bearer ")
            .ok_or("Invalid authorization scheme")?;
        return api_keys::authenticate(&services.storage, value)
            .await?
            .map(Principal::ApiKey)
            .ok_or_else(|| "Invalid bearer token".to_string());
    }
    let session = match request.cookie(accounts::SESSION_COOKIE) {
        Some(cookie) => accounts::session_user(&services.storage, cookie.value())
            .await?
            .map(Principal::Session)
            .unwrap_or(Principal::Anonymous),
        None => Principal::Anonymous,
    };
    if matches!(
        &session,
        Principal::Session(session)
            if session.user.password_change_required
                && !matches!(
                    (request.method().as_str(), request.path()),
                    ("GET", "/api/v1/session")
                        | ("DELETE", "/api/v1/session")
                        | ("PATCH", "/api/v1/account/password")
                )
    ) {
        Err("Password change required".to_string())
    } else {
        Ok(session)
    }
}

pub(super) fn require_auth(principal: Principal) -> Result<Principal, HttpResponse> {
    if matches!(principal, Principal::Anonymous) {
        Err(unauthorized(
            "authentication_required",
            "Authentication required",
        ))
    } else {
        Ok(principal)
    }
}

fn unauthorized(code: &'static str, detail: impl Into<String>) -> HttpResponse {
    let mut response = error(StatusCode::UNAUTHORIZED, code, detail);
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Bearer realm=\"Racebin API\""),
    );
    response
}

pub(super) fn require_mutation(
    _services: &PasteService,
    request: &HttpRequest,
    principal: Principal,
) -> Result<Principal, HttpResponse> {
    let principal = require_auth(principal)?;
    let csrf_valid = match &principal {
        Principal::ApiKey(_) => true,
        Principal::Session(session) => request
            .headers()
            .get("X-CSRF-Token")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == session.csrf_token),
        Principal::Anonymous => false,
    };
    if !csrf_valid {
        return Err(error(
            StatusCode::FORBIDDEN,
            "csrf_failed",
            "Missing or invalid CSRF token",
        ));
    }
    Ok(principal)
}

pub(super) fn require_admin(principal: &Principal, scope: &str) -> Result<(), HttpResponse> {
    if principal.can(scope) {
        Ok(())
    } else {
        Err(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            format!("Missing {scope} permission"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::client_address;
    use actix_web::test;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[actix_web::test]
    async fn forwarding_headers_from_untrusted_peers_are_ignored() {
        let request = test::TestRequest::default()
            .peer_addr(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234))
            .insert_header(("X-Forwarded-For", "203.0.113.10"))
            .to_http_request();
        assert_eq!(client_address(&request), "127.0.0.1");
    }
}
