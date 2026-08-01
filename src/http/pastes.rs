use super::*;
use crate::http::dto::{
    self, BodyInput, CreatePasteRequest, Pagination, PastePage, UpdatePasteRequest,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_pastes)
        .service(create_paste)
        .service(convert_paste_content)
        .service(read_paste)
        .service(get_paste)
        .service(get_paste_source)
        .service(update_paste)
        .service(delete_paste);
}

#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ApiPasteQuery {
    page: Option<u32>,
    page_size: Option<u32>,
    q: Option<String>,
    visibility: Option<String>,
    owner: Option<String>,
    folder_id: Option<i64>,
    unfiled: Option<bool>,
    format: Option<String>,
    language: Option<String>,
    has_attachments: Option<bool>,
    created_after: Option<String>,
    created_before: Option<String>,
    expiration: Option<String>,
    min_reads: Option<i64>,
    max_reads: Option<i64>,
    min_size_bytes: Option<i64>,
    max_size_bytes: Option<i64>,
    read_limit: Option<String>,
    sort: Option<String>,
    direction: Option<String>,
}

impl ApiPasteQuery {
    fn into_internal(self) -> Result<PasteQuery, String> {
        if self.owner.as_deref().is_some_and(|owner| owner != "me") {
            return Err("owner must be me when supplied".into());
        }
        Ok(PasteQuery {
            page: self.page,
            page_size: self.page_size,
            search: self.q,
            visibility: self.visibility,
            owner_id: None,
            folder_id: self.folder_id,
            unfiled: self.unfiled,
            mine: Some(self.owner.as_deref() == Some("me")),
            content_kind: self.format,
            language: self.language,
            has_attachments: self.has_attachments,
            created_after: self
                .created_after
                .map(|value| dto::parse_timestamp(&value))
                .transpose()?,
            created_before: self
                .created_before
                .map(|value| dto::parse_timestamp(&value))
                .transpose()?,
            expiration: self.expiration,
            min_reads: self.min_reads,
            max_reads: self.max_reads,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            read_limit: self.read_limit,
            sort: self.sort,
            direction: self.direction,
        })
    }
}

#[derive(Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FlatCreateRequest {
    title: Option<String>,
    format: Option<String>,
    content: Option<String>,
    language: Option<String>,
    visibility: Option<String>,
    folder_id: Option<i64>,
    expires_at: Option<String>,
    expires_in: Option<i64>,
    read_limit: Option<i64>,
}

impl FlatCreateRequest {
    fn structured(self) -> Result<CreatePasteRequest, String> {
        let format = self.format.as_deref().unwrap_or("text");
        let body = match format {
            "text" => Some(BodyInput::Text {
                content: self.content.unwrap_or_default(),
                language: self.language,
            }),
            "rich_text" => {
                if self.language.is_some() {
                    return Err("Rich text does not accept a language".into());
                }
                Some(BodyInput::RichText {
                    content: self.content.unwrap_or_default(),
                })
            }
            _ => return Err("format must be text or rich_text".into()),
        };
        Ok(CreatePasteRequest {
            title: self.title,
            body,
            visibility: self.visibility,
            folder_id: self.folder_id,
            expires_at: self.expires_at,
            expires_in: self.expires_in,
            read_limit: self.read_limit,
        })
    }
}

