use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_keys)
        .service(create_key)
        .service(update_key)
        .service(delete_key);
}

#[get("/account/api-keys")]
async fn list_keys(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    if matches!(&value, Principal::Key(key) if !key.has_scope("key:admin")) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing key:admin permission",
        );
    }
    match api_keys::list_for_user(&services.repo, user_id).await {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyInput {
    name: String,
    scopes: Vec<String>,
}

#[post("/account/api-keys")]
async fn create_key(
    req: HttpRequest,
    services: web::Data<Services>,
    body: web::Json<KeyInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    if matches!(&value, Principal::Key(key) if !key.has_scope("key:admin")) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing key:admin permission",
        );
    }
    if let Principal::Key(key) = &value {
        if !key.has_scope("key:admin") || body.scopes.iter().any(|scope| !key.has_scope(scope)) {
            return error(
                StatusCode::FORBIDDEN,
                "forbidden",
                "A key can only grant scopes it holds",
            );
        }
    } else if !value.is_admin() && body.scopes.iter().any(|scope| scope.ends_with(":admin")) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only administrators can grant administrative scopes",
        );
    }
    match api_keys::create(&services.repo, Some(user_id), &body.name, &body.scopes).await {
        Ok((key, token)) => HttpResponse::Created().json(json!({"key": key, "token": token})),
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_api_key", e),
    }
}

#[patch("/account/api-keys/{id}")]
async fn update_key(
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
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    if matches!(&value, Principal::Key(key) if !key.has_scope("key:admin")) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing key:admin permission",
        );
    }
    match api_keys::set_enabled_for_user(&services.repo, *id, user_id, body.enabled).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}

#[delete("/account/api-keys/{id}")]
async fn delete_key(
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
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    if matches!(&value, Principal::Key(key) if !key.has_scope("key:admin")) {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing key:admin permission",
        );
    }
    match api_keys::delete_for_user(&services.repo, *id, user_id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}
