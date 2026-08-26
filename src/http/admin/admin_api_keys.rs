use super::*;

#[utoipa::path(
    get, path = "/admin/api-keys", tag = "administration",
    params(AdminApiKeyQuery),
    responses(
        (status = 200, description = "Paginated API keys", body = crate::http::contract::ApiKeyPage),
        (status = 400, description = "Invalid filter, sort, or pagination parameter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/api-keys")]
pub(crate) async fn admin_keys(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<AdminApiKeyQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    let query = query.into_inner();
    let enabled = match query.status.as_deref() {
        None | Some("all") => None,
        Some("enabled") => Some(true),
        Some("disabled") => Some(false),
        Some(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_query",
                "Unknown API-key status",
            )
        }
    };
    let sort = query.sort.as_deref().unwrap_or("created");
    if !matches!(sort, "created" | "name" | "owner" | "used") {
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
    let (page, page_size) = match dto::page_parameters(query.page, query.page_size, 25) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    match api_keys::list_page(
        &services.storage,
        &api_keys::ApiKeyListQuery {
            user_id: None,
            include_privileged: value.is_owner(),
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
        Ok(keys) => HttpResponse::Ok().json(contract::ApiKeyPage {
            items: keys
                .items
                .into_iter()
                .map(contract::ApiKeyResource::from)
                .collect(),
            pagination: dto::Pagination {
                page: keys.page,
                page_size: keys.page_size,
                total_items: keys.total_items,
                total_pages: dto::total_pages(keys.total_items, keys.page_size),
            },
        }),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct AdminApiKeyQuery {
    /// Search key name, owner, token prefix, and scope.
    search: Option<String>,
    /// Restrict results to `enabled` or `disabled` keys.
    status: Option<String>,
    /// Order by `created`, `name`, `owner`, or `used`.
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

#[utoipa::path(
    patch, path = "/admin/api-keys/{id}", tag = "administration",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = EnabledInput,
    responses(
        (status = 204, description = "API key state updated"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/admin/api-keys/{id}")]
pub(crate) async fn admin_update_key(
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
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    if let Err(response) = require_manageable_key(&services, &value, *id).await {
        return response;
    }
    match api_keys::set_enabled(&services.storage, *id, body.enabled).await {
        Ok(true) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "api_key.access_changed",
                "api_key",
                Some(id.to_string()),
                None,
                serde_json::json!({"enabled":body.enabled}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    delete, path = "/admin/api-keys/{id}", tag = "administration",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/api-keys/{id}")]
pub(crate) async fn admin_delete_key(
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
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    if let Err(response) = require_manageable_key(&services, &value, *id).await {
        return response;
    }
    match api_keys::delete(&services.storage, *id).await {
        Ok(true) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "api_key.deleted",
                "api_key",
                Some(id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}