struct StagedFile {
    temporary: PathBuf,
    filename: String,
    storage_key: String,
    size_bytes: i64,
    digest: String,
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[utoipa::path(get, path = "/pastes", tag = "pastes", responses((status = 200, body = PastePage), (status = 400, body = crate::http::errors::ProblemDetails)))]
#[get("/pastes")]
pub(crate) async fn list_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<ApiPasteQuery>,
) -> HttpResponse {
    let principal = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let query = match query.into_inner().into_internal() {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    if query.mine.unwrap_or(false) && principal.user_id().is_none() {
        return error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication required for owner=me",
        );
    }
    if let Err(message) = validate_paste_query(&query) {
        return error(StatusCode::BAD_REQUEST, "invalid_query", message);
    }
    let wants_private =
        query.mine.unwrap_or(false) || query.visibility.as_deref() != Some("public");
    if matches!(principal, Principal::ApiKey(_)) && wants_private && !principal.can("paste:list") {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing paste:list permission",
        );
    }
    match services.list_pastes(&principal, &query, false).await {
        Ok(page) => {
            let total_pages = if page.total_items == 0 {
                0
            } else {
                (page.total_items as u64).div_ceil(u64::from(page.page_size)) as u32
            };
            HttpResponse::Ok().json(PastePage {
                items: page
                    .items
                    .into_iter()
                    .map(|paste| dto::summary(&req, &principal, paste, false))
                    .collect(),
                pagination: Pagination {
                    page: page.page,
                    page_size: page.page_size,
                    total_items: page.total_items,
                    total_pages,
                },
            })
        }
        Err(message) => paste_error(message),
    }
}

#[utoipa::path(post, path = "/pastes", tag = "pastes", request_body = CreatePasteRequest, responses((status = 201, body = crate::http::dto::PasteResource), (status = 422, body = crate::http::errors::ProblemDetails)), security(("bearerAuth" = []), ("sessionCookie" = [])))]
#[post("/pastes")]
pub(crate) async fn create_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<FlatCreateRequest>,
    payload: web::Payload,
) -> HttpResponse {
    let principal = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let idempotency_key = match idempotency_key(&req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/json")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let parsed = if content_type == "multipart/form-data" {
        parse_multipart(&req, payload, &services).await
    } else {
        parse_non_multipart(&content_type, query.into_inner(), payload)
            .await
            .map(|request| (request, Vec::new()))
    };
    let (request, mut staged) = match parsed {
        Ok(value) => value,
        Err(response) => return response,
    };
    let has_content = match request.body.as_ref() {
        Some(BodyInput::Text { content, .. } | BodyInput::RichText { content }) => {
            !content.is_empty()
        }
        None => false,
    };
    if !has_content && staged.is_empty() {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_paste",
            "A paste requires content or at least one attachment",
        );
    }
    let fingerprint = request_fingerprint(&request, &staged);
    let input = match request.into_input(crate::time::unix_timestamp()) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message),
    };
    let (mut paste, replayed) = match services
        .create_paste_idempotent(&principal, &input, idempotency_key.as_deref(), &fingerprint)
        .await
    {
        Ok(value) => value,
        Err(message) if message.starts_with("Idempotency key") => {
            return error(StatusCode::CONFLICT, "idempotency_conflict", message)
        }
        Err(message) if message.starts_with("Idempotency resource") => {
            return error(StatusCode::CONFLICT, "idempotency_resource_gone", message)
        }
        Err(message) => return paste_error(message),
    };
    if !replayed && !staged.is_empty() {
        if let Err(message) =
            promote_created_files(&services, &principal, &mut paste, &mut staged).await
        {
            if let Some(key) = idempotency_key.as_deref() {
                let _ = services.clear_create_idempotency(&principal, key).await;
            }
            let _ = services.delete_paste(&principal, &paste.id, None).await;
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_attachment",
                message,
            );
        }
    }
    let tag = dto::etag(&paste);
    let resource = dto::resource(&req, &principal, paste, true, None);
    let mut response = if accepts(&req, "text/plain") {
        HttpResponse::Created()
            .content_type("text/plain; charset=utf-8")
            .body(resource.url.clone())
    } else {
        HttpResponse::Created().json(resource.clone())
    };
    response.headers_mut().insert(
        header::LOCATION,
        header::HeaderValue::from_str(&resource.url).unwrap(),
    );
    response
        .headers_mut()
        .insert(header::ETAG, header::HeaderValue::from_str(&tag).unwrap());
    if replayed {
        response.headers_mut().insert(
            header::HeaderName::from_static("idempotency-replayed"),
            header::HeaderValue::from_static("true"),
        );
    }
    response
}

