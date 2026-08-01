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

#[get("/admin/users")]
async fn admin_users(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    match accounts::list_admin_users(&services.storage).await {
        Ok(users) => HttpResponse::Ok().json(users),
        Err(e) => internal(e),
    }
}

#[get("/admin/users/{id}")]
async fn admin_user(
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
        Err(message) => internal(message),
    }
}

#[get("/admin/pastes")]
async fn admin_pastes(
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
    if let Err(message) = super::pastes::validate_paste_query(&query) {
        return error(StatusCode::BAD_REQUEST, "invalid_query", message);
    }
    match services.list_pastes(&value, &query, true).await {
        Ok(page) => HttpResponse::Ok().json(page),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UserUpdate {
    enabled: Option<bool>,
    role: Option<String>,
}

#[patch("/admin/users/{id}")]
async fn admin_update_user(
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
    let result = async {
        if let Some(enabled) = body.enabled {
            accounts::set_enabled(&services.storage, *id, enabled).await?;
        }
        if let Some(role) = &body.role {
            let admin = match role.as_str() {
                "admin" => true,
                "user" => false,
                _ => return Err("Role must be user or admin".to_string()),
            };
            accounts::set_role(&services.storage, *id, admin).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    match result {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_user", e),
    }
}

#[post("/admin/users/{id}/password-reset")]
async fn admin_create_password_reset(
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
        Err(message) => error(StatusCode::BAD_REQUEST, "invalid_user", message),
    }
}

#[delete("/admin/users/{id}/sessions")]
async fn admin_revoke_user_sessions(
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
        Err(message) => internal(message),
    }
}

#[delete("/admin/users/{id}/api-keys")]
async fn admin_revoke_user_keys(
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
            Err(message) => internal(message),
        },
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(message) => internal(message),
    }
}

#[get("/admin/invitations")]
async fn admin_invitations(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
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
        Err(e) => internal(e),
    }
}

#[post("/admin/invitations")]
async fn admin_create_invitation(
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
        Err(e) => internal(e),
    }
}

#[delete("/admin/invitations/{id}")]
async fn admin_revoke_invitation(
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
        Err(e) => internal(e),
    }
}

#[get("/admin/api-keys")]
async fn admin_keys(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    match api_keys::list(&services.storage).await {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => internal(e),
    }
}

#[patch("/admin/api-keys/{id}")]
async fn admin_update_key(
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
        Err(e) => internal(e),
    }
}

#[delete("/admin/api-keys/{id}")]
async fn admin_delete_key(
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
        Err(e) => internal(e),
    }
}
