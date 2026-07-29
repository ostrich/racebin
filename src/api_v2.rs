use crate::args::ARGS;
use crate::services::{PasteInput, PasteQuery, Principal, Services};
use crate::util::{accounts, api_keys};
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::http::{header, Method, StatusCode};
use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use futures::{StreamExt, TryStreamExt};
use rust_embed::RustEmbed;
use serde::Deserialize;
use serde_json::json;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;
use zip::write::SimpleFileOptions;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

#[derive(Default)]
struct UploadCleanup {
    paths: Vec<PathBuf>,
}

impl Drop for UploadCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(serde::Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(serde::Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    details: Option<serde_json::Value>,
}

fn error(status: StatusCode, code: &'static str, message: impl Into<String>) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(ErrorEnvelope {
            error: ErrorBody {
                code,
                message: message.into(),
                details: None,
            },
        })
}

fn internal(message: impl Into<String>) -> HttpResponse {
    log::error!("{}", message.into());
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "Internal server error",
    )
}

fn paste_error(message: String) -> HttpResponse {
    if message.starts_with("Missing ") || message == "You do not own this paste" {
        error(StatusCode::FORBIDDEN, "forbidden", message)
    } else if [
        "Content is required",
        "Title exceeds",
        "Kind must",
        "Access must",
        "Burn count",
        "URL content",
    ]
    .iter()
    .any(|prefix| message.starts_with(prefix))
    {
        error(StatusCode::BAD_REQUEST, "invalid_paste", message)
    } else {
        internal(message)
    }
}

fn attachment_path(data_dir: &Path, slug: &str, name: &str) -> Result<PathBuf, String> {
    let safe_component = |value: &str| {
        let mut components = Path::new(value).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    if !safe_component(slug) || !safe_component(name) || name.starts_with('.') {
        return Err("Unsafe attachment metadata".to_string());
    }
    Ok(data_dir.join("attachments").join(slug).join(name))
}

fn principal(services: &Services, req: &HttpRequest) -> Result<Principal, HttpResponse> {
    services.principal(req).map_err(|message| {
        if message == "Password change required" {
            error(StatusCode::FORBIDDEN, "password_change_required", message)
        } else if message.contains("authorization") || message.contains("bearer") {
            error(StatusCode::UNAUTHORIZED, "invalid_token", message)
        } else {
            internal(message)
        }
    })
}

fn require_auth(value: Principal) -> Result<Principal, HttpResponse> {
    if matches!(value, Principal::Anonymous) {
        Err(error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication required",
        ))
    } else {
        Ok(value)
    }
}

fn require_mutation(
    services: &Services,
    req: &HttpRequest,
    value: Principal,
) -> Result<Principal, HttpResponse> {
    let value = require_auth(value)?;
    if !services.csrf_valid(req, &value) {
        return Err(error(
            StatusCode::FORBIDDEN,
            "csrf_failed",
            "Missing or invalid CSRF token",
        ));
    }
    Ok(value)
}

fn require_admin(value: &Principal, scope: &str) -> Result<(), HttpResponse> {
    if value.can(scope) {
        Ok(())
    } else {
        Err(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            format!("Missing {scope} permission"),
        ))
    }
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .app_data(
            web::JsonConfig::default()
                .limit(2 * 1024 * 1024)
                .error_handler(|parse_error, _| {
                    actix_web::error::InternalError::from_response(
                        parse_error,
                        error(
                            StatusCode::BAD_REQUEST,
                            "invalid_json",
                            "Request body is not valid for this endpoint",
                        ),
                    )
                    .into()
                }),
        )
        .app_data(web::QueryConfig::default().error_handler(|parse_error, _| {
            actix_web::error::InternalError::from_response(
                parse_error,
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_query",
                    "Query parameters are not valid for this endpoint",
                ),
            )
            .into()
        }))
        .service(
            web::scope("/api/v2")
                .service(get_config)
                .service(get_openapi)
                .service(get_session)
                .service(login)
                .service(logout)
                .service(change_password)
                .service(accept_invite)
                .service(list_pastes)
                .service(create_paste)
                .service(consume_paste)
                .service(get_paste)
                .service(update_paste)
                .service(delete_paste)
                .service(raw_paste)
                .service(upload_files)
                .service(get_file)
                .service(delete_file)
                .service(get_archive)
                .service(get_qr)
                .service(list_keys)
                .service(create_key)
                .service(update_key)
                .service(delete_key)
                .service(admin_users)
                .service(admin_update_user)
                .service(admin_pastes)
                .service(admin_invites)
                .service(admin_create_invite)
                .service(admin_revoke_invite)
                .service(admin_keys)
                .service(admin_update_key)
                .service(admin_delete_key),
        )
        .service(web::resource("/assets/{path:.*}").route(web::get().to(asset)))
        .default_service(web::route().to(spa));
}