#[utoipa::path(get, path = "/pastes/{paste_id}", tag = "pastes", params(("paste_id" = String, Path)), responses((status = 200, body = crate::http::dto::PasteResource), (status = 404, body = crate::http::errors::ProblemDetails)))]
#[get("/pastes/{paste_id}")]
pub(crate) async fn get_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let principal = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_paste(&principal, &paste_id).await {
        Ok(Some(paste)) => resource_response(&req, &principal, paste, false, None),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(message) => internal(message),
    }
}

#[utoipa::path(get, path = "/pastes/{paste_id}/source", tag = "pastes", params(("paste_id" = String, Path)), responses((status = 200, body = crate::http::dto::PasteResource), (status = 403, body = crate::http::errors::ProblemDetails)), security(("bearerAuth" = []), ("sessionCookie" = [])))]
#[get("/pastes/{paste_id}/source")]
pub(crate) async fn get_paste_source(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let principal = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_source(&principal, &paste_id).await {
        Ok(Some(paste)) => content_response(&req, &principal, paste, None),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(message)
            if message == "You do not own this paste" || message.starts_with("Missing ") =>
        {
            error(StatusCode::FORBIDDEN, "forbidden", message)
        }
        Err(message) => internal(message),
    }
}

#[utoipa::path(post, path = "/pastes/{paste_id}/reads", tag = "pastes", params(("paste_id" = String, Path)), responses((status = 200, body = crate::http::dto::PasteResource), (status = 404, body = crate::http::errors::ProblemDetails)))]
#[post("/pastes/{paste_id}/reads")]
pub(crate) async fn read_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let principal = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let key = match idempotency_key(&req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services
        .read_paste(&principal, &paste_id, key.as_deref())
        .await
    {
        Ok(Some(read)) => {
            let mut response =
                content_response(&req, &principal, read.paste, read.grant_token.as_deref());
            if read.replayed {
                response.headers_mut().insert(
                    header::HeaderName::from_static("idempotency-replayed"),
                    header::HeaderValue::from_static("true"),
                );
            }
            response
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(message) => internal(message),
    }
}

#[utoipa::path(patch, path = "/pastes/{paste_id}", tag = "pastes", params(("paste_id" = String, Path)), request_body = UpdatePasteRequest, responses((status = 200, body = crate::http::dto::PasteResource), (status = 412, body = crate::http::errors::ProblemDetails)), security(("bearerAuth" = []), ("sessionCookie" = [])))]
#[patch("/pastes/{paste_id}")]
pub(crate) async fn update_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
    body: web::Json<UpdatePasteRequest>,
) -> HttpResponse {
    let principal = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let current = match services.ensure_can_update(&principal, &paste_id).await {
        Ok(value) => value,
        Err(message) if message == "Paste not found" => {
            return error(StatusCode::NOT_FOUND, "not_found", message)
        }
        Err(message) => return error(StatusCode::FORBIDDEN, "forbidden", message),
    };
    let expected_revision = match require_match(&req, &current) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let input = match body.into_inner().into_input() {
        Ok(value) => value,
        Err(message) => return error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message),
    };
    match services
        .update_paste(&principal, &paste_id, &input, expected_revision)
        .await
    {
        Ok(Some(paste)) => resource_response(
            &req,
            &principal,
            paste,
            principal.can("paste:read") || matches!(principal, Principal::Session(_)),
            None,
        ),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(message) if message == "Paste revision changed" => error(
            StatusCode::PRECONDITION_FAILED,
            "precondition_failed",
            "Paste changed since it was loaded",
        ),
        Err(message) => paste_error(message),
    }
}

