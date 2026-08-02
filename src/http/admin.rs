use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(admin_users)
        .service(admin_user)
        .service(admin_update_user)
        .service(admin_create_password_reset)
        .service(admin_revoke_user_sessions)
        .service(admin_revoke_user_keys)
        .service(admin_pastes)
        .service(admin_invitations)
        .service(admin_create_invitation)
        .service(admin_revoke_invitation)
        .service(admin_keys)
        .service(admin_update_key)
        .service(admin_delete_key);
}

#[utoipa::path(
    get, path = "/admin/users", tag = "administration",
    responses(
        (status = 200, description = "Administrative user summaries", body = [crate::http::contract::AdminUserResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/users")]
pub(crate) async fn admin_users(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    match accounts::list_admin_users(&services.storage).await {
        Ok(users) => HttpResponse::Ok().json(
            users
                .into_iter()
                .map(contract::AdminUserResource::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
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
    params(super::pastes::ApiPasteQuery),
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
    let query = match query.into_inner().into_internal() {
        Ok(query) => query,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    if let Err(error) = crate::services::validate_paste_query(&query) {
        return domain_error(error);
    }
    match services.list_pastes(&value, &query, true).await {
        Ok(page) => {
            let total_pages = if page.total_items == 0 {
                0
            } else {
                (page.total_items as u64).div_ceil(u64::from(page.page_size)) as u32
            };
            HttpResponse::Ok().json(dto::PastePage {
                items: page
                    .items
                    .into_iter()
                    .map(|paste| dto::summary(&req, &value, paste, true))
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

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct UserUpdate {
    enabled: Option<bool>,
    role: Option<UserRole>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
enum UserRole {
    User,
    Admin,
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
    let admin = body
        .role
        .as_ref()
        .map(|role| matches!(role, UserRole::Admin));
    let result = accounts::update_user(&services.storage, *id, body.enabled, admin).await;
    match result {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    post, path = "/admin/users/{id}/password-reset", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 201, description = "One-time password-reset link created", body = crate::http::contract::LinkResponse),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage and user identity required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/admin/users/{id}/password-reset")]
pub(crate) async fn admin_create_password_reset(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "user:manage") {
        return response;
    }
    let Some(created_by_user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    match accounts::create_password_reset(&services.storage, *id, created_by_user_id).await {
        Ok(token) => HttpResponse::Created().json(contract::LinkResponse {
            url: super::dto::absolute(&req, &format!("/password-reset/{token}")),
        }),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    delete, path = "/admin/users/{id}/sessions", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "All user sessions revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/users/{id}/sessions")]
pub(crate) async fn admin_revoke_user_sessions(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "user:manage") {
        return response;
    }
    match accounts::revoke_sessions(&services.storage, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    delete, path = "/admin/users/{id}/api-keys", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "All user API keys revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/users/{id}/api-keys")]
pub(crate) async fn admin_revoke_user_keys(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "api_key:manage") {
        return response;
    }
    match accounts::admin_user(&services.storage, *id).await {
        Ok(Some(_)) => match api_keys::delete_all_for_user(&services.storage, *id).await {
            Ok(_) => HttpResponse::NoContent().finish(),
            Err(error) => domain_error(error),
        },
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    get, path = "/admin/invitations", tag = "administration",
    responses(
        (status = 200, description = "Invitation records", body = [crate::http::contract::InvitationResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/invitations")]
pub(crate) async fn admin_invitations(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    match accounts::list_invitations(&services.storage).await {
        Ok(items) => HttpResponse::Ok().json(
            items
                .into_iter()
                .map(|i| {
                    let status = i.status();
                    let url = if i.is_active() {
                        i.token.as_ref().map(|token| {
                            super::dto::absolute(&req, &format!("/invitations/{token}"))
                        })
                    } else {
                        None
                    };
                    contract::InvitationResource::from_invitation(i, url, status)
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    post, path = "/admin/invitations", tag = "administration",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 201, description = "Invitation created", body = crate::http::contract::InvitationCreatedResponse),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage and user identity required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/admin/invitations")]
pub(crate) async fn admin_create_invitation(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    match accounts::create_invitation(&services.storage, user_id).await {
        Ok(token) => {
            let url = super::dto::absolute(&req, &format!("/invitations/{token}"));
            HttpResponse::Created().json(contract::InvitationCreatedResponse { token, url })
        }
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    delete, path = "/admin/invitations/{id}", tag = "administration",
    params(("id" = i64, Path, description = "Invitation ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "Invitation revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Invitation not found or already redeemed", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/invitations/{id}")]
pub(crate) async fn admin_revoke_invitation(
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
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    match accounts::revoke_invitation(&services.storage, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Invitation not found"),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    get, path = "/admin/api-keys", tag = "administration",
    responses(
        (status = 200, description = "All API keys", body = [crate::http::contract::ApiKeyResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/api-keys")]
pub(crate) async fn admin_keys(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    match api_keys::list(&services.storage).await {
        Ok(v) => HttpResponse::Ok().json(
            v.into_iter()
                .map(contract::ApiKeyResource::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
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
    match api_keys::set_enabled(&services.storage, *id, body.enabled).await {
        Ok(true) => HttpResponse::NoContent().finish(),
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
    match api_keys::delete(&services.storage, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}