#[get("/openapi.json")]
async fn get_openapi() -> impl Responder {
    HttpResponse::Ok().json(json!({
      "openapi": "3.1.0",
      "info": {"title":"Racebin API","version":"3.0.0"},
      "servers": [{"url":"/api/v2"}],
      "components": {
        "securitySchemes": {
          "bearerAuth": {"type":"http","scheme":"bearer"},
          "sessionCookie": {"type":"apiKey","in":"cookie","name":"racebin_session"}
        }
      },
      "paths": {
        "/config": {"get":{"summary":"Runtime configuration"}},
        "/session": {
          "get":{"summary":"Current session"},
          "post":{"summary":"Log in"},
          "delete":{"summary":"Log out"}
        },
        "/account/password":{"patch":{"summary":"Change current user's password"}},
        "/account/api-keys":{
          "get":{"summary":"List current user's API keys"},
          "post":{"summary":"Create an API key"}
        },
        "/account/api-keys/{id}":{
          "patch":{"summary":"Enable or disable an API key"},
          "delete":{"summary":"Delete an API key"}
        },
        "/invites/{token}/accept":{"post":{"summary":"Accept an invitation"}},
        "/pastes":{
          "get":{"summary":"List visible pastes"},
          "post":{"summary":"Create a paste"}
        },
        "/pastes/{slug}":{
          "get":{"summary":"Get paste metadata and content without consuming a read"},
          "patch":{"summary":"Update a paste"},
          "delete":{"summary":"Delete a paste"}
        },
        "/pastes/{slug}/consume":{"get":{"summary":"Read and consume a paste"}},
        "/pastes/{slug}/raw":{"get":{"summary":"Read raw paste content"}},
        "/pastes/{slug}/files":{"post":{"summary":"Upload paste files"}},
        "/pastes/{slug}/files/{file_id}":{
          "get":{"summary":"Download a paste file"},
          "delete":{"summary":"Delete a paste file"}
        },
        "/pastes/{slug}/archive":{"get":{"summary":"Download paste and files as ZIP"}},
        "/pastes/{slug}/qr":{"get":{"summary":"Generate a paste QR code"}},
        "/admin/pastes":{"get":{"summary":"List all pastes"}},
        "/admin/users":{
          "get":{"summary":"List users"}
        },
        "/admin/users/{id}":{"patch":{"summary":"Update a user"}},
        "/admin/invites":{
          "get":{"summary":"List invitations"},
          "post":{"summary":"Create an invitation"}
        },
        "/admin/invites/{id}":{"delete":{"summary":"Revoke an invitation"}},
        "/admin/api-keys":{"get":{"summary":"List all API keys"}},
        "/admin/api-keys/{id}":{
          "patch":{"summary":"Enable or disable any API key"},
          "delete":{"summary":"Delete any API key"}
        }
      }
    }))
}

#[get("/config")]
async fn get_config() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "name": ARGS.title.as_deref().unwrap_or("Racebin"),
        "max_file_size": ARGS.max_file_size_mb * 1024 * 1024,
        "file_uploads": !ARGS.no_file_upload,
        "qr": ARGS.qr,
        "access_modes": ["public", "unlisted", "owner"]
    }))
}

