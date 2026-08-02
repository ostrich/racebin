use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_keys)
        .service(create_key)
        .service(update_key)
        .service(delete_key);
}

#[utoipa::path(
    get, path = "/account/api-keys", tag = "api keys",
    responses(
        (status = 200, description = "API keys owned by the authenticated user", body = [crate::http::contract::ApiKeyResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/account/api-keys")]
pub(crate) async fn list_keys(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.list_api_keys(&value).await {
        Ok(v) => HttpResponse::Ok().json(
            v.into_iter()
                .map(contract::ApiKeyResource::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct KeyInput {
    name: String,
    scopes: Vec<String>,
}

#[utoipa::path(
    post, path = "/account/api-keys", tag = "api keys",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = KeyInput,
    responses(
        (status = 201, description = "API key created; token is returned only once", body = crate::http::contract::ApiKeyCreatedResponse),
        (status = 400, description = "Invalid name, scopes, or delegated privileges", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/account/api-keys")]
pub(crate) async fn create_key(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<KeyInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services
        .create_api_key(&value, &body.name, &body.scopes)
        .await
    {
        Ok((key, token)) => HttpResponse::Created().json(contract::ApiKeyCreatedResponse {
            key: key.into(),
            token,
        }),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    patch, path = "/account/api-keys/{id}", tag = "api keys",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = EnabledInput,
    responses(
        (status = 204, description = "API key state updated"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/account/api-keys/{id}")]
pub(crate) async fn update_key(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<EnabledInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services
        .set_api_key_enabled(&value, *id, body.enabled)
        .await
    {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    delete, path = "/account/api-keys/{id}", tag = "api keys",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/account/api-keys/{id}")]
pub(crate) async fn delete_key(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.delete_api_key(&value, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}
