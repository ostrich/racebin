use super::*;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct FolderInput {
    name: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct MovePastesInput {
    #[schema(value_type = std::collections::HashSet<String>, min_items = 1, max_items = 100)]
    ids: Vec<String>,
    #[schema(minimum = 1)]
    folder_id: Option<i64>,
}

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_folders)
        .service(create_folder)
        .service(rename_folder)
        .service(delete_folder)
        .service(move_pastes);
}

#[utoipa::path(
    get, path = "/folders", tag = "folders",
    responses(
        (status = 200, description = "The authenticated user's folders and counts", body = crate::http::contract::FolderOverviewResource),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "API key lacks paste:list", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/folders")]
pub(crate) async fn list_folders(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.list_folders(&value).await {
        Ok(folders) => HttpResponse::Ok().json(contract::FolderOverviewResource::from(folders)),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    post, path = "/folders", tag = "folders",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = FolderInput,
    responses(
        (status = 201, description = "Folder created", body = crate::http::contract::FolderResource),
        (status = 400, description = "Invalid folder name", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "API key lacks paste:write or CSRF failed", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Folder name already exists", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/folders")]
pub(crate) async fn create_folder(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<FolderInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|value| require_mutation(&services, &req, value))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.create_folder(&value, &body.name).await {
        Ok(folder) => HttpResponse::Created().json(contract::FolderResource::from(folder)),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    patch, path = "/folders/{folder_id}", tag = "folders",
    params(("folder_id" = i64, Path, description = "Folder ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = FolderInput,
    responses(
        (status = 200, description = "Folder renamed", body = crate::http::contract::FolderResource),
        (status = 400, description = "Invalid folder name", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Folder not found", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Folder name already exists", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/folders/{folder_id}")]
pub(crate) async fn rename_folder(
    req: HttpRequest,
    services: web::Data<PasteService>,
    folder_id: web::Path<i64>,
    body: web::Json<FolderInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|value| require_mutation(&services, &req, value))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.rename_folder(&value, *folder_id, &body.name).await {
        Ok(Some(folder)) => HttpResponse::Ok().json(contract::FolderResource::from(folder)),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Folder not found"),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    delete, path = "/folders/{folder_id}", tag = "folders",
    params(("folder_id" = i64, Path, description = "Folder ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "Folder deleted; its pastes become unfiled"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Folder not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/folders/{folder_id}")]
pub(crate) async fn delete_folder(
    req: HttpRequest,
    services: web::Data<PasteService>,
    folder_id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|value| require_mutation(&services, &req, value))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.delete_folder(&value, *folder_id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Folder not found"),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    patch, path = "/pastes", tag = "folders",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = MovePastesInput,
    responses(
        (status = 204, description = "Pastes moved"),
        (status = 400, description = "Invalid paste or folder selection", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Insufficient permission or CSRF failure", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste or folder not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/pastes")]
pub(crate) async fn move_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<MovePastesInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|value| require_mutation(&services, &req, value))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services
        .move_pastes(&value, &body.ids, body.folder_id)
        .await
    {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(value) => domain_error(value),
    }
}