#[get("/session")]
async fn get_session(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    match principal(&services, &req) {
        Ok(Principal::User(session)) => HttpResponse::Ok().json(json!({
            "authenticated": true,
            "user": {
                "id": session.user.id, "username": session.user.username,
                "role": session.user.role, "force_password_change": session.user.force_password_change
            },
            "csrf_token": session.csrf_token
        })),
        Ok(Principal::Key(key)) => HttpResponse::Ok().json(json!({
            "authenticated": true, "api_key": {"id": key.id, "name": key.name, "scopes": key.scopes}
        })),
        Ok(Principal::Anonymous) => HttpResponse::Ok().json(json!({"authenticated": false})),
        Err(response) => response,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginInput {
    username: String,
    password: String,
    remember: Option<bool>,
}

#[post("/session")]
async fn login(req: HttpRequest, body: web::Json<LoginInput>) -> HttpResponse {
    let client = req
        .peer_addr()
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    if !accounts::login_allowed(&body.username, &client) {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many login attempts",
        );
    }
    let username = body.username.clone();
    let password = body.password.clone();
    let verified = web::block(move || accounts::verify_user(&username, &password)).await;
    match verified {
        Ok(Ok(Some(user))) => {
            accounts::clear_login_failures(&body.username, &client);
            match accounts::create_session(user.id, body.remember.unwrap_or(false)) {
                Ok((token, csrf, _)) => HttpResponse::Ok()
                    .cookie(accounts::session_cookie(
                        token,
                        body.remember.unwrap_or(false),
                    ))
                    .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
                Err(e) => internal(e),
            }
        }
        Ok(Ok(None)) => {
            accounts::record_login_failure(&body.username, &client);
            error(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid username or password",
            )
        }
        Ok(Err(e)) => internal(e),
        Err(e) => internal(e.to_string()),
    }
}

#[delete("/session")]
async fn logout(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !matches!(value, Principal::User(_)) {
        return error(
            StatusCode::BAD_REQUEST,
            "session_required",
            "A browser session is required",
        );
    }
    if let Some(cookie) = req.cookie(accounts::SESSION_COOKIE) {
        if let Err(e) = accounts::delete_session(cookie.value()) {
            return internal(e);
        }
    }
    HttpResponse::NoContent()
        .cookie(
            Cookie::build(accounts::SESSION_COOKIE, "")
                .path("/")
                .http_only(true)
                .secure(!ARGS.insecure_cookie)
                .same_site(SameSite::Lax)
                .max_age(actix_web::cookie::time::Duration::ZERO)
                .finish(),
        )
        .finish()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PasswordInput {
    current_password: String,
    new_password: String,
}

#[patch("/account/password")]
async fn change_password(
    req: HttpRequest,
    services: web::Data<Services>,
    body: web::Json<PasswordInput>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(Principal::User(session)) => session,
        Ok(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "session_required",
                "A browser session is required",
            )
        }
        Err(response) => return response,
    };
    let username = value.user.username;
    let current_password = body.current_password.clone();
    let new_password = body.new_password.clone();
    let user_id = value.user.id;
    let result = web::block(move || {
        if accounts::verify_user(&username, &current_password)?.is_none() {
            return Ok::<_, String>(false);
        }
        accounts::set_password(user_id, &new_password, false)?;
        Ok(true)
    })
    .await;
    match result {
        Ok(Ok(true)) => HttpResponse::NoContent().finish(),
        Ok(Ok(false)) => error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Current password is incorrect",
        ),
        Ok(Err(e)) if e.starts_with("Password must") => {
            error(StatusCode::BAD_REQUEST, "invalid_password", e)
        }
        Ok(Err(e)) => internal(e),
        Err(e) => internal(e.to_string()),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InviteInput {
    username: String,
    password: String,
}

#[post("/invites/{token}/accept")]
async fn accept_invite(token: web::Path<String>, body: web::Json<InviteInput>) -> HttpResponse {
    let token = token.into_inner();
    let username = body.username.clone();
    let password = body.password.clone();
    match web::block(move || accounts::accept_invite(&token, &username, &password)).await {
        Ok(Ok(user)) => match accounts::create_session(user.id, false) {
            Ok((session, csrf, _)) => HttpResponse::Created()
                .cookie(accounts::session_cookie(session, false))
                .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
            Err(e) => internal(e),
        },
        Ok(Err(e)) => error(StatusCode::BAD_REQUEST, "invalid_invitation", e),
        Err(e) => internal(e.to_string()),
    }
}

#[get("/pastes")]
async fn list_pastes(
    req: HttpRequest,
    services: web::Data<Services>,
    query: web::Query<PasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if query.mine.unwrap_or(false) && value.user_id().is_none() {
        return error(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "Authentication required for mine=true",
        );
    }
    if query
        .access
        .as_deref()
        .is_some_and(|access| !matches!(access, "public" | "unlisted" | "owner"))
    {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Access must be public, unlisted, or owner",
        );
    }
    let wants_private = query.owner_user_id.is_some() || query.access.as_deref() != Some("public");
    if matches!(value, Principal::Key(_)) && wants_private && !value.can("paste:list") {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Missing paste:list permission",
        );
    }
    match services.list_pastes(&value, &query, false) {
        Ok(page) => HttpResponse::Ok().json(page),
        Err(e) => internal(e),
    }
}

