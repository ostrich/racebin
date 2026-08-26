use super::*;

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct AdminSummaryResource {
    #[schema(minimum = 0)]
    user_count: i64,
    #[schema(minimum = 0)]
    paste_count: i64,
    #[schema(minimum = 0)]
    storage_bytes: i64,
    #[schema(minimum = 0)]
    active_session_count: i64,
    #[schema(minimum = 0)]
    active_invitation_count: i64,
    #[schema(minimum = 0)]
    expiring_invitation_count: i64,
    #[schema(minimum = 0)]
    password_change_required_count: i64,
}

#[utoipa::path(get, path="/admin/summary", tag="administration", responses((status=200,description="Administrative aggregate counts",body=AdminSummaryResource),(status=401,description="Authentication required",body=crate::http::errors::ProblemDetails),(status=403,description="Administrator required",body=crate::http::errors::ProblemDetails),(status=500,description="Internal error",body=crate::http::errors::ProblemDetails)), security(("bearerAuth"=[]),("sessionCookie"=[])))]
#[get("/admin/summary")]
pub(crate) async fn admin_summary(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "user:manage") {
        return response;
    }
    match accounts::admin_summary(&services.storage).await {
        Ok(value) => HttpResponse::Ok().json(AdminSummaryResource {
            user_count: value.user_count,
            paste_count: value.paste_count,
            storage_bytes: value.storage_bytes,
            active_session_count: value.active_session_count,
            active_invitation_count: value.active_invitation_count,
            expiring_invitation_count: value.expiring_invitation_count,
            password_change_required_count: value.password_change_required_count,
        }),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    get, path = "/admin/users", tag = "administration",
    params(AdminUserQuery),
    responses(
        (status = 200, description = "Paginated administrative user summaries", body = crate::http::contract::AdminUserPage),
        (status = 400, description = "Invalid filter, sort, or pagination parameter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/users")]
pub(crate) async fn admin_users(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<AdminUserQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    let query = query.into_inner();
    let role = query.role.as_deref();
    if !matches!(role, None | Some("user" | "admin" | "owner")) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Unknown user role",
        );
    }
    let enabled = match query.status.as_deref() {
        None | Some("all") => None,
        Some("enabled") => Some(true),
        Some("disabled") => Some(false),
        Some(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_query",
                "Unknown user status",
            )
        }
    };
    let sort = query.sort.as_deref().unwrap_or("username");
    if !matches!(
        sort,
        "username" | "created" | "login" | "pastes" | "storage"
    ) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Unknown user sort field",
        );
    }
    let descending =
        query
            .direction
            .as_deref()
            .unwrap_or(if sort == "username" { "asc" } else { "desc" })
            == "desc";
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
    match accounts::list_admin_users(
        &services.storage,
        &accounts::AdminUserListQuery {
            search: query.search.as_deref(),
            role,
            enabled,
            sort,
            descending,
            page,
            page_size,
        },
    )
    .await
    {
        Ok(users) => HttpResponse::Ok().json(contract::AdminUserPage {
            items: users
                .items
                .into_iter()
                .map(contract::AdminUserResource::from)
                .collect(),
            pagination: dto::Pagination {
                page: users.page,
                page_size: users.page_size,
                total_items: users.total_items,
                total_pages: dto::total_pages(users.total_items, users.page_size),
            },
        }),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct AdminUserQuery {
    /// Case-insensitive username search.
    search: Option<String>,
    /// Restrict results to `user`, `admin`, or `owner`.
    role: Option<String>,
    /// Restrict results to `enabled` or `disabled` accounts.
    status: Option<String>,
    /// Order by `username`, `created`, `login`, `pastes`, or `storage`.
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
    get, path = "/admin/users/{id}", tag = "administration",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "Administrative user detail", body = crate::http::contract::AdminUserResource),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/users/{id}")]
pub(crate) async fn admin_user(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "user:manage") {
        return response;
    }
    match accounts::admin_user(&services.storage, *id).await {
        Ok(Some(user)) => HttpResponse::Ok().json(contract::AdminUserResource::from(user)),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    get, path = "/admin/pastes", tag = "administration",
    params(super::pastes::ApiPasteQuery, AdminPasteOwnerQuery),
    responses(
        (status = 200, description = "Canonical paginated paste summaries including ownership", body = crate::http::dto::PastePage),
        (status = 400, description = "Invalid filter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with paste:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/pastes")]
pub(crate) async fn admin_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<super::pastes::ApiPasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "paste:manage") {
        return response;
    }
    let query = match query.into_inner().into_admin_internal() {
        Ok(query) => query,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    if let Err(error) = crate::pastes::validate_paste_query(&query) {
        return domain_error(error);
    }
    match services.list_pastes(&value, &query, true).await {
        Ok(page) => {
            let total_pages = dto::total_pages(page.total_items, page.page_size);
            let owner_names = match accounts::usernames_by_ids(
                &services.storage,
                page.items.iter().filter_map(|paste| paste.owner_id),
            )
            .await
            {
                Ok(names) => names,
                Err(error) => return domain_error(error),
            };
            HttpResponse::Ok().json(dto::PastePage {
                items: page
                    .items
                    .into_iter()
                    .map(|paste| {
                        let owner_username =
                            paste.owner_id.and_then(|id| owner_names.get(&id).cloned());
                        let mut summary = dto::summary(&req, &value, paste, true);
                        summary.owner_username = owner_username;
                        summary
                    })
                    .collect(),
                pagination: dto::Pagination {
                    page: page.page,
                    page_size: page.page_size,
                    total_items: page.total_items,
                    total_pages,
                },
            })
        }
        Err(e) => domain_error(e),
    }
}

#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)] // This DTO contributes the admin-only parameter to OpenAPI.
struct AdminPasteOwnerQuery {
    /// Restrict results to pastes owned by this positive user ID.
    #[param(minimum = 1)]
    owner_id: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct UserUpdate {
    #[serde(default, deserialize_with = "dto::optional_non_null")]
    enabled: Option<bool>,
}

#[cfg(test)]
mod user_update_tests {
    use super::UserUpdate;
    use serde_json::json;

    #[test]
    fn user_update_fields_may_not_be_null() {
        assert!(serde_json::from_value::<UserUpdate>(json!({ "enabled": null })).is_err());
    }
}

#[utoipa::path(
    patch, path = "/admin/users/{id}", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = UserUpdate,
    responses(
        (status = 204, description = "User updated"),
        (status = 400, description = "Invalid update or last-administrator invariant", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/admin/users/{id}")]
pub(crate) async fn admin_update_user(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<UserUpdate>,
) -> HttpResponse {
    if body.enabled.is_none() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_update",
            "Update must contain at least one field",
        );
    }
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    if let Err(response) = require_manageable_user(&services, &value, *id).await {
        return response;
    }
    let result = accounts::update_user(&services.storage, *id, body.enabled, None).await;
    match result {
        Ok(()) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "user.access_changed",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({"enabled":body.enabled}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct RoleUpdate {
    role: contract::UserRole,
}

#[utoipa::path(patch, path="/admin/users/{id}/role", tag="administration", params(("id"=i64,Path)), request_body=RoleUpdate, responses((status=204,description="Role updated"),(status=403,description="Owner browser session and recent authentication required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[patch("/admin/users/{id}/role")]
pub(crate) async fn admin_update_user_role(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<RoleUpdate>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value)
        .and_then(|_| auth::require_recent_authentication(&value))
    {
        return r;
    }
    let admin = match body.role {
        contract::UserRole::Admin => true,
        contract::UserRole::User => false,
        contract::UserRole::Owner => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_role",
                "Use ownership transfer to assign the owner role",
            )
        }
    };
    match accounts::set_role(&services.storage, *id, admin).await {
        Ok(()) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "user.role_changed",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({"role": if admin {"admin"} else {"user"}}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct OwnershipTransfer {
    user_id: i64,
}

#[utoipa::path(post, path="/admin/ownership-transfer", tag="administration", request_body=OwnershipTransfer, responses((status=204,description="Ownership transferred"),(status=403,description="Owner browser session and recent authentication required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[post("/admin/ownership-transfer")]
pub(crate) async fn admin_transfer_ownership(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<OwnershipTransfer>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value)
        .and_then(|_| auth::require_recent_authentication(&value))
    {
        return r;
    }
    match accounts::transfer_ownership(&services.storage, value.user_id().unwrap(), body.user_id)
        .await
    {
        Ok(()) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "ownership.transferred",
                "user",
                Some(body.user_id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}