#[utoipa::path(delete, path = "/pastes/{paste_id}", tag = "pastes", params(("paste_id" = String, Path)), responses((status = 204), (status = 412, body = crate::http::errors::ProblemDetails)), security(("bearerAuth" = []), ("sessionCookie" = [])))]
#[delete("/pastes/{paste_id}")]
pub(crate) async fn delete_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let principal = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let current = match services.ensure_can_delete(&principal, &paste_id).await {
        Ok(value) => value,
        Err(message) if message == "Paste not found" => {
            return error(StatusCode::NOT_FOUND, "not_found", message)
        }
        Err(message) => return error(StatusCode::FORBIDDEN, "forbidden", message),
    };
    let expected_revision = match require_match(&req, &current) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services
        .delete_paste(&principal, &paste_id, expected_revision)
        .await
    {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(message)
            if message == "You do not own this paste" || message.starts_with("Missing ") =>
        {
            error(StatusCode::FORBIDDEN, "forbidden", message)
        }
        Err(message) if message == "Paste revision changed" => error(
            StatusCode::PRECONDITION_FAILED,
            "precondition_failed",
            "Paste changed since it was loaded",
        ),
        Err(message) => internal(message),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ConversionInput {
    source: BodyInput,
    target_format: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ConversionOutput {
    body: BodyInput,
}

#[utoipa::path(post, path = "/content-conversions", tag = "pastes", responses((status = 200), (status = 422, body = crate::http::errors::ProblemDetails)), security(("bearerAuth" = []), ("sessionCookie" = [])))]
#[post("/content-conversions")]
pub(crate) async fn convert_paste_content(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<ConversionInput>,
) -> HttpResponse {
    if let Err(response) = principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        return response;
    }
    let result = match (&body.source, body.target_format.as_str()) {
        (BodyInput::Text { content, .. }, "rich_text") => Ok(BodyInput::RichText {
            content: crate::services::document_to_html(&text_to_document(content)),
        }),
        (BodyInput::RichText { content }, "text") => crate::services::html_to_document(content)
            .map(|document| BodyInput::Text {
                content: crate::services::document_to_text(&document),
                language: Some("plaintext".into()),
            }),
        (source, target)
            if matches!(
                (source, target),
                (BodyInput::Text { .. }, "text") | (BodyInput::RichText { .. }, "rich_text")
            ) =>
        {
            Ok(source.clone())
        }
        _ => Err("Conversion supports only text and rich_text".into()),
    };
    match result {
        Ok(body) => HttpResponse::Ok().json(ConversionOutput { body }),
        Err(message) => error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_conversion",
            message,
        ),
    }
}

pub(super) fn validate_paste_query(query: &PasteQuery) -> Result<(), &'static str> {
    if query.folder_id.is_some() && query.unfiled.unwrap_or(false) {
        return Err("folder_id and unfiled cannot be combined");
    }
    if (query.folder_id.is_some() || query.unfiled.unwrap_or(false)) && !query.mine.unwrap_or(false)
    {
        return Err("Folder filters require owner=me");
    }
    if query.folder_id.is_some_and(|value| value < 1) {
        return Err("Folder ID must be positive");
    }
    if query
        .visibility
        .as_deref()
        .is_some_and(|value| !matches!(value, "public" | "unlisted" | "private"))
    {
        return Err("Visibility must be public, unlisted, or private");
    }
    if query
        .content_kind
        .as_deref()
        .is_some_and(|value| !matches!(value, "text" | "rich_text"))
    {
        return Err("Format must be text or rich_text");
    }
    if query
        .expiration
        .as_deref()
        .is_some_and(|value| !matches!(value, "never" | "scheduled"))
    {
        return Err("Expiration must be never or scheduled");
    }
    if query
        .read_limit
        .as_deref()
        .is_some_and(|value| !matches!(value, "unlimited" | "limited"))
    {
        return Err("Read limit must be unlimited or limited");
    }
    if query
        .sort
        .as_deref()
        .is_some_and(|value| !matches!(value, "created" | "title" | "reads" | "expires" | "size"))
    {
        return Err("Unknown sort field");
    }
    if query
        .direction
        .as_deref()
        .is_some_and(|value| !matches!(value, "asc" | "desc"))
    {
        return Err("Direction must be asc or desc");
    }
    if matches!((query.created_after, query.created_before), (Some(after), Some(before)) if after > before)
    {
        return Err("Created-after date must precede created-before date");
    }
    if matches!((query.min_reads, query.max_reads), (Some(minimum), Some(maximum)) if minimum > maximum)
    {
        return Err("Minimum reads cannot exceed maximum reads");
    }
    if query.min_reads.is_some_and(|value| value < 0)
        || query.max_reads.is_some_and(|value| value < 0)
        || query.min_size_bytes.is_some_and(|value| value < 0)
        || query.max_size_bytes.is_some_and(|value| value < 0)
    {
        return Err("Read counts and sizes cannot be negative");
    }
    if matches!((query.min_size_bytes, query.max_size_bytes), (Some(minimum), Some(maximum)) if minimum > maximum)
    {
        return Err("Minimum size cannot exceed maximum size");
    }
    Ok(())
}

