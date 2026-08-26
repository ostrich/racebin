use super::*;

#[utoipa::path(
    post, path = "/admin/users/{id}/password-reset", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 201, description = "One-time password-reset link created", body = crate::http::contract::LinkResponse),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage and user identity required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
    if let Err(response) = require_manageable_user(&services, &value, *id).await {
        return response;
    }
    match accounts::create_password_reset(&services.storage, *id, created_by_user_id).await {
        Ok(token) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "user.password_reset_created",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::Created().json(contract::LinkResponse {
                url: contract::absolute(&req, &format!("/password-reset/{token}")),
            })
        }
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    delete, path = "/admin/users/{id}/sessions", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "All user sessions revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
    if let Err(response) = require_manageable_user(&services, &value, *id).await {
        return response;
    }
    match accounts::revoke_sessions(&services.storage, *id).await {
        Ok(true) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "user.sessions_revoked",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    delete, path = "/admin/users/{id}/api-keys", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "All user API keys revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
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
    match require_manageable_user(&services, &value, *id).await {
        Ok(_) => match api_keys::delete_all_for_user(&services.storage, *id).await {
            Ok(_) => {
                let _ = crate::instance::audit::record(
                    &services.storage,
                    &value,
                    "user.api_keys_revoked",
                    "user",
                    Some(id.to_string()),
                    None,
                    serde_json::json!({}),
                )
                .await;
                HttpResponse::NoContent().finish()
            }
            Err(error) => domain_error(error),
        },
        Err(response) => response,
    }
}
