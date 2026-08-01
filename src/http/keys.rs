use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_keys)
        .service(create_key)
        .service(update_key)
        .service(delete_key);
}

#[utoipa::path(get, path = "/account/api-keys", tag = "api keys")]
#[get("/account/api-keys")]
pub(crate) async fn list_keys(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.list_api_keys(&value).await {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct KeyInput {
    name: String,
    scopes: Vec<String>,
}

#[utoipa::path(post, path = "/account/api-keys", tag = "api keys")]
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
        Ok((key, token)) => HttpResponse::Created().json(json!({"key": key, "token": token})),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(patch, path = "/account/api-keys/{id}", tag = "api keys", params(("id" = i64, Path)))]
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

#[utoipa::path(delete, path = "/account/api-keys/{id}", tag = "api keys", params(("id" = i64, Path)))]
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
