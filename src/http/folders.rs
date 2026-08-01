use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FolderInput {
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MovePastesInput {
    ids: Vec<String>,
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

#[get("/folders")]
async fn list_folders(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.list_folders(&value).await {
        Ok(folders) => HttpResponse::Ok().json(folders),
        Err(message) => paste_error(message),
    }
}

#[post("/folders")]
async fn create_folder(
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
        Ok(folder) => HttpResponse::Created().json(folder),
        Err(message) => paste_error(message),
    }
}

#[patch("/folders/{folder_id}")]
async fn rename_folder(
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
        Ok(Some(folder)) => HttpResponse::Ok().json(folder),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Folder not found"),
        Err(message) => paste_error(message),
    }
}

#[delete("/folders/{folder_id}")]
async fn delete_folder(
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
        Err(message) => paste_error(message),
    }
}

#[patch("/pastes")]
async fn move_pastes(
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
        Err(message) => paste_error(message),
    }
}
