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
    /// One-based result page. Defaults to 1.
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
    /// Results per page. Defaults to 30; the maximum is 100.
    #[param(minimum = 1, maximum = 100, default = 30)]
    page_size: Option<u32>,
    /// Case-insensitive search across paste ID, title, content, language, and attachment filename. Administrative listings also search owner usernames.
    q: Option<String>,
    /// Visibility identifier advertised by `/capabilities`.
    visibility: Option<String>,
    /// Restrict results to resources owned by the authenticated user.
    owner: Option<OwnerFilter>,
    /// Positive folder ID. Requires `owner=me` and cannot be combined with `unfiled=true`.
    #[param(minimum = 1)]
    folder_id: Option<i64>,
    /// Restrict results to pastes without a folder. Requires `owner=me` when true and cannot be combined with `folder_id`.
    unfiled: Option<bool>,
    /// Content format identifier advertised by `/capabilities`.
    format: Option<String>,
    /// Syntax identifier advertised by `/languages`.
    language: Option<String>,
    /// Restrict results according to whether at least one attachment exists.
    has_attachments: Option<bool>,
    /// Inclusive lower bound on creation time, expressed as RFC 3339.
    #[param(format = DateTime)]
    created_after: Option<String>,
    /// Inclusive upper bound on creation time, expressed as RFC 3339.
    #[param(format = DateTime)]
    created_before: Option<String>,
    /// Restrict results by whether expiration is scheduled.
    expiration: Option<ExpirationFilter>,
    /// Inclusive minimum read count.
    #[param(minimum = 0)]
    min_reads: Option<i64>,
    /// Inclusive maximum read count.
    #[param(minimum = 0)]
    max_reads: Option<i64>,
    /// Inclusive minimum total size in bytes, including attachments.
    #[param(minimum = 0)]
    min_size_bytes: Option<i64>,
    /// Inclusive maximum total size in bytes, including attachments.
    #[param(minimum = 0)]
    max_size_bytes: Option<i64>,
    /// Restrict results according to whether a read limit is configured.
    read_limit: Option<ReadLimitFilter>,
    /// Field used to order results. Defaults to `created`.
    #[param(default = "created")]
    sort: Option<PasteSort>,
    /// Sort direction. Defaults to `desc`.
    #[param(default = "desc")]
    direction: Option<SortDirection>,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OwnerFilter {
    Me,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ExpirationFilter {
    Never,
    Scheduled,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReadLimitFilter {
    Unlimited,
    Limited,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PasteSort {
    Created,
    Title,
    Reads,
    Expires,
    Size,
}

#[derive(Clone, Deserialize, PartialEq, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}

macro_rules! filter_as_str {
    ($type:ty, {$($variant:path => $value:literal),+ $(,)?}) => {
        impl $type {
            fn as_str(&self) -> &'static str {
                match self { $($variant => $value),+ }
            }
        }
    };
}

filter_as_str!(ExpirationFilter, {
    ExpirationFilter::Never => "never",
    ExpirationFilter::Scheduled => "scheduled",
});
filter_as_str!(ReadLimitFilter, {
    ReadLimitFilter::Unlimited => "unlimited",
    ReadLimitFilter::Limited => "limited",
});
filter_as_str!(PasteSort, {
    PasteSort::Created => "created",
    PasteSort::Title => "title",
    PasteSort::Reads => "reads",
    PasteSort::Expires => "expires",
    PasteSort::Size => "size",
});
filter_as_str!(SortDirection, {
    SortDirection::Asc => "asc",
    SortDirection::Desc => "desc",
});

impl ApiPasteQuery {
    pub(super) fn into_internal(self) -> Result<PasteQuery, String> {
        if self.page == Some(0) {
            return Err("page must be at least 1".into());
        }
        if self
            .page_size
            .is_some_and(|value| !(1..=100).contains(&value))
        {
            return Err("page_size must be between 1 and 100".into());
        }
        let mine = self.owner.is_some();
        Ok(PasteQuery {
            page: self.page,
            page_size: self.page_size,
            search: self.q,
            visibility: self.visibility,
            owner_id: None,
            folder_id: self.folder_id,
            unfiled: self.unfiled,
            mine: Some(mine),
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
            expiration: self.expiration.map(|value| value.as_str().into()),
            min_reads: self.min_reads,
            max_reads: self.max_reads,
            min_size_bytes: self.min_size_bytes,
            max_size_bytes: self.max_size_bytes,
            read_limit: self.read_limit.map(|value| value.as_str().into()),
            sort: self.sort.map(|value| value.as_str().into()),
            direction: self.direction.map(|value| value.as_str().into()),
        })
    }
}

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(super) struct FlatCreateRequest {
    #[schema(max_length = 200)]
    #[param(max_length = 200)]
    pub(super) title: Option<String>,
    pub(super) format: Option<String>,
    pub(super) content: Option<String>,
    pub(super) language: Option<String>,
    pub(super) visibility: Option<String>,
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) folder_id: Option<i64>,
    /// Absolute RFC 3339 expiration time. Cannot be combined with `expires_in`.
    #[schema(format = DateTime)]
    #[param(format = DateTime)]
    pub(super) expires_at: Option<String>,
    /// Positive lifetime in seconds from creation. Cannot be combined with `expires_at`.
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) expires_in: Option<i64>,
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) read_limit: Option<i64>,
}