async fn parse_non_multipart(
    content_type: &str,
    query: FlatCreateRequest,
    mut payload: web::Payload,
) -> Result<CreatePasteRequest, HttpResponse> {
    let bytes = read_body(&mut payload, 2 * 1024 * 1024).await?;
    match content_type {
        "application/json" => {
            if query != FlatCreateRequest::default() {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "invalid_query",
                    "JSON requests do not accept creation fields in the query string",
                ));
            }
            serde_json::from_slice(&bytes).map_err(|_| {
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_json",
                    "Request body is not valid JSON",
                )
            })
        }
        "application/x-www-form-urlencoded" => serde_urlencoded::from_bytes::<FlatCreateRequest>(&bytes)
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_form", "Request body is not valid form data"))?
            .structured()
            .map_err(|message| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message)),
        "text/plain" | "text/markdown" | "text/html" => {
            let content = String::from_utf8(bytes.to_vec())
                .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_text", "Text input must be UTF-8"))?;
            let mut query = query;
            query.content = Some(content);
            query.format = Some(if content_type == "text/html" {
                "rich_text"
            } else {
                "text"
            }.into());
            if content_type == "text/markdown" {
                query.language = Some("markdown".into());
            }
            query.structured().map_err(|message| {
                error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message)
            })
        }
        _ => Err(error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Supported content types are JSON, plain text, Markdown, HTML, form data, and multipart form data",
        )),
    }
}

