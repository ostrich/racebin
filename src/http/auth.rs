use super::errors::{error, internal};
use crate::services::{Principal, Services};
use actix_web::http::StatusCode;
use actix_web::{HttpRequest, HttpResponse};

pub(super) async fn principal(
    services: &Services,
    request: &HttpRequest,
) -> Result<Principal, HttpResponse> {
    services.principal(request).await.map_err(|message| {
        if message == "Password change required" {
            error(StatusCode::FORBIDDEN, "password_change_required", message)
        } else if message.contains("authorization") || message.contains("bearer") {
            error(StatusCode::UNAUTHORIZED, "invalid_token", message)
        } else {
            internal(message)
        }
    })
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
    services: &Services,
    request: &HttpRequest,
    principal: Principal,
) -> Result<Principal, HttpResponse> {
    let principal = require_auth(principal)?;
    if !services.csrf_valid(request, &principal) {
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
