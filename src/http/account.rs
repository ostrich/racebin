use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(get_session)
        .service(login)
        .service(logout)
        .service(change_password)
        .service(reset_password)
        .service(redeem_invitation);
}

#[utoipa::path(get, path = "/session", tag = "account")]
#[get("/session")]
pub(crate) async fn get_session(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    match principal(&services, &req).await {
        Ok(Principal::Session(session)) => HttpResponse::Ok().json(json!({
            "authenticated": true,
            "user": {
                "id": session.user.id, "username": session.user.username,
                "role": session.user.role, "password_change_required": session.user.password_change_required
            },
            "csrf_token": session.csrf_token
        })),
        Ok(Principal::ApiKey(key)) => HttpResponse::Ok().json(json!({
            "authenticated": true, "api_key": {"id": key.id, "name": key.name, "scopes": key.scopes}
        })),
        Ok(Principal::Anonymous) => HttpResponse::Ok().json(json!({"authenticated": false})),
        Err(response) => response,
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct LoginInput {
    username: String,
    password: String,
    remember: Option<bool>,
}

#[utoipa::path(post, path = "/session", tag = "account")]
#[post("/session")]
pub(crate) async fn login(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<LoginInput>,
) -> HttpResponse {
    let client = auth::client_address(&req);
    let retry_after =
        match accounts::login_retry_after(&services.storage, &body.username, &client).await {
            Ok(value) => value,
            Err(error) => return internal(error),
        };
    if let Some(retry_after) = retry_after {
        let mut response = error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many login attempts",
        );
        response.headers_mut().insert(
            header::RETRY_AFTER,
            header::HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );
        return response;
    }
    match accounts::verify_user(&services.storage, &body.username, &body.password).await {
        Ok(Some(user)) => {
            if let Err(error) =
                accounts::clear_login_failures(&services.storage, &body.username).await
            {
                return internal(error);
            }
            match accounts::create_session(&services.storage, user.id, body.remember.unwrap_or(false)).await {
                Ok((token, csrf, _)) => HttpResponse::Ok()
                    .cookie(cookies::session_cookie(
                        token,
                        body.remember.unwrap_or(false),
                    ))
                    .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
                Err(e) => internal(e),
            }
        }
        Ok(None) => {
            if let Err(error) =
                accounts::record_login_failure(&services.storage, &body.username, &client).await
            {
                return internal(error);
            }
            error(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid username or password",
            )
        }
        Err(e) => internal(e),
    }
}

#[utoipa::path(delete, path = "/session", tag = "account")]
#[delete("/session")]
pub(crate) async fn logout(req: HttpRequest, services: web::Data<PasteService>) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !matches!(value, Principal::Session(_)) {
        return error(
            StatusCode::BAD_REQUEST,
            "session_required",
            "A browser session is required",
        );
    }
    if let Some(cookie) = req.cookie(accounts::SESSION_COOKIE) {
        if let Err(e) = accounts::delete_session(&services.storage, cookie.value()).await {
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

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PasswordInput {
    current_password: String,
    new_password: String,
}

#[utoipa::path(patch, path = "/account/password", tag = "account")]
#[patch("/account/password")]
pub(crate) async fn change_password(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<PasswordInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(Principal::Session(session)) => session,
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
    let result = match accounts::verify_user(
        &services.storage,
        &value.user.username,
        &body.current_password,
    )
    .await
    {
        Ok(Some(_)) => {
            accounts::set_password(&services.storage, user_id, &body.new_password, false)
                .await
                .map(|_| true)
        }
        Ok(None) => Ok(false),
        Err(error) => Err(crate::services::DomainError::internal(error)),
    };
    match result {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "Current password is incorrect",
        ),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct InvitationInput {
    username: String,
    password: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct PasswordResetInput {
    new_password: String,
}

#[utoipa::path(post, path = "/password-resets/{token}", tag = "account", params(("token" = String, Path)))]
#[post("/password-resets/{token}")]
pub(crate) async fn reset_password(
    req: HttpRequest,
    services: web::Data<PasteService>,
    token: web::Path<String>,
    body: web::Json<PasswordResetInput>,
) -> HttpResponse {
    let client = auth::client_address(&req);
    let retry_after = match accounts::password_reset_retry_after(&services.storage, &client).await {
        Ok(value) => value,
        Err(error) => return internal(error),
    };
    if let Some(retry_after) = retry_after {
        let mut response = error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many password reset attempts",
        );
        response.headers_mut().insert(
            header::RETRY_AFTER,
            header::HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );
        return response;
    }
    match accounts::reset_password(&services.storage, &token, &body.new_password).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(value) => {
            if let Err(error) =
                accounts::record_password_reset_failure(&services.storage, &client).await
            {
                return internal(error);
            }
            domain_error(value)
        }
    }
}

#[utoipa::path(post, path = "/invitations/{token}/redeem", tag = "account", params(("token" = String, Path)))]
#[post("/invitations/{token}/redeem")]
pub(crate) async fn redeem_invitation(
    req: HttpRequest,
    services: web::Data<PasteService>,
    token: web::Path<String>,
    body: web::Json<InvitationInput>,
) -> HttpResponse {
    let client = auth::client_address(&req);
    let retry_after = match accounts::invitation_retry_after(&services.storage, &client).await {
        Ok(value) => value,
        Err(error) => return internal(error),
    };
    if let Some(retry_after) = retry_after {
        let mut response = error(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many invitation redemption attempts",
        );
        response.headers_mut().insert(
            header::RETRY_AFTER,
            header::HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );
        return response;
    }
    let token = token.into_inner();
    let username = body.username.clone();
    let password = body.password.clone();
    match accounts::redeem_invitation(&services.storage, &token, &username, &password).await {
        Ok(user) => match accounts::create_session(&services.storage, user.id, false).await {
            Ok((session, csrf, _)) => HttpResponse::Created()
                .cookie(cookies::session_cookie(session, false))
                .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
            Err(e) => internal(e),
        },
        Err(value) => {
            if let Err(error) = accounts::record_invitation_failure(&services.storage, &client).await {
                return internal(error);
            }
            domain_error(value)
        }
    }
}