/// Schema-only representation of the atomic multipart create request.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct MultipartCreateRequest {
    #[schema(max_length = 200)]
    title: Option<String>,
    format: Option<String>,
    content: Option<String>,
    language: Option<String>,
    visibility: Option<String>,
    #[schema(minimum = 1)]
    folder_id: Option<i64>,
    /// Absolute RFC 3339 expiration time. Cannot be combined with `expires_in`.
    #[schema(format = DateTime)]
    expires_at: Option<String>,
    /// Positive lifetime in seconds from creation. Cannot be combined with `expires_at`.
    #[schema(minimum = 1)]
    expires_in: Option<i64>,
    #[schema(minimum = 1)]
    read_limit: Option<i64>,
    #[schema(value_type = Vec<Value>, min_items = 1)]
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
    description = "Creates a paste. Query creation fields are accepted only for text/plain, text/markdown, and text/html request bodies. JSON, URL-encoded, and multipart requests carry creation fields exclusively in the body. expires_at and expires_in are mutually exclusive.",
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
                ("Idempotency-Replayed" = bool, description = "true when this is a replay of an earlier idempotent creation")
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
    let query = query.into_inner();
    if matches!(
        content_type.as_str(),
        "application/json" | "application/x-www-form-urlencoded" | "multipart/form-data"
    ) && query != FlatCreateRequest::default()
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Creation query parameters are accepted only with text/plain, text/markdown, or text/html bodies",
        );
    }
    let parsed = if content_type == "multipart/form-data" {
        parse_multipart(&req, payload, &services).await
    } else {
        parse_non_multipart(&content_type, query, payload)
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
    let resource = dto::resource(&req, &principal, paste, None);
    let mut response = if accepts(&req, "text/plain") {
        HttpResponse::Created()
            .content_type("text/plain; charset=utf-8")
            .body(resource.metadata.url.clone())
    } else {
        HttpResponse::Created().json(resource.clone())
    };
    response.headers_mut().insert(
        header::LOCATION,
        header::HeaderValue::from_str(&resource.metadata.url).unwrap(),
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
        (status = 200, description = "Paste metadata without consuming a read", body = crate::http::dto::PasteMetadataResource,
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
                ("Read-Token" = String, description = "Short-lived attachment-download grant issued when a limited read consumes the paste's final permitted read"),
                ("Idempotency-Replayed" = bool, description = "true when this is a replay of an earlier idempotent read")
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
        (status = 422, description = "Invalid update", body = crate::http::errors::ProblemDetails),
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
    let principal = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    if matches!(principal, Principal::ApiKey(_)) && !principal.can("paste:write") {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing paste:write permission",
        );
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
    if include_body {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .json(dto::resource(req, principal, paste, grant))
    } else {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .json(dto::metadata_resource(req, principal, paste, grant))
    }
}

fn content_response(
    req: &HttpRequest,
    principal: &Principal,
    paste: crate::services::Paste,
    grant: Option<&str>,
) -> HttpResponse {
    let tag = dto::etag(&paste);
    let mut response = if accepts(req, "text/plain") {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/plain; charset=utf-8")
            .body(paste.content)
    } else if accepts(req, "text/html") {
        if paste.content_kind != "rich_text" {
            return error(
                StatusCode::NOT_ACCEPTABLE,
                "not_acceptable",
                "HTML is available only for rich-text pastes",
            );
        }
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/html; charset=utf-8")
            .body(
                paste
                    .document
                    .as_ref()
                    .map(crate::services::document_to_html)
                    .unwrap_or_default(),
            )
    } else {
        resource_response(req, principal, paste, true, grant)
    };
    if let Some(grant) = grant {
        response.headers_mut().insert(
            header::HeaderName::from_static("read-token"),
            header::HeaderValue::from_str(grant).expect("generated read token is a valid header"),
        );
    }
    response
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
