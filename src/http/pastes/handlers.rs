use super::*;

#[utoipa::path(
    get,
    path = "/pastes",
    tag = "pastes",
    params(ApiPasteQuery),
    responses(
        (status = 200, description = "Paginated paste summaries", body = PastePage),
        (status = 400, description = "Invalid filter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required for owner=me, or bearer credential invalid", body = crate::http::errors::ProblemDetails),
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
    let public_explore_enabled = match crate::instance::settings::get(&services.storage).await {
        Ok(settings) => settings.public_explore_enabled,
        Err(value) => return domain_error(value),
    };
    if !query.mine.unwrap_or(false) && !public_explore_enabled {
        return error(
            StatusCode::NOT_FOUND,
            "not_found",
            "Public discovery is disabled",
        );
    }
    if let Err(error) = crate::pastes::validate_paste_query(&query) {
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
            let total_pages = contract::total_pages(page.total_items, page.page_size);
            HttpResponse::Ok().json(PastePage {
                items: page
                    .items
                    .into_iter()
                    .map(|paste| contract::summary(&req, &principal, paste, false))
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
    description = "Creates a paste. Query metadata is accepted only for raw bodies. text/plain creates text and accepts an optional language; text/markdown creates canonical Markdown; text/html imports supported markup into canonical Markdown. The raw request body is always the content. JSON, URL-encoded, and multipart requests carry creation fields exclusively in the body. Multipart requests require nonempty content or at least one file; text-only multipart remains available when attachment uploads are disabled. An omitted structured body creates empty text. expires_at and expires_in are mutually exclusive. Clients may request text/plain instead of JSON to receive only the created paste URL.",
    params(
        RawCreateQuery,
        ("Idempotency-Key" = Option<String>, Header, description = "Recommended unique key for safely retrying creation"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations; not used with bearer keys")
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
                (crate::http::contract::PasteResource = "application/json"),
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
    query: web::Query<RawCreateQuery>,
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
    ) && query != RawCreateQuery::default()
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
    let (mut request, mut staged) = match parsed {
        Ok(value) => value,
        Err(response) => return response,
    };
    let settings = match crate::instance::settings::get(&services.storage).await {
        Ok(settings) => settings,
        Err(error) => return domain_error(error),
    };
    if request.visibility.is_none() {
        request.visibility = Some(settings.default_visibility);
    }
    if request.expires_at.is_none() && request.expires_in.is_none() {
        request.expires_in = settings.default_expiration_seconds;
    }
    if let Some(BodyInput::Text { language, .. }) = request.body.as_mut() {
        if language.is_none() {
            *language = Some(settings.default_language);
        }
    }
    let has_content = match request.body.as_ref() {
        Some(BodyInput::Text { content, .. } | BodyInput::Markdown { content }) => {
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
    let (mut paste, replayed, operation_token) = match services
        .create_paste_idempotent(
            &principal,
            &input,
            idempotency_key.as_deref(),
            &fingerprint,
            staged.len(),
        )
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
            if let (Some(key), Some(token)) =
                (idempotency_key.as_deref(), operation_token.as_deref())
            {
                let _ = services
                    .rollback_create_idempotency(&principal, key, token)
                    .await;
            } else if !replayed {
                let _ = services
                    .rollback_pending_creation(&principal, &paste.id)
                    .await;
            }
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_attachment",
                message,
            );
        }
    }
    if let (Some(key), Some(token)) = (idempotency_key.as_deref(), operation_token.as_deref()) {
        match services
            .complete_create_idempotency(&principal, key, token)
            .await
        {
            Ok(true) => {}
            Ok(false) => {
                return internal("Paste creation operation ownership was lost");
            }
            Err(value) => return domain_error(value),
        }
        paste = match services.get_paste(&principal, &paste.id).await {
            Ok(Some(paste)) => paste,
            Ok(None) => return internal("Completed paste is unavailable"),
            Err(value) => return domain_error(value),
        };
    } else if !staged.is_empty() && !replayed {
        paste = match services
            .complete_pending_creation(&principal, &paste.id)
            .await
        {
            Ok(paste) => paste,
            Err(value) => return domain_error(value),
        };
    }
    let tag = contract::etag(&paste);
    let resource = match contract::resource(&req, &principal, paste, None) {
        Ok(resource) => resource,
        Err(value) => return domain_error(value),
    };
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
        (status = 200, description = "Paste metadata without consuming a read", body = crate::http::contract::PasteMetadataResource,
            headers(("ETag" = String, description = "Current paste entity tag"))),
        (status = 401, description = "Invalid bearer credential", body = crate::http::errors::ProblemDetails),
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
    description = "Returns JSON by default. Clients may negotiate a plain-text projection, canonical Markdown, or sanitized rendered HTML.",
    params(("paste_id" = String, Path, description = "Paste ID")),
    responses(
        (status = 200, description = "Non-consuming owner or administrator source",
            content(
                (crate::http::contract::PasteResource = "application/json"),
                (String = "text/plain"),
                (String = "text/markdown"),
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

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
struct RawPasteQuery {
    /// Short-lived grant returned by a deliberate read of a read-limited paste.
    read_token: Option<String>,
}

#[utoipa::path(
    get, path = "/pastes/{paste_id}/raw", tag = "pastes",
    description = "Returns a paste's native content without the Racebin interface. Ordinary visible pastes have stable, non-consuming raw URLs. A read-limited paste requires owner access or the short-lived read_token returned by POST /pastes/{paste_id}/reads, preventing crawlers and link previews from consuming a read.",
    params(("paste_id" = String, Path, description = "Paste ID"), RawPasteQuery),
    responses(
        (status = 200, description = "Native plain text or canonical Markdown",
            content((String = "text/plain"), (String = "text/markdown")),
            headers(("ETag" = String, description = "Current paste entity tag"))),
        (status = 401, description = "Invalid bearer credential", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "API key lacks paste:read", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found, not visible, or read grant invalid", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/pastes/{paste_id}/raw")]
pub(crate) async fn get_paste_raw(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
    query: web::Query<RawPasteQuery>,
) -> HttpResponse {
    let principal = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste_id = paste_id.into_inner();
    let paste = match services.get_paste(&principal, &paste_id).await {
        Ok(Some(paste))
            if paste.read_limit.is_none() || principal.can_bypass_read_limit(paste.owner_id) =>
        {
            Some(paste)
        }
        Ok(_) => match query.read_token.as_deref() {
            Some(token) => match services.get_paste_with_grant(&paste_id, token).await {
                Ok(paste) => paste,
                Err(value) => return domain_error(value),
            },
            None => None,
        },
        Err(value) => return domain_error(value),
    };
    match paste {
        Some(paste) => raw_content_response(paste),
        None => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
    }
}

#[utoipa::path(
    post, path = "/pastes/{paste_id}/reads", tag = "pastes",
    description = "Consumes a permitted read and returns JSON by default. A browser-session owner reading their own paste does not increment its read count or consume its read limit. Clients may instead negotiate text/plain, or text/html for rich-text pastes.",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("Idempotency-Key" = Option<String>, Header, description = "Recommended key for safely retrying a consuming read")),
    responses(
        (status = 200, description = "Paste content; this may consume a limited read",
            content(
                (crate::http::contract::PasteResource = "application/json"),
                (String = "text/plain"),
                (String = "text/html")
            ),
            headers(
                ("ETag" = String, description = "Current paste entity tag"),
                ("Read-Token" = String, description = "Short-lived grant for raw content and attachment downloads after a deliberate read of a read-limited paste"),
                ("Idempotency-Replayed" = bool, description = "true when this is a replay of an earlier idempotent read")
            )),
        (status = 400, description = "Invalid idempotency key", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Invalid bearer credential", body = crate::http::errors::ProblemDetails),
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
        (status = 200, description = "Paste updated", body = crate::http::contract::PasteResource,
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
        (BodyInput::Text { content, .. }, "markdown") => Ok(BodyInput::Markdown {
            content: crate::pastes::text_to_markdown(content),
        }),
        (BodyInput::Markdown { content }, "text") => {
            crate::pastes::render_markdown(content).map(|rendered| BodyInput::Text {
                content: rendered.plain_text,
                language: Some("plaintext".into()),
            })
        }
        (source @ BodyInput::Text { .. }, "text") => Ok(source.clone()),
        (source @ BodyInput::Markdown { content }, "markdown") => {
            crate::pastes::render_markdown(content).map(|_| source.clone())
        }
        _ => Err("Conversion supports only text and markdown".into()),
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
    paste: crate::pastes::Paste,
    include_body: bool,
    grant: Option<&str>,
) -> HttpResponse {
    let tag = contract::etag(&paste);
    if include_body {
        match contract::resource(req, principal, paste, grant) {
            Ok(resource) => HttpResponse::Ok()
                .insert_header((header::ETAG, tag))
                .json(resource),
            Err(value) => domain_error(value),
        }
    } else {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .json(contract::metadata_resource(req, principal, paste, grant))
    }
}

fn content_response(
    req: &HttpRequest,
    principal: &Principal,
    paste: crate::pastes::Paste,
    grant: Option<&str>,
) -> HttpResponse {
    let tag = contract::etag(&paste);
    let mut response = if accepts(req, "text/markdown") && paste.content_kind == "markdown" {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/markdown; charset=utf-8")
            .body(paste.content)
    } else if accepts(req, "text/plain") {
        let content = if paste.content_kind == "markdown" {
            crate::pastes::render_markdown(&paste.content)
                .map(|value| value.plain_text)
                .unwrap_or_default()
        } else {
            paste.content
        };
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .content_type("text/plain; charset=utf-8")
            .body(content)
    } else if accepts(req, "text/html") {
        if paste.content_kind != "markdown" {
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
                crate::pastes::render_markdown(&paste.content)
                    .map(|value| value.html)
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

fn raw_content_response(paste: crate::pastes::Paste) -> HttpResponse {
    let tag = contract::etag(&paste);
    if paste.content_kind == "markdown" {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .insert_header((header::CONTENT_TYPE, "text/markdown; charset=utf-8"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .insert_header(("Referrer-Policy", "no-referrer"))
            .body(paste.content)
    } else {
        HttpResponse::Ok()
            .insert_header((header::ETAG, tag))
            .insert_header((header::CONTENT_TYPE, "text/plain; charset=utf-8"))
            .insert_header(("X-Content-Type-Options", "nosniff"))
            .insert_header(("Referrer-Policy", "no-referrer"))
            .body(paste.content)
    }
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
    paste: &crate::pastes::Paste,
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
        .any(|candidate| candidate.trim() == contract::etag(paste))
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
