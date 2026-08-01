use crate::services::{DomainError, ErrorKind};
use actix_web::http::{header, StatusCode};
use actix_web::HttpResponse;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<serde_json::Value>,
}

pub(super) fn error(
    status: StatusCode,
    code: &'static str,
    message: impl std::fmt::Display,
) -> HttpResponse {
    let title = status
        .canonical_reason()
        .unwrap_or("Request failed")
        .to_string();
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("application/problem+json")
        .json(ProblemDetails {
            problem_type: format!("urn:racebin:problem:{code}"),
            title,
            status: status.as_u16(),
            detail: message.to_string(),
            errors: None,
        })
}

pub(super) fn internal(message: impl std::fmt::Display) -> HttpResponse {
    log::error!("{message}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Internal server error",
    )
}

pub(super) fn domain_error(value: DomainError) -> HttpResponse {
    let status = match value.kind {
        ErrorKind::NotFound => StatusCode::NOT_FOUND,
        ErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ErrorKind::Validation => StatusCode::BAD_REQUEST,
        ErrorKind::Conflict => StatusCode::CONFLICT,
        ErrorKind::Precondition => StatusCode::PRECONDITION_FAILED,
        ErrorKind::Internal => return internal(value.message),
    };
    error(status, value.code, value.message)
}
