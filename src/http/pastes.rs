use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_pastes)
        .service(create_paste)
        .service(convert_paste_content)
        .service(consume_paste)
        .service(get_paste)
        .service(update_paste)
        .service(delete_paste)
        .service(raw_paste);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionInput {
    source_kind: String,
    target_kind: String,
    content: Option<String>,
    document: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ConversionOutput {
    content: String,
    document: Option<serde_json::Value>,
}

#[post("/pastes/convert")]
async fn convert_paste_content(
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
    let result = match (body.source_kind.as_str(), body.target_kind.as_str()) {
        ("text", "rich_text") => {
            let content = body.content.as_deref().unwrap_or("");
            let document = text_to_document(content);
            Ok(ConversionOutput {
                content: content.to_string(),
                document: Some(document),
            })
        }
        ("rich_text", "text") => {
            let document = body
                .document
                .as_ref()
                .ok_or_else(|| "Rich-text conversion requires a document".to_string());
            document.and_then(|document| {
                validate_document(document).map(|content| ConversionOutput {
                    content,
                    document: None,
                })
            })
        }
        (source, target) if source == target && source == "text" => Ok(ConversionOutput {
            content: body.content.clone().unwrap_or_default(),
            document: None,
        }),
        (source, target) if source == target && source == "rich_text" => body
            .document
            .as_ref()
            .ok_or_else(|| "Rich-text conversion requires a document".to_string())
            .and_then(|document| {
                validate_document(document).map(|content| ConversionOutput {
                    content,
                    document: Some(document.clone()),
                })
            }),
        _ => Err("Conversion supports only text and rich_text".to_string()),
    };
    match result {
        Ok(output) => HttpResponse::Ok().json(output),
        Err(message) => error(StatusCode::BAD_REQUEST, "invalid_conversion", message),
    }
}

#[get("/pastes")]
async fn list_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<PasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if query.mine.unwrap_or(false) && value.user_id().is_none() {
        return error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication required for mine=true",
        );
    }
    if query
        .visibility
        .as_deref()
        .is_some_and(|visibility| !matches!(visibility, "public" | "unlisted" | "private"))
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Visibility must be public, unlisted, or private",
        );
    }
    let wants_private = query.owner_id.is_some() || query.visibility.as_deref() != Some("public");
    if matches!(value, Principal::ApiKey(_)) && wants_private && !value.can("paste:list") {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing paste:list permission",
        );
    }
    match services.list_pastes(&value, &query, false).await {
        Ok(page) => HttpResponse::Ok().json(page),
        Err(e) => internal(e),
    }
}

#[post("/pastes")]
async fn create_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<PasteInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.create_paste(&value, &body).await {
        Ok(paste) => HttpResponse::Created().json(paste),
        Err(e) => paste_error(e),
    }
}

#[get("/pastes/{paste_id}")]
async fn get_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.get_paste(&value, &paste_id).await {
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}

#[get("/pastes/{paste_id}/consume")]
async fn consume_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.consume_paste(&value, &paste_id).await {
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}

#[patch("/pastes/{paste_id}")]
async fn update_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
    body: web::Json<PasteInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.update_paste(&value, &paste_id, &body).await {
        Ok(Some(_)) if matches!(&value, Principal::ApiKey(key) if !key.has_scope("paste:read") && !key.has_scope("paste:manage")) => {
            HttpResponse::NoContent().finish()
        }
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => paste_error(e),
    }
}

#[delete("/pastes/{paste_id}")]
async fn delete_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.delete_paste(&value, &paste_id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) if e == "You do not own this paste" || e.starts_with("Missing ") => {
            error(StatusCode::FORBIDDEN, "forbidden", e)
        }
        Err(e) => internal(e),
    }
}

#[get("/pastes/{paste_id}/raw")]
async fn raw_paste(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.consume_paste(&value, &paste_id).await {
        Ok(Some(paste)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "text/plain; charset=utf-8"))
            .body(paste.content),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}
