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
    params(ApiKeyQuery),
    responses(
        (status = 200, description = "Paginated API keys owned by the authenticated user", body = crate::http::contract::ApiKeyPage),
        (status = 400, description = "Invalid filter, sort, or pagination parameter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/account/api-keys")]
pub(crate) async fn list_keys(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<ApiKeyQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let query = query.into_inner();
    let (page, page_size) =
        match crate::http::contract::page_parameters(query.page, query.page_size, 25) {
            Ok(value) => value,
            Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
        };
    let enabled = match query.status.as_deref() {
        None | Some("all") => None,
        Some("enabled") => Some(true),
        Some("disabled") => Some(false),
        Some(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_query",
                "Status must be enabled or disabled",
            )
        }
    };
    let sort = query.sort.as_deref().unwrap_or("created");
    if !matches!(sort, "created" | "name" | "used") {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Unknown API-key sort field",
        );
    }
    let descending = query.direction.as_deref().unwrap_or("desc") == "desc";
    if !matches!(query.direction.as_deref(), None | Some("asc" | "desc")) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Direction must be asc or desc",
        );
    }
    match services
        .list_api_keys(
            &value,
            &crate::pastes::ApiKeyListOptions {
                search: query.search.as_deref(),
                enabled,
                sort,
                descending,
                page,
                page_size,
            },
        )
        .await
    {
        Ok(v) => HttpResponse::Ok().json(contract::ApiKeyPage {
            items: v
                .items
                .into_iter()
                .map(contract::ApiKeyResource::from)
                .collect(),
            pagination: crate::http::contract::Pagination {
                page: v.page,
                page_size: v.page_size,
                total_items: v.total_items,
                total_pages: crate::http::contract::total_pages(v.total_items, v.page_size),
            },
        }),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ApiKeyQuery {
    /// Search key name, token prefix, and scope.
    search: Option<String>,
    /// Restrict results to `enabled` or `disabled` keys.
    status: Option<String>,
    /// Order by `created`, `name`, or `used`.
    sort: Option<String>,
    /// Sort in `asc` or `desc` order.
    direction: Option<String>,
    /// One-based result page.
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
    /// Results per page, from 1 through 100.
    #[param(minimum = 1, maximum = 100, default = 25)]
    page_size: Option<u32>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct KeyInput {
    #[schema(min_length = 1, max_length = 100)]
    name: String,
    #[schema(value_type = std::collections::HashSet<String>, min_items = 1)]
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
