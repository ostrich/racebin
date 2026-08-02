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

#[utoipa::path(get, path = "/admin/users", tag = "administration")]
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
        Ok(users) => HttpResponse::Ok().json(users),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(get, path = "/admin/users/{id}", tag = "administration", params(("id" = i64, Path)))]
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
        Ok(Some(user)) => HttpResponse::Ok().json(user),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(get, path = "/admin/pastes", tag = "administration")]
#[get("/admin/pastes")]
pub(crate) async fn admin_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<PasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "paste:manage") {
        return response;
    }
    if let Err(error) = crate::services::validate_paste_query(&query) {
        return domain_error(error);
    }
    match services.list_pastes(&value, &query, true).await {
        Ok(page) => HttpResponse::Ok().json(page),
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

#[utoipa::path(patch, path = "/admin/users/{id}", tag = "administration", params(("id" = i64, Path)))]
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

#[utoipa::path(post, path = "/admin/users/{id}/password-reset", tag = "administration", params(("id" = i64, Path)))]
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
        Ok(token) => HttpResponse::Created().json(json!({
            "url": super::dto::absolute(&req, &format!("/password-reset/{token}"))
        })),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(delete, path = "/admin/users/{id}/sessions", tag = "administration", params(("id" = i64, Path)))]
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

#[utoipa::path(delete, path = "/admin/users/{id}/api-keys", tag = "administration", params(("id" = i64, Path)))]
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

#[utoipa::path(get, path = "/admin/invitations", tag = "administration")]
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
                    json!({
                        "id":i.id,
                        "token_prefix":i.token_prefix,
                        "expires_at":i.expires_at,
                        "status":status,
                        "url":url,
                        "redeemed_by_username":i.redeemed_by_username
                    })
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(post, path = "/admin/invitations", tag = "administration")]
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
        Ok(token) => HttpResponse::Created().json(json!({
            "token": token,
            "url": super::dto::absolute(&req, &format!("/invitations/{token}"))
        })),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(delete, path = "/admin/invitations/{id}", tag = "administration", params(("id" = i64, Path)))]
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

#[utoipa::path(get, path = "/admin/api-keys", tag = "administration")]
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
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(patch, path = "/admin/api-keys/{id}", tag = "administration", params(("id" = i64, Path)))]
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

#[utoipa::path(delete, path = "/admin/api-keys/{id}", tag = "administration", params(("id" = i64, Path)))]
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
