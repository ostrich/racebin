use crate::args::ARGS;
use crate::services::{PasteInput, PasteQuery, Principal, Services};
use crate::util::{accounts, api_keys};
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::http::{header, Method, StatusCode};
use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use futures::{StreamExt, TryStreamExt};
use serde::Deserialize;
use serde_json::json;
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;
use zip::write::SimpleFileOptions;

const SPA_INDEX: &[u8] = include_bytes!("../web/dist/index.html");
const SPA_SCRIPT: &[u8] = include_bytes!("../web/dist/assets/app.js");
const SPA_STYLE: &[u8] = include_bytes!("../web/dist/assets/app.css");

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

fn sanitize_upload_filename(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or("");
    let mut sanitized = String::with_capacity(basename.len().min(255));
    for character in basename.chars() {
        let character = if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
            '_'
        } else {
            character
        };
        if sanitized.len() + character.len_utf8() > 255 {
            break;
        }
        sanitized.push(character);
    }
    sanitized
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .to_string()
}

async fn principal(services: &Services, req: &HttpRequest) -> Result<Principal, HttpResponse> {
    services.principal(req).await.map_err(|message| {
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
    match principal(&services, &req).await {
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
async fn login(
    req: HttpRequest,
    services: web::Data<Services>,
    body: web::Json<LoginInput>,
) -> HttpResponse {
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
    match accounts::verify_user(&services.repo, &body.username, &body.password).await {
        Ok(Some(user)) => {
            accounts::clear_login_failures(&body.username, &client);
            match accounts::create_session(&services.repo, user.id, body.remember.unwrap_or(false)).await {
                Ok((token, csrf, _)) => HttpResponse::Ok()
                    .cookie(accounts::session_cookie(
                        token,
                        body.remember.unwrap_or(false),
                    ))
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
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
        if let Err(e) = accounts::delete_session(&services.repo, cookie.value()).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
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
    let user_id = value.user.id;
    let result =
        match accounts::verify_user(&services.repo, &value.user.username, &body.current_password)
            .await
        {
            Ok(Some(_)) => {
                accounts::set_password(&services.repo, user_id, &body.new_password, false)
                    .await
                    .map(|_| true)
            }
            Ok(None) => Ok(false),
            Err(error) => Err(error),
        };
    match result {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Current password is incorrect",
        ),
        Err(e) if e.starts_with("Password must") => {
            error(StatusCode::BAD_REQUEST, "invalid_password", e)
        }
        Err(e) => internal(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InviteInput {
    username: String,
    password: String,
}

#[post("/invites/{token}/accept")]
async fn accept_invite(
    services: web::Data<Services>,
    token: web::Path<String>,
    body: web::Json<InviteInput>,
) -> HttpResponse {
    let token = token.into_inner();
    let username = body.username.clone();
    let password = body.password.clone();
    match accounts::accept_invite(&services.repo, &token, &username, &password).await {
        Ok(user) => match accounts::create_session(&services.repo, user.id, false).await {
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
    let value = match principal(&services, &req).await {
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
    match services.list_pastes(&value, &query, false).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.create_paste(&value, &body).await {
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
    let value = match principal(&services, &req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.get_paste(&value, &slug).await {
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
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.read_paste(&value, &slug).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.update_paste(&value, &slug, &body).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.delete_paste(&value, &slug).await {
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
    let value = match principal(&services, &req).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match services.read_paste(&value, &slug).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.ensure_can_update(&value, &slug).await {
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
            .map(sanitize_upload_filename)
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
    let created = match services.add_files(&value, &slug, &inputs).await {
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
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    let paste = match services.get_paste(&value, &slug).await {
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
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    match services.delete_file(&value, &slug, file_id).await {
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
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.get_paste(&value, &slug).await {
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
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_paste(&value, &slug).await {
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

async fn asset(path: web::Path<String>) -> HttpResponse {
    let asset = match path.as_str() {
        "app.js" => Some((SPA_SCRIPT, "text/javascript; charset=utf-8")),
        "app.css" => Some((SPA_STYLE, "text/css; charset=utf-8")),
        _ => None,
    };
    match asset {
        Some((bytes, content_type)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, content_type))
            .insert_header((header::CACHE_CONTROL, "public, max-age=31536000, immutable"))
            .body(bytes),
        None => HttpResponse::NotFound().finish(),
    }
}

async fn spa(req: HttpRequest) -> HttpResponse {
    if req.method() != Method::GET || req.path().starts_with("/api/") || !spa_route(req.path()) {
        return error(StatusCode::NOT_FOUND, "not_found", "Route not found");
    }
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
        .insert_header((header::CACHE_CONTROL, "no-cache"))
        .body(SPA_INDEX)
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
    use super::{attachment_path, configure, sanitize_upload_filename};
    use crate::repository::Repository;
    use crate::services::{now, PasteInput, Principal, Services};
    use crate::util::{accounts, api_keys};
    use actix_web::{http::StatusCode, test, web, App};
    use serde_json::{json, Value};
    use std::path::Path;

    #[actix_web::test]
    async fn attachment_paths_reject_traversal_and_absolute_components() {
        let root = Path::new("/tmp/racebin-test");
        assert!(attachment_path(root, "safe-slug", "safe-name").is_ok());
        assert!(attachment_path(root, "..", "safe-name").is_err());
        assert!(attachment_path(root, "safe-slug", "../secret").is_err());
        assert!(attachment_path(root, "safe-slug", "/etc/passwd").is_err());
        assert!(attachment_path(root, "safe-slug", ".hidden").is_err());
    }

    #[actix_web::test]
    async fn upload_filenames_are_reduced_to_safe_components() {
        assert_eq!(sanitize_upload_filename("hello.txt"), "hello.txt");
        assert_eq!(sanitize_upload_filename("../hello.txt"), "hello.txt");
        assert_eq!(
            sanitize_upload_filename(r"C:\Users\someone\hello.txt"),
            "hello.txt"
        );
        assert_eq!(
            sanitize_upload_filename(" bad:<name>?.txt "),
            "bad__name__.txt"
        );
        assert_eq!(sanitize_upload_filename("..."), "");

        let long = sanitize_upload_filename(&"é".repeat(200));
        assert!(long.len() <= 255);
        assert!(long.is_char_boundary(long.len()));
    }

    #[actix_web::test]
    async fn http_auth_authorization_visibility_and_file_lifecycle() {
        let data_dir = std::env::temp_dir().join(format!("racebin-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &data_dir).await.unwrap();
        repository.migrate().await.unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES(1,'http-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("correct horse battery staple").unwrap())
        .bind(now())
        .execute(repository.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
             VALUES(2,'other-user',$1,'user',1,0,$2)",
        )
        .bind(accounts::password_hash("another correct horse battery staple").unwrap())
        .bind(now())
        .execute(repository.pool())
        .await
        .unwrap();
        let user = accounts::User {
            id: 1,
            username: "http-user".to_string(),
            role: "user".to_string(),
            enabled: true,
            force_password_change: false,
        };
        let principal = Principal::User(accounts::SessionUser {
            user,
            csrf_token: "direct".to_string(),
        });
        let services = Services::new(repository.clone());
        let input = |title: &str, access: &str| PasteInput {
            title: Some(title.to_string()),
            content: Some(format!("{title} content")),
            kind: Some("text".to_string()),
            syntax: Some("none".to_string()),
            access: Some(access.to_string()),
            expiration: None,
            burn_after_reads: None,
        };
        let public = services
            .create_paste(&principal, &input("public", "public"))
            .await
            .unwrap();
        let unlisted = services
            .create_paste(&principal, &input("unlisted", "unlisted"))
            .await
            .unwrap();
        let owner = services
            .create_paste(&principal, &input("owner", "owner"))
            .await
            .unwrap();
        let other_principal = Principal::User(accounts::SessionUser {
            user: accounts::User {
                id: 2,
                username: "other-user".to_string(),
                role: "user".to_string(),
                enabled: true,
                force_password_change: false,
            },
            csrf_token: "other".to_string(),
        });
        let other_owner = services
            .create_paste(&other_principal, &input("other owner", "owner"))
            .await
            .unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(services.clone()))
                .configure(configure),
        )
        .await;

        for (slug, expected) in [
            (&public.slug, StatusCode::OK),
            (&unlisted.slug, StatusCode::OK),
            (&owner.slug, StatusCode::NOT_FOUND),
        ] {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&format!("/api/v2/pastes/{slug}"))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), expected);
        }

        let login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/session")
                .set_json(json!({
                    "username": "http-user",
                    "password": "correct horse battery staple"
                }))
                .to_request(),
        )
        .await;
        assert_eq!(login.status(), StatusCode::OK);
        let cookie = login.response().cookies().next().unwrap().into_owned();
        let login_body: Value = test::read_body_json(login).await;
        let csrf = login_body["csrf_token"].as_str().unwrap().to_string();

        let without_csrf = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .cookie(cookie.clone())
                .set_json(json!({"title":"blocked","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(without_csrf.status(), StatusCode::FORBIDDEN);

        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"title":"files","content":"body","access":"owner"}))
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let created: Value = test::read_body_json(created).await;
        let slug = created["slug"].as_str().unwrap();

        let boundary = "racebin-test-boundary";
        let multipart = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"hello.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--{boundary}--\r\n"
        );
        let uploaded = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v2/pastes/{slug}/files"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .insert_header((
                    "Content-Type",
                    format!("multipart/form-data; boundary={boundary}"),
                ))
                .set_payload(multipart)
                .to_request(),
        )
        .await;
        assert_eq!(uploaded.status(), StatusCode::CREATED);
        let uploaded: Value = test::read_body_json(uploaded).await;
        let file_id = uploaded["items"][0]["id"].as_i64().unwrap();

        let downloaded = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(downloaded.status(), StatusCode::OK);
        assert_eq!(test::read_body(downloaded).await.as_ref(), b"hello");

        let scopes = vec!["paste:read".to_string()];
        let (_, read_token) = api_keys::create(&repository, Some(1), "read only", &scopes)
            .await
            .unwrap();
        let key_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(key_read.status(), StatusCode::OK);
        let key_write = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .set_json(json!({"title":"forbidden","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(key_write.status(), StatusCode::FORBIDDEN);

        let positive_get_scopes = [
            (
                "paste:read",
                format!("/api/v2/pastes/{}", owner.slug),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "paste:list",
                "/api/v2/pastes?mine=true".to_string(),
                owner.slug.as_str(),
                StatusCode::NOT_FOUND,
            ),
            (
                "paste:admin",
                "/api/v2/admin/pastes".to_string(),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
            (
                "user:admin",
                "/api/v2/admin/users".to_string(),
                "/api/v2/admin/invites",
                StatusCode::FORBIDDEN,
            ),
            (
                "invite:admin",
                "/api/v2/admin/invites".to_string(),
                "/api/v2/admin/api-keys",
                StatusCode::FORBIDDEN,
            ),
            (
                "key:admin",
                "/api/v2/admin/api-keys".to_string(),
                "/api/v2/admin/users",
                StatusCode::FORBIDDEN,
            ),
        ];
        for (scope, uri, denied_uri, denied_status) in positive_get_scopes {
            let (_, token) = api_keys::create(&repository, Some(1), scope, &[scope.to_string()])
                .await
                .unwrap();
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&uri)
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK, "scope {scope}");
            let denied_uri = if scope == "paste:list" {
                format!("/api/v2/pastes/{denied_uri}")
            } else {
                denied_uri.to_string()
            };
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(&denied_uri)
                    .insert_header(("Authorization", format!("Bearer {token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), denied_status, "isolated scope {scope}");
        }

        let forbidden_gets = [
            "/api/v2/pastes?mine=true",
            "/api/v2/admin/pastes",
            "/api/v2/admin/users",
            "/api/v2/admin/invites",
            "/api/v2/admin/api-keys",
        ];
        for uri in forbidden_gets {
            let response = test::call_service(
                &app,
                test::TestRequest::get()
                    .uri(uri)
                    .insert_header(("Authorization", format!("Bearer {read_token}")))
                    .to_request(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{uri}");
        }

        let (_, write_token) =
            api_keys::create(&repository, Some(1), "write", &["paste:write".to_string()])
                .await
                .unwrap();
        let write_allowed = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"written by key"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_allowed.status(), StatusCode::NO_CONTENT);
        let write_create_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/pastes")
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"key-created","content":"body"}))
                .to_request(),
        )
        .await;
        assert_eq!(write_create_allowed.status(), StatusCode::CREATED);
        let cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .set_json(json!({"title":"not allowed"}))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_write.status(), StatusCode::FORBIDDEN);
        let write_cannot_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {write_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(write_cannot_read.status(), StatusCode::NOT_FOUND);

        let disposable = services
            .create_paste(&principal, &input("delete target", "owner"))
            .await
            .unwrap();
        let read_cannot_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", disposable.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(read_cannot_delete.status(), StatusCode::FORBIDDEN);
        let (_, delete_token) = api_keys::create(
            &repository,
            Some(1),
            "delete",
            &["paste:delete".to_string()],
        )
        .await
        .unwrap();
        let delete_allowed = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", disposable.slug))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(delete_allowed.status(), StatusCode::NO_CONTENT);
        let delete_other_denied = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {delete_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(delete_other_denied.status(), StatusCode::FORBIDDEN);

        let (_, paste_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "paste admin",
            &["paste:admin".to_string()],
        )
        .await
        .unwrap();
        let cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {read_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(cross_owner_read.status(), StatusCode::NOT_FOUND);
        let admin_cross_owner_read = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_read.status(), StatusCode::OK);
        let admin_cross_owner_write = test::call_service(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .set_json(json!({"title":"administered"}))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_write.status(), StatusCode::OK);

        let (_, delegating_token) = api_keys::create(
            &repository,
            Some(1),
            "delegator",
            &["key:admin".to_string(), "paste:read".to_string()],
        )
        .await
        .unwrap();
        let delegation_allowed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated read","scopes":["paste:read"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_allowed.status(), StatusCode::CREATED);
        let delegation_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .insert_header(("Authorization", format!("Bearer {delegating_token}")))
                .set_json(json!({"name":"delegated write","scopes":["paste:write"]}))
                .to_request(),
        )
        .await;
        assert_eq!(delegation_denied.status(), StatusCode::FORBIDDEN);
        let browser_admin_scope_denied = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/api/v2/account/api-keys")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .set_json(json!({"name":"admin attempt","scopes":["user:admin"]}))
                .to_request(),
        )
        .await;
        assert_eq!(browser_admin_scope_denied.status(), StatusCode::FORBIDDEN);

        let (disabled_key, disabled_token) = api_keys::create(
            &repository,
            Some(1),
            "disabled",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        api_keys::set_enabled(&repository, disabled_key.id, false)
            .await
            .unwrap();
        let disabled_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", owner.slug))
                .insert_header(("Authorization", format!("Bearer {disabled_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(disabled_response.status(), StatusCode::UNAUTHORIZED);
        let (_, owner_disabled_token) = api_keys::create(
            &repository,
            Some(2),
            "disabled owner",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        sqlx::query("UPDATE app_user SET enabled=0 WHERE id=2")
            .execute(repository.pool())
            .await
            .unwrap();
        let disabled_owner_response = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {owner_disabled_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(disabled_owner_response.status(), StatusCode::UNAUTHORIZED);
        sqlx::query("UPDATE app_user SET enabled=1 WHERE id=2")
            .execute(repository.pool())
            .await
            .unwrap();

        let (other_key, _) = api_keys::create(
            &repository,
            Some(2),
            "other user's key",
            &["paste:read".to_string()],
        )
        .await
        .unwrap();
        let (_, key_admin_token) = api_keys::create(
            &repository,
            Some(1),
            "key boundary",
            &["key:admin".to_string()],
        )
        .await
        .unwrap();
        let account_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/account/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(account_delete_other.status(), StatusCode::NOT_FOUND);
        let admin_delete_other = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/admin/api-keys/{}", other_key.id))
                .insert_header(("Authorization", format!("Bearer {key_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_delete_other.status(), StatusCode::NO_CONTENT);
        let admin_cross_owner_delete = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{}", other_owner.slug))
                .insert_header(("Authorization", format!("Bearer {paste_admin_token}")))
                .to_request(),
        )
        .await;
        assert_eq!(admin_cross_owner_delete.status(), StatusCode::NO_CONTENT);

        let deleted = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        let missing = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v2/pastes/{slug}/files/{file_id}"))
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let logout = test::call_service(
            &app,
            test::TestRequest::delete()
                .uri("/api/v2/session")
                .cookie(cookie.clone())
                .insert_header(("X-CSRF-Token", csrf.as_str()))
                .to_request(),
        )
        .await;
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        let session = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/api/v2/session")
                .cookie(cookie)
                .to_request(),
        )
        .await;
        let session: Value = test::read_body_json(session).await;
        assert_eq!(session["authenticated"], false);

        drop(app);
        drop(repository);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