async fn parse_multipart(
    req: &HttpRequest,
    payload: web::Payload,
    services: &PasteService,
) -> Result<(CreatePasteRequest, Vec<StagedFile>), HttpResponse> {
    if !ARGS.attachments_enabled {
        return Err(error(
            StatusCode::FORBIDDEN,
            "uploads_disabled",
            "Attachments are disabled",
        ));
    }
    let mut multipart = Multipart::new(req.headers(), payload);
    let mut values = HashMap::<String, String>::new();
    let mut files = Vec::new();
    let mut total_size = 0usize;
    let limit = ARGS
        .max_attachment_size_mb
        .checked_mul(1024 * 1024)
        .ok_or_else(|| internal("Configured upload size is too large"))?;
    let staging = services
        .storage
        .data_dir
        .join("attachments")
        .join(".staging");
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|error| internal(error.to_string()))?;
    while let Some(mut field) = multipart.try_next().await.map_err(|reason| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_multipart",
            reason.to_string(),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        let filename = field
            .content_disposition()
            .and_then(|value| value.get_filename());
        if let Some(filename) = filename {
            if name != "file" {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_attachment",
                    "File parts must use the field name file",
                ));
            }
            if files.len() >= 32 {
                return Err(error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "too_many_attachments",
                    "A paste may contain at most 32 attachments",
                ));
            }
            let filename = super::attachments::sanitize_upload_filename(filename);
            if filename.is_empty() {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_attachment",
                    "Attachment filename is invalid",
                ));
            }
            let temporary = staging.join(format!("upload-{}", uuid::Uuid::new_v4().simple()));
            let mut output = tokio::fs::File::create(&temporary)
                .await
                .map_err(|error| internal(error.to_string()))?;
            let mut size = 0usize;
            let mut digest = Sha256::new();
            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_err(|reason| {
                    error(
                        StatusCode::BAD_REQUEST,
                        "invalid_multipart",
                        reason.to_string(),
                    )
                })?;
                size = size.saturating_add(chunk.len());
                total_size = total_size.saturating_add(chunk.len());
                if size > limit || total_size > limit {
                    let _ = tokio::fs::remove_file(&temporary).await;
                    return Err(error(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "attachment_too_large",
                        "Upload exceeds configured size limit",
                    ));
                }
                digest.update(&chunk);
                output
                    .write_all(&chunk)
                    .await
                    .map_err(|error| internal(error.to_string()))?;
            }
            files.push(StagedFile {
                temporary,
                filename,
                storage_key: uuid::Uuid::new_v4().simple().to_string(),
                size_bytes: size as i64,
                digest: format!("{:x}", digest.finalize()),
            });
        } else {
            if !matches!(
                name.as_str(),
                "title"
                    | "format"
                    | "content"
                    | "language"
                    | "visibility"
                    | "folder_id"
                    | "expires_at"
                    | "expires_in"
                    | "read_limit"
            ) {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "unknown_field",
                    format!("Unknown multipart field: {name}"),
                ));
            }
            if values.contains_key(&name) {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "duplicate_field",
                    format!("Multipart field appears more than once: {name}"),
                ));
            }
            let bytes = read_field(&mut field, 2 * 1024 * 1024).await?;
            let value = String::from_utf8(bytes).map_err(|_| {
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_text",
                    "Multipart text fields must be UTF-8",
                )
            })?;
            values.insert(name, value);
        }
    }
    let mut take_numeric = |name: &str| -> Result<Option<i64>, HttpResponse> {
        values
            .remove(name)
            .map(|value| {
                value.parse::<i64>().map_err(|_| {
                    error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid_field",
                        format!("{name} must be an integer"),
                    )
                })
            })
            .transpose()
    };
    let folder_id = take_numeric("folder_id")?;
    let expires_in = take_numeric("expires_in")?;
    let read_limit = take_numeric("read_limit")?;
    let flat = FlatCreateRequest {
        title: values.remove("title"),
        format: values.remove("format"),
        content: values.remove("content"),
        language: values.remove("language"),
        visibility: values.remove("visibility"),
        folder_id,
        expires_at: values.remove("expires_at"),
        expires_in,
        read_limit,
    };
    Ok((
        flat.structured()
            .map_err(|message| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message))?,
        files,
    ))
}

async fn promote_created_files(
    services: &PasteService,
    principal: &Principal,
    paste: &mut crate::services::Paste,
    staged: &mut [StagedFile],
) -> Result<(), String> {
    let directory = services
        .storage
        .data_dir
        .join("attachments")
        .join(&paste.id);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| error.to_string())?;
    let mut promoted = Vec::new();
    for file in staged.iter_mut() {
        let destination = super::attachments::attachment_path(
            &services.storage.data_dir,
            &paste.id,
            &file.storage_key,
        )?;
        if let Err(error) = tokio::fs::rename(&file.temporary, &destination).await {
            for path in promoted {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(error.to_string());
        }
        file.temporary = PathBuf::new();
        promoted.push(destination);
    }
    let inputs = staged
        .iter()
        .map(|file| {
            (
                file.filename.clone(),
                file.storage_key.clone(),
                file.size_bytes,
            )
        })
        .collect::<Vec<_>>();
    match services
        .add_attachments(principal, &paste.id, &inputs, None)
        .await
    {
        Ok(_) => {
            *paste = services
                .get_source(principal, &paste.id)
                .await?
                .ok_or("Paste disappeared after attachment upload")?;
            Ok(())
        }
        Err(message) => {
            for path in promoted {
                let _ = tokio::fs::remove_file(path).await;
            }
            Err(message)
        }
    }
}