#[post("/pastes")]
async fn create_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    body: web::Json<PasteInput>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.create_paste(&value, &body) {
        Ok(paste) => HttpResponse::Created().json(paste),
        Err(e) => paste_error(e),
    }
}

#[get("/pastes/{slug}")]
async fn get_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.get_paste(&value, &slug) {
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}

#[get("/pastes/{slug}/consume")]
async fn consume_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.read_paste(&value, &slug) {
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}

#[patch("/pastes/{slug}")]
async fn update_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
    body: web::Json<PasteInput>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.update_paste(&value, &slug, &body) {
        Ok(Some(_)) if matches!(&value, Principal::Key(key) if !key.has_scope("paste:read") && !key.has_scope("paste:admin")) => {
            HttpResponse::NoContent().finish()
        }
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => paste_error(e),
    }
}

#[delete("/pastes/{slug}")]
async fn delete_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.delete_paste(&value, &slug) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) if e == "You do not own this paste" || e.starts_with("Missing ") => {
            error(StatusCode::FORBIDDEN, "forbidden", e)
        }
        Err(e) => internal(e),
    }
}

#[get("/pastes/{slug}/raw")]
async fn raw_paste(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.read_paste(&value, &slug) {
        Ok(Some(paste)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "text/plain; charset=utf-8"))
            .body(paste.content),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => internal(e),
    }
}

