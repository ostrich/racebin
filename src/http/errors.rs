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
    message: impl Into<String>,
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
            detail: message.into(),
            errors: None,
        })
}

pub(super) fn internal(message: impl Into<String>) -> HttpResponse {
    log::error!("{}", message.into());
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Internal server error",
    )
}

pub(super) fn paste_error(message: String) -> HttpResponse {
    if message.starts_with("Missing ")
        || message == "You do not own this paste"
        || message.starts_with("Folders require")
    {
        error(StatusCode::FORBIDDEN, "forbidden", message)
    } else if [
        "Content is required",
        "Title exceeds",
        "Content kind must",
        "Visibility must",
        "Read limit",
        "Expiration",
        "URL content",
        "Rich-text",
        "Only rich-text",
        "Unsupported rich-text",
        "Every rich-text",
        "Folder not found",
        "Folder ",
        "Folder name",
        "A folder",
        "Paste IDs",
        "Select between",
        "One or more pastes",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
    {
        error(StatusCode::BAD_REQUEST, "invalid_paste", message)
    } else {
        internal(message)
    }
}
