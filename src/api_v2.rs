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
use zip::write::SimpleFileOptions;

#[derive(RustEmbed)]
#[folder = "web/dist"]
struct Assets;

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

fn principal(services: &Services, req: &HttpRequest) -> Result<Principal, HttpResponse> {
    services.principal(req).map_err(internal)
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
      "info": {"title":"Racebin API","version":"2.0.0"},
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
          "get":{"summary":"Read a paste"},
          "patch":{"summary":"Update a paste"},
          "delete":{"summary":"Delete a paste"}
        },
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
        "max_file_size": ARGS.max_file_size_unencrypted_mb * 1024 * 1024,
        "file_uploads": !ARGS.no_file_upload,
        "qr": ARGS.qr,
        "default_expiration": if ARGS.default_expiry == "never" { serde_json::Value::Null } else { json!(ARGS.default_expiry) },
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
struct LoginInput {
    username: String,
    password: String,
    remember: Option<bool>,
}

#[post("/session")]
async fn login(req: HttpRequest, body: web::Json<LoginInput>) -> HttpResponse {
    let client = req
        .connection_info()
        .realip_remote_addr()
        .unwrap_or("unknown")
        .to_string();
    if !accounts::login_allowed(&body.username, &client) {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many login attempts",
        );
    }
    match accounts::verify_user(&body.username, &body.password) {
        Ok(Some(user)) => {
            accounts::clear_login_failures(&body.username, &client);
            match accounts::create_session(user.id, body.remember.unwrap_or(false)) {
                Ok((token, csrf, _)) => HttpResponse::Ok()
                    .cookie(accounts::session_cookie(token, body.remember.unwrap_or(false)))
                    .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
                Err(e) => internal(e),
            }
        }
        Ok(None) => {
            accounts::record_login_failure(&body.username, &client);
            error(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid username or password",
            )
        }
        Err(e) => internal(e),
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
                .secure(true)
                .same_site(SameSite::Lax)
                .max_age(actix_web::cookie::time::Duration::ZERO)
                .finish(),
        )
        .finish()
}

#[derive(Deserialize)]
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
    match accounts::verify_user(&value.user.username, &body.current_password) {
        Ok(Some(_)) => match accounts::set_password(value.user.id, &body.new_password, false) {
            Ok(()) => HttpResponse::NoContent().finish(),
            Err(e) => error(StatusCode::BAD_REQUEST, "invalid_password", e),
        },
        Ok(None) => error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Current password is incorrect",
        ),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
struct InviteInput {
    username: String,
    password: String,
}

#[post("/invites/{token}/accept")]
async fn accept_invite(token: web::Path<String>, body: web::Json<InviteInput>) -> HttpResponse {
    match accounts::accept_invite(&token, &body.username, &body.password) {
        Ok(user) => match accounts::create_session(user.id, false) {
            Ok((session, csrf, _)) => HttpResponse::Created()
                .cookie(accounts::session_cookie(session, false))
                .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
            Err(e) => internal(e),
        },
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_invitation", e),
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
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_paste", e),
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
        Ok(Some(paste)) => HttpResponse::Ok().json(paste),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => error(StatusCode::FORBIDDEN, "forbidden", e),
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
        Err(e) => error(StatusCode::FORBIDDEN, "forbidden", e),
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
    let directory = services
        .repo
        .data_dir
        .join("attachments")
        .join(slug.as_str());
    if let Err(e) = std::fs::create_dir_all(&directory) {
        return internal(e.to_string());
    }
    let limit = ARGS.max_file_size_unencrypted_mb * 1024 * 1024;
    let mut created = Vec::new();
    while let Ok(Some(mut field)) = payload.try_next().await {
        let Some(filename) = field
            .content_disposition()
            .and_then(|value| value.get_filename())
            .map(sanitize_filename::sanitize)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let temporary = directory.join(format!(".upload-{}", uuid::Uuid::new_v4()));
        let mut output = match std::fs::File::create(&temporary) {
            Ok(file) => file,
            Err(e) => return internal(e.to_string()),
        };
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
            if size > limit {
                let _ = std::fs::remove_file(&temporary);
                return error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "file_too_large",
                    "File exceeds configured size limit",
                );
            }
            if let Err(e) = output.write_all(&chunk) {
                let _ = std::fs::remove_file(&temporary);
                return internal(e.to_string());
            }
        }
        let destination = directory.join(&filename);
        if destination.exists() {
            let _ = std::fs::remove_file(&temporary);
            return error(
                StatusCode::CONFLICT,
                "file_exists",
                format!("{filename} already exists"),
            );
        }
        if let Err(e) = std::fs::rename(&temporary, &destination) {
            let _ = std::fs::remove_file(&temporary);
            return internal(e.to_string());
        }
        match services.add_file(&value, &slug, &filename, size as i64) {
            Ok(file) => created.push(file),
            Err(e) => {
                let _ = std::fs::remove_file(destination);
                return error(StatusCode::BAD_REQUEST, "invalid_file", e);
            }
        }
    }
    HttpResponse::Created().json(json!({"items": created}))
}

#[get("/pastes/{slug}/files/{file_id}")]
async fn get_file(
    req: HttpRequest,
    services: web::Data<Services>,
    path: web::Path<(String, i64)>,
) -> actix_web::Result<HttpResponse> {
    let value = match principal(&services, &req) {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let (slug, file_id) = path.into_inner();
    let paste = services
        .get_paste(&value, &slug)
        .map_err(actix_web::error::ErrorInternalServerError)?
        .ok_or_else(|| actix_web::error::ErrorNotFound("Paste not found"))?;
    let file = paste
        .files
        .iter()
        .find(|file| file.id == file_id)
        .ok_or_else(|| actix_web::error::ErrorNotFound("File not found"))?;
    let path = services
        .repo
        .data_dir
        .join("attachments")
        .join(&paste.slug)
        .join(&file.name);
    Ok(NamedFile::open(path)?.into_response(&req))
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
        Ok(Some(name)) => {
            let _ = std::fs::remove_file(
                services
                    .repo
                    .data_dir
                    .join("attachments")
                    .join(slug)
                    .join(name),
            );
            HttpResponse::NoContent().finish()
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "File not found"),
        Err(e) => error(StatusCode::FORBIDDEN, "forbidden", e),
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
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    if !paste.content.is_empty()
        && (zip.start_file("paste.txt", options).is_err()
            || zip.write_all(paste.content.as_bytes()).is_err())
    {
        return internal("Failed to create archive");
    }
    for file in &paste.files {
        let path = services
            .repo
            .data_dir
            .join("attachments")
            .join(&paste.slug)
            .join(&file.name);
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if zip.start_file(&file.name, options).is_err() || zip.write_all(&bytes).is_err() {
            return internal("Failed to create archive");
        }
    }
    match zip.finish() {
        Ok(cursor) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "application/zip"))
            .insert_header((
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.zip\"", paste.slug),
            ))
            .body(cursor.into_inner()),
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
    if let Ok(None) = services.get_paste(&value, &slug) {
        return error(StatusCode::NOT_FOUND, "not_found", "Paste not found");
    }
    let origin = req.connection_info().scheme().to_string() + "://" + req.connection_info().host();
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
    match api_keys::list_for_user(user_id) {
        Ok(v) => HttpResponse::Ok().json(v),
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
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
            accounts::set_role(*id, role == "admin")?;
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