#[post("/pastes/{slug}/files")]
async fn upload_files(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
    mut payload: Multipart,
) -> HttpResponse {
    if ARGS.no_file_upload {
        return error(
            StatusCode::FORBIDDEN,
            "uploads_disabled",
            "File uploads are disabled",
        );
    }
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.ensure_can_update(&value, &slug) {
        Ok(paste) => paste,
        Err(e) => return error(StatusCode::NOT_FOUND, "not_found", e),
    };
    let directory = services.repo.data_dir.join("attachments").join(&paste.slug);
    if let Err(e) = tokio::fs::create_dir_all(&directory).await {
        return internal(e.to_string());
    }
    let Some(limit) = ARGS.max_file_size_mb.checked_mul(1024 * 1024) else {
        return internal("Configured upload size is too large");
    };
    let mut staged: Vec<(PathBuf, PathBuf, String, String, i64)> = Vec::new();
    let mut cleanup = UploadCleanup::default();
    let mut total_size = 0usize;
    loop {
        let mut field = match payload.try_next().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                return error(StatusCode::BAD_REQUEST, "invalid_upload", e.to_string());
            }
        };
        if staged.len() >= 32 {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_many_files",
                "A paste may contain at most 32 files",
            );
        }
        let Some(filename) = field
            .content_disposition()
            .and_then(|value| value.get_filename())
            .map(sanitize_filename::sanitize)
            .filter(|value| !value.is_empty())
        else {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_upload",
                "Every multipart field must contain a filename",
            );
        };
        let temporary = directory.join(format!(".upload-{}", uuid::Uuid::new_v4()));
        let mut output = match tokio::fs::File::create(&temporary).await {
            Ok(file) => file,
            Err(e) => return internal(e.to_string()),
        };
        cleanup.paths.push(temporary.clone());
        let mut size = 0usize;
        while let Some(chunk) = field.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => {
                    let _ = std::fs::remove_file(&temporary);
                    return error(StatusCode::BAD_REQUEST, "invalid_upload", e.to_string());
                }
            };
            size += chunk.len();
            total_size = total_size.saturating_add(chunk.len());
            if size > limit || total_size > limit {
                let _ = std::fs::remove_file(&temporary);
                return error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "file_too_large",
                    "Upload exceeds configured size limit",
                );
            }
            if let Err(e) = output.write_all(&chunk).await {
                let _ = std::fs::remove_file(&temporary);
                return internal(e.to_string());
            }
        }
        let storage_name = uuid::Uuid::new_v4().simple().to_string();
        let destination = match attachment_path(&services.repo.data_dir, &paste.slug, &storage_name)
        {
            Ok(path) => path,
            Err(e) => {
                let _ = std::fs::remove_file(&temporary);
                return error(StatusCode::BAD_REQUEST, "invalid_file", e);
            }
        };
        if tokio::fs::try_exists(&destination).await.unwrap_or(true) {
            let _ = std::fs::remove_file(&temporary);
            return error(
                StatusCode::CONFLICT,
                "file_exists",
                format!("{filename} already exists"),
            );
        }
        staged.push((temporary, destination, filename, storage_name, size as i64));
    }
    let mut promoted = Vec::new();
    for (temporary, destination, _, _, _) in &staged {
        if let Err(e) = tokio::fs::rename(temporary, destination).await {
            let _ = std::fs::remove_file(temporary);
            for path in promoted {
                let _ = std::fs::remove_file(path);
            }
            for (path, _, _, _, _) in &staged {
                let _ = std::fs::remove_file(path);
            }
            return internal(e.to_string());
        }
        cleanup.paths.push(destination.clone());
        promoted.push(destination.clone());
    }
    let inputs = staged
        .iter()
        .map(|(_, _, name, storage_name, size)| (name.clone(), storage_name.clone(), *size))
        .collect::<Vec<_>>();
    let created = match services.add_files(&value, &slug, &inputs) {
        Ok(files) => files,
        Err(e) => {
            for path in promoted {
                let _ = std::fs::remove_file(path);
            }
            return error(StatusCode::BAD_REQUEST, "invalid_file", e);
        }
    };
    if created.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_upload",
            "No valid files were supplied",
        );
    }
    cleanup.paths.clear();
    HttpResponse::Created().json(json!({"items": created}))
}

#[get("/pastes/{slug}/files/{file_id}")]
async fn get_file(
    req: HttpRequest,
    services: web::Data<Services>,
    path: web::Path<(String, i64)>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    let paste = match services.get_paste(&value, &slug) {
        Ok(Some(paste)) => paste,
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    };
    let Some(file) = paste.files.iter().find(|file| file.id == file_id) else {
        return error(StatusCode::NOT_FOUND, "not_found", "File not found");
    };
    let path = match attachment_path(&services.repo.data_dir, &paste.slug, &file.storage_name) {
        Ok(path) => path,
        Err(e) => return internal(e),
    };
    let content_type = mime_guess::from_path(&file.name).first_or_octet_stream();
    let named = match NamedFile::open(path) {
        Ok(named) => named,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return error(StatusCode::NOT_FOUND, "not_found", "File data is missing")
        }
        Err(e) => return internal(e.to_string()),
    };
    named
        .set_content_type(content_type)
        .set_content_disposition(header::ContentDisposition {
            disposition: header::DispositionType::Attachment,
            parameters: vec![header::DispositionParam::Filename(file.name.clone())],
        })
        .into_response(&req)
}