fn request_fingerprint(request: &CreatePasteRequest, files: &[StagedFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(request).unwrap_or_default());
    for file in files {
        digest.update(file.filename.as_bytes());
        digest.update(file.size_bytes.to_be_bytes());
        digest.update(file.digest.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

async fn read_body(payload: &mut web::Payload, limit: usize) -> Result<web::Bytes, HttpResponse> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk
            .map_err(|reason| error(StatusCode::BAD_REQUEST, "invalid_body", reason.to_string()))?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Request body exceeds 2 MiB",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

async fn read_field(
    field: &mut actix_multipart::Field,
    limit: usize,
) -> Result<Vec<u8>, HttpResponse> {
    let mut body = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|reason| {
            error(
                StatusCode::BAD_REQUEST,
                "invalid_multipart",
                reason.to_string(),
            )
        })?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "field_too_large",
                "Multipart text field exceeds 2 MiB",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn resource_response(
    req: &HttpRequest,
    principal: &Principal,
    paste: crate::services::Paste,
    include_body: bool,
    grant: Option<&str>,
) -> HttpResponse {
    let tag = dto::etag(&paste);
    HttpResponse::Ok()
        .insert_header((header::ETAG, tag))
        .json(dto::resource(req, principal, paste, include_body, grant))
}

fn content_response(
    req: &HttpRequest,
    principal: &Principal,
    paste: crate::services::Paste,
    grant: Option<&str>,
) -> HttpResponse {
    let tag = dto::etag(&paste);
    if accepts(req, "text/plain") {
        return HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/plain; charset=utf-8")
            .body(paste.content);
    }
    if accepts(req, "text/html") {
        if paste.content_kind != "rich_text" {
            return error(
                StatusCode::NOT_ACCEPTABLE,
                "not_acceptable",
                "HTML is available only for rich-text pastes",
            );
        }
        return HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/html; charset=utf-8")
            .body(
                paste
                    .document
                    .as_ref()
                    .map(crate::services::document_to_html)
                    .unwrap_or_default(),
            );
    }
    resource_response(req, principal, paste, true, grant)
}

fn accepts(req: &HttpRequest, mime: &str) -> bool {
    req.headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .any(|part| part.trim().split(';').next() == Some(mime))
        })
}

fn idempotency_key(req: &HttpRequest) -> Result<Option<String>, HttpResponse> {
    let Some(value) = req.headers().get("Idempotency-Key") else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_idempotency_key",
            "Idempotency-Key must be ASCII",
        )
    })?;
    if value.is_empty()
        || value.len() > 200
        || value.bytes().any(|byte| !(0x21..=0x7e).contains(&byte))
    {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_idempotency_key",
            "Idempotency-Key must contain 1 to 200 printable ASCII characters without spaces",
        ));
    }
    Ok(Some(value.to_string()))
}

pub(crate) fn require_match(
    req: &HttpRequest,
    paste: &crate::services::Paste,
) -> Result<Option<i64>, HttpResponse> {
    let Some(value) = req.headers().get(header::IF_MATCH) else {
        return Err(error(
            StatusCode::PRECONDITION_REQUIRED,
            "precondition_required",
            "If-Match is required; use the current ETag or *",
        ));
    };
    let value = value.to_str().unwrap_or("");
    if value == "*" {
        Ok(None)
    } else if value
        .split(',')
        .any(|candidate| candidate.trim() == dto::etag(paste))
    {
        Ok(Some(paste.revision))
    } else {
        Err(error(
            StatusCode::PRECONDITION_FAILED,
            "precondition_failed",
            "Paste changed since it was loaded",
        ))
    }
}
