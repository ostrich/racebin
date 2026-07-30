use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(admin_users)
        .service(admin_update_user)
        .service(admin_pastes)
        .service(admin_invites)
        .service(admin_create_invite)
        .service(admin_revoke_invite)
        .service(admin_keys)
        .service(admin_update_key)
        .service(admin_delete_key);
}

#[get("/admin/users")]
async fn admin_users(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:admin") {
        return r;
    }
    match accounts::list_users(&services.repo).await {
        Ok(users) => HttpResponse::Ok().json(users.into_iter().map(|u| json!({"id":u.id,"username":u.username,"role":u.role,"enabled":u.enabled,"force_password_change":u.force_password_change})).collect::<Vec<_>>()),
        Err(e) => internal(e),
    }
}

#[get("/admin/pastes")]
async fn admin_pastes(
    req: HttpRequest,
    services: web::Data<Services>,
    query: web::Query<PasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "paste:admin") {
        return response;
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
    services: web::Data<Services>,
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
    if let Err(r) = require_admin(&value, "user:admin") {
        return r;
    }
    let result = async {
        if let Some(enabled) = body.enabled {
            accounts::set_enabled(&services.repo, *id, enabled).await?;
        }
        if let Some(role) = &body.role {
            let admin = match role.as_str() {
                "admin" => true,
                "user" => false,
                _ => return Err("Role must be user or admin".to_string()),
            };
            accounts::set_role(&services.repo, *id, admin).await?;
        }
        Ok::<_, String>(())
    }
    .await;
    match result {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_user", e),
    }
}

#[get("/admin/invites")]
async fn admin_invites(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invite:admin") {
        return r;
    }
    match accounts::list_invites(&services.repo).await {
        Ok(items) => HttpResponse::Ok().json(items.into_iter().map(|i| json!({"id":i.id,"token_prefix":i.token_prefix,"expires":i.expires,"status":i.status()})).collect::<Vec<_>>()),
        Err(e) => internal(e),
    }
}

#[post("/admin/invites")]
async fn admin_create_invite(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invite:admin") {
        return r;
    }
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    match accounts::create_invite(&services.repo, user_id).await {
        Ok(token) => {
            HttpResponse::Created().json(json!({"token":token,"url":format!("/invite/{token}")}))
        }
        Err(e) => internal(e),
    }
}

#[delete("/admin/invites/{id}")]
async fn admin_revoke_invite(
    req: HttpRequest,
    services: web::Data<Services>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invite:admin") {
        return r;
    }
    match accounts::revoke_invite(&services.repo, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Invitation not found"),
        Err(e) => internal(e),
    }
}

#[get("/admin/api-keys")]
async fn admin_keys(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::list(&services.repo).await {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => internal(e),
    }
}

#[patch("/admin/api-keys/{id}")]
async fn admin_update_key(
    req: HttpRequest,
    services: web::Data<Services>,
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
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::set_enabled(&services.repo, *id, body.enabled).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}

#[delete("/admin/api-keys/{id}")]
async fn admin_delete_key(
    req: HttpRequest,
    services: web::Data<Services>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::delete(&services.repo, *id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}
