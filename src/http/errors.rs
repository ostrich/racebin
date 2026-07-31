use actix_web::http::{header, StatusCode};
use actix_web::HttpResponse;

#[derive(serde::Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(serde::Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    details: Option<serde_json::Value>,
}

pub(super) fn error(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ErrorEnvelope {
            error: ErrorBody {
                code,
                message: message.into(),
                details: None,
            },
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