#[delete("/pastes/{slug}/files/{file_id}")]
async fn delete_file(
    req: HttpRequest,
    services: web::Data<Services>,
    path: web::Path<(String, i64)>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    match services.delete_file(&value, &slug, file_id) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "File not found"),
        Err(e) if e == "You do not own this paste" || e.starts_with("Missing ") => {
            error(StatusCode::FORBIDDEN, "forbidden", e)
        }
        Err(e) => internal(e),
    }
}

#[get("/pastes/{slug}/archive")]
async fn get_archive(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.get_paste(&value, &slug) {
        Ok(Some(paste)) => paste,
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    };
    const MAX_ARCHIVE_INPUT: u64 = 64 * 1024 * 1024;
    let archive_input = paste.content.len() as u64
        + paste
            .files
            .iter()
            .map(|file| file.size.max(0) as u64)
            .sum::<u64>();
    if archive_input > MAX_ARCHIVE_INPUT {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "archive_too_large",
            "Archive input exceeds 64 MiB",
        );
    }
    let data_dir = services.repo.data_dir.clone();
    let archive_slug = paste.slug.clone();
    let archive = web::block(move || {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if !paste.content.is_empty() {
            zip.start_file("paste.txt", options)
                .map_err(|e| e.to_string())?;
            zip.write_all(paste.content.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        for file in &paste.files {
            let path = attachment_path(&data_dir, &paste.slug, &file.storage_name)?;
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            zip.start_file(&file.name, options)
                .map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.finish()
            .map(|cursor| cursor.into_inner())
            .map_err(|e| e.to_string())
    })
    .await;
    match archive {
        Ok(Ok(bytes)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "application/zip"))
            .insert_header((
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{archive_slug}.zip\""),
            ))
            .body(bytes),
        Ok(Err(e)) => internal(e),
        Err(e) => internal(e.to_string()),
    }
}

#[get("/pastes/{slug}/qr")]
async fn get_qr(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    if !ARGS.qr {
        return error(StatusCode::NOT_FOUND, "not_found", "QR codes are disabled");
    }
    let value = match principal(&services, &req) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_paste(&value, &slug) {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    }
    let origin = ARGS
        .public_url
        .as_ref()
        .expect("validated at startup")
        .as_str()
        .trim_end_matches('/');
    match qrcode_generator::to_png_to_vec(
        format!("{origin}/pastes/{slug}"),
        qrcode_generator::QrCodeEcc::Low,
        512,
    ) {
        Ok(bytes) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "image/png"))
            .body(bytes),
        Err(e) => internal(e.to_string()),
    }
}

