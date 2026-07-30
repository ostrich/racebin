use super::errors::{error, internal};
use crate::account::{self as accounts, api_keys};
use crate::services::{PasteService, Principal};
use actix_web::http::StatusCode;
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
                error(StatusCode::UNAUTHORIZED, "invalid_token", message)
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
        Err(error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication required",
        ))
    } else {
        Ok(principal)
    }
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
