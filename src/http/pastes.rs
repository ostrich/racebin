use super::paste_payload::{
    parse_multipart, parse_non_multipart, promote_created_files, request_fingerprint,
};
use super::*;
use crate::http::dto::{
    self, BodyInput, CreatePasteRequest, Pagination, PastePage, UpdatePasteRequest,
};

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

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(super) struct ApiPasteQuery {
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
    pub(super) fn into_internal(self) -> Result<PasteQuery, String> {
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

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(super) struct FlatCreateRequest {
    pub(super) title: Option<String>,
    pub(super) format: Option<String>,
    pub(super) content: Option<String>,
    pub(super) language: Option<String>,
    pub(super) visibility: Option<String>,
    pub(super) folder_id: Option<i64>,
    pub(super) expires_at: Option<String>,
    pub(super) expires_in: Option<i64>,
    pub(super) read_limit: Option<i64>,
}

/// Schema-only representation of the atomic multipart create request.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct MultipartCreateRequest {
    title: Option<String>,
    format: Option<String>,
    content: Option<String>,
    language: Option<String>,
    visibility: Option<String>,
    folder_id: Option<i64>,
    expires_at: Option<String>,
    expires_in: Option<i64>,
    read_limit: Option<i64>,
    #[schema(content_media_type = "application/octet-stream")]
    file: Vec<String>,
}

impl FlatCreateRequest {
    pub(super) fn structured(self) -> Result<CreatePasteRequest, String> {
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

pub(super) struct StagedFile {
    pub(super) temporary: PathBuf,
    pub(super) filename: String,
    pub(super) storage_key: String,
    pub(super) size_bytes: i64,
    pub(super) digest: String,
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[utoipa::path(
    get,
    path = "/pastes",
    tag = "pastes",
    params(ApiPasteQuery),
    responses(
        (status = 200, description = "Paginated paste summaries", body = PastePage),
        (status = 400, description = "Invalid filter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required for owner=me", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "API key lacks paste:list", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
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
    if let Err(error) = crate::services::validate_paste_query(&query) {
        return domain_error(error);
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
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    post,
    path = "/pastes",
    tag = "pastes",
    params(
        FlatCreateRequest,
        ("Idempotency-Key" = Option<String>, Header, description = "Recommended unique key for safely retrying creation"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations; not used with bearer keys"),
        ("Accept" = Option<String>, Header, description = "Use text/plain to receive only the created paste URL")
    ),
    request_body(
        description = "Canonical JSON, raw text/HTML, URL-encoded form, or multipart paste payload",
        content(
            (CreatePasteRequest = "application/json"),
            (String = "text/plain"),
            (String = "text/markdown"),
            (String = "text/html"),
            (FlatCreateRequest = "application/x-www-form-urlencoded"),
            (MultipartCreateRequest = "multipart/form-data")
        )
    ),
    responses(
        (status = 201, description = "Paste created",
            content(
                (crate::http::dto::PasteResource = "application/json"),
                (String = "text/plain")
            ),
            headers(
                ("Location" = String, description = "Absolute URL of the created paste"),
                ("ETag" = String, description = "Current paste entity tag"),
                ("Idempotency-Replayed" = String, description = "true when this is a replay of an earlier idempotent creation")
            )),
        (status = 400, description = "Malformed request or idempotency key", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Idempotency-key conflict", body = crate::http::errors::ProblemDetails),
        (status = 413, description = "Upload exceeds configured limits", body = crate::http::errors::ProblemDetails),
        (status = 422, description = "Invalid paste content or metadata", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => return domain_error(value),
    };
    let attachments_complete = replayed
        && staged.iter().all(|file| {
            paste.attachments.iter().any(|attachment| {
                attachment.filename == file.filename && attachment.size_bytes == file.size_bytes
            })
        });
    if !staged.is_empty() && !attachments_complete {
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

#[utoipa::path(
    get, path = "/pastes/{paste_id}", tag = "pastes",
    params(("paste_id" = String, Path, description = "Paste ID")),
    responses(
        (status = 200, description = "Paste metadata without consuming a read", body = crate::http::dto::PasteResource,
            headers(("ETag" = String, description = "Current paste entity tag"))),
        (status = 404, description = "Paste not found or not visible", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    get, path = "/pastes/{paste_id}/source", tag = "pastes",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("Accept" = Option<String>, Header, description = "application/json, text/plain, or text/html for rich text")),
    responses(
        (status = 200, description = "Non-consuming owner or administrator source",
            content(
                (crate::http::dto::PasteResource = "application/json"),
                (String = "text/plain"),
                (String = "text/html")
            ),
            headers(("ETag" = String, description = "Current paste entity tag"))),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Not the owner and lacks paste:manage", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found", body = crate::http::errors::ProblemDetails),
        (status = 406, description = "Requested representation is unavailable", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    post, path = "/pastes/{paste_id}/reads", tag = "pastes",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("Idempotency-Key" = Option<String>, Header, description = "Recommended key for safely retrying a consuming read"),
        ("Accept" = Option<String>, Header, description = "application/json, text/plain, or text/html for rich text")),
    responses(
        (status = 200, description = "Paste content; this may consume a limited read",
            content(
                (crate::http::dto::PasteResource = "application/json"),
                (String = "text/plain"),
                (String = "text/html")
            ),
            headers(
                ("ETag" = String, description = "Current paste entity tag"),
                ("Idempotency-Replayed" = String, description = "true when this is a replay of an earlier idempotent read")
            )),
        (status = 400, description = "Invalid idempotency key", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "API key lacks paste:read", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found, unavailable, or fully consumed", body = crate::http::errors::ProblemDetails),
        (status = 406, description = "Requested representation is unavailable", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Idempotency-key conflict", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    patch, path = "/pastes/{paste_id}", tag = "pastes",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("If-Match" = String, Header, description = "Current ETag or *"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = UpdatePasteRequest,
    responses(
        (status = 200, description = "Paste updated", body = crate::http::dto::PasteResource,
            headers(("ETag" = String, description = "New paste entity tag"))),
        (status = 400, description = "Invalid update", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Not permitted to update this paste", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found", body = crate::http::errors::ProblemDetails),
        (status = 412, description = "ETag does not match", body = crate::http::errors::ProblemDetails),
        (status = 428, description = "If-Match is required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => return domain_error(value),
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
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    delete, path = "/pastes/{paste_id}", tag = "pastes",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("If-Match" = String, Header, description = "Current ETag or *"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "Paste deleted"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Not permitted to delete this paste", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found", body = crate::http::errors::ProblemDetails),
        (status = 412, description = "ETag does not match", body = crate::http::errors::ProblemDetails),
        (status = 428, description = "If-Match is required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
        Err(value) => return domain_error(value),
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
        Err(value) => domain_error(value),
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

#[utoipa::path(
    post, path = "/content-conversions", tag = "pastes",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = ConversionInput,
    responses(
        (status = 200, description = "Converted content", body = ConversionOutput),
        (status = 400, description = "Malformed request", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 422, description = "Unsupported conversion", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