#[get("/account/api-keys")]
async fn list_keys(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(require_auth) {
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
    match api_keys::list_for_user(user_id) {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
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
    match api_keys::create(Some(user_id), &body.name, &body.scopes) {
        Ok((key, token)) => HttpResponse::Created().json(json!({"key": key, "token": token})),
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_api_key", e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnabledInput {
    enabled: bool,
}

#[patch("/account/api-keys/{id}")]
async fn update_key(
    req: HttpRequest,
    services: web::Data<Services>,
    id: web::Path<i64>,
    body: web::Json<EnabledInput>,
) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
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
    match api_keys::set_enabled_for_user(*id, user_id, body.enabled) {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
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
    match api_keys::delete_for_user(*id, user_id) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}

#[get("/admin/users")]
async fn admin_users(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:admin") {
        return r;
    }
    match accounts::list_users() {
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
    let value = match principal(&services, &req).and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "paste:admin") {
        return response;
    }
    match services.list_pastes(&value, &query, true) {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:admin") {
        return r;
    }
    let result = (|| {
        if let Some(enabled) = body.enabled {
            accounts::set_enabled(*id, enabled)?;
        }
        if let Some(role) = &body.role {
            let admin = match role.as_str() {
                "admin" => true,
                "user" => false,
                _ => return Err("Role must be user or admin".to_string()),
            };
            accounts::set_role(*id, admin)?;
        }
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_user", e),
    }
}

#[get("/admin/invites")]
async fn admin_invites(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invite:admin") {
        return r;
    }
    match accounts::list_invites() {
        Ok(items) => HttpResponse::Ok().json(items.into_iter().map(|i| json!({"id":i.id,"token_prefix":i.token_prefix,"expires":i.expires,"status":i.status()})).collect::<Vec<_>>()),
        Err(e) => internal(e),
    }
}

#[post("/admin/invites")]
async fn admin_create_invite(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
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
    match accounts::create_invite(user_id) {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invite:admin") {
        return r;
    }
    match accounts::revoke_invite(*id) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Invitation not found"),
        Err(e) => internal(e),
    }
}

#[get("/admin/api-keys")]
async fn admin_keys(req: HttpRequest, services: web::Data<Services>) -> HttpResponse {
    let value = match principal(&services, &req).and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::list() {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::set_enabled(*id, body.enabled) {
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
    let value = match principal(&services, &req).and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "key:admin") {
        return r;
    }
    match api_keys::delete(*id) {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => internal(e),
    }
}

async fn asset(path: web::Path<String>) -> HttpResponse {
    let embedded_path = format!("assets/{}", path.as_str());
    match Assets::get(&embedded_path) {
        Some(asset) => {
            let content_type = mime_guess::from_path(path.as_str()).first_or_octet_stream();
            HttpResponse::Ok()
                .insert_header((header::CONTENT_TYPE, content_type.as_ref()))
                .insert_header((header::CACHE_CONTROL, "public, max-age=31536000, immutable"))
                .body(asset.data.into_owned())
        }
        None => HttpResponse::NotFound().finish(),
    }
}

async fn spa(req: HttpRequest) -> HttpResponse {
    if req.method() != Method::GET || req.path().starts_with("/api/") || !spa_route(req.path()) {
        return error(StatusCode::NOT_FOUND, "not_found", "Route not found");
    }
    match Assets::get("index.html") {
        Some(asset) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
            .insert_header((header::CACHE_CONTROL, "no-cache"))
            .body(asset.data.into_owned()),
        None => internal("SPA index is missing"),
    }
}

fn spa_route(path: &str) -> bool {
    path == "/"
        || matches!(
            path,
            "/explore"
                | "/login"
                | "/new"
                | "/pastes"
                | "/account"
                | "/account/password"
                | "/admin"
                | "/admin/pastes"
                | "/guide"
        )
        || path
            .strip_prefix("/invite/")
            .is_some_and(|v| !v.is_empty() && !v.contains('/'))
        || path.strip_prefix("/pastes/").is_some_and(|v| {
            let pieces: Vec<_> = v.split('/').collect();
            pieces.len() == 1 || (pieces.len() == 2 && pieces[1] == "edit")
        })
}

#[cfg(test)]
mod tests {
    use super::attachment_path;
    use std::path::Path;

    #[test]
    fn attachment_paths_reject_traversal_and_absolute_components() {
        let root = Path::new("/tmp/racebin-test");
        assert!(attachment_path(root, "safe-slug", "safe-name").is_ok());
        assert!(attachment_path(root, "..", "safe-name").is_err());
        assert!(attachment_path(root, "safe-slug", "../secret").is_err());
        assert!(attachment_path(root, "safe-slug", "/etc/passwd").is_err());
        assert!(attachment_path(root, "safe-slug", ".hidden").is_err());
    }
}
