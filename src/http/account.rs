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

#[utoipa::path(
    get, path = "/session", tag = "account",
    responses(
        (status = 200, description = "Current browser session, bearer-key identity, or anonymous state", body = crate::http::contract::SessionResponse),
        (status = 401, description = "Invalid bearer credential", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/session")]
pub(crate) async fn get_session(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    match principal(&services, &req).await {
        Ok(Principal::Session(session)) => HttpResponse::Ok().json(
            contract::SessionResponse::Browser(contract::BrowserSessionResponse {
                authenticated: true,
                user: session.user.into(),
                csrf_token: session.csrf_token,
            }),
        ),
        Ok(Principal::ApiKey(key)) => HttpResponse::Ok().json(contract::SessionResponse::Bearer(
            contract::BearerSessionResponse {
                authenticated: true,
                api_key: contract::ApiKeyIdentity {
                    id: key.id,
                    name: key.name,
                    scopes: key.scopes,
                },
            },
        )),
        Ok(Principal::Anonymous) => HttpResponse::Ok().json(contract::SessionResponse::Anonymous(
            contract::AnonymousSessionResponse {
                authenticated: false,
            },
        )),
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

#[utoipa::path(
    post, path = "/session", tag = "account",
    request_body = LoginInput,
    responses(
        (status = 200, description = "Browser session created", body = crate::http::contract::SessionCreatedResponse,
            headers(("Set-Cookie" = String, description = "HTTP-only Racebin session cookie"))),
        (status = 400, description = "Malformed credentials", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Invalid credentials", body = crate::http::errors::ProblemDetails),
        (status = 429, description = "Login rate limit exceeded", body = crate::http::errors::ProblemDetails,
            headers(("Retry-After" = i64, description = "Seconds before another attempt"))),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(())
)]
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
            Err(error) => return domain_error(error),
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
                return domain_error(error);
            }
            match accounts::create_session(
                &services.storage,
                user.id,
                body.remember.unwrap_or(false),
            )
            .await
            {
                Ok((token, csrf, _)) => HttpResponse::Ok()
                    .cookie(cookies::session_cookie(
                        token,
                        body.remember.unwrap_or(false),
                    ))
                    .json(contract::SessionCreatedResponse {
                        user: user.into(),
                        csrf_token: csrf,
                    }),
                Err(e) => domain_error(e),
            }
        }
        Ok(None) => {
            if let Err(error) =
                accounts::record_login_failure(&services.storage, &body.username, &client).await
            {
                return domain_error(error);
            }
            error(
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "Invalid username or password",
            )
        }
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    delete, path = "/session", tag = "account",
    params(("X-CSRF-Token" = String, Header, description = "Current browser session CSRF token")),
    responses(
        (status = 204, description = "Session deleted",
            headers(("Set-Cookie" = String, description = "Expired Racebin session cookie"))),
        (status = 400, description = "Browser session required", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "CSRF validation failed", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("sessionCookie" = []))
)]
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
            return domain_error(e);
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

#[utoipa::path(
    patch, path = "/account/password", tag = "account",
    params(("X-CSRF-Token" = String, Header, description = "Current browser session CSRF token")),
    request_body = PasswordInput,
    responses(
        (status = 204, description = "Password changed and existing sessions revoked"),
        (status = 400, description = "Invalid password or browser session required", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Current password is incorrect", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "CSRF validation failed", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("sessionCookie" = []))
)]
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
        Err(error) => Err(error),
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

#[utoipa::path(
    post, path = "/password-resets/{token}", tag = "account",
    params(("token" = String, Path, description = "One-time password-reset token")),
    request_body = PasswordResetInput,
    responses(
        (status = 204, description = "Password reset and existing sessions revoked"),
        (status = 400, description = "Invalid, expired, or consumed token; or invalid password", body = crate::http::errors::ProblemDetails),
        (status = 429, description = "Reset-attempt rate limit exceeded", body = crate::http::errors::ProblemDetails,
            headers(("Retry-After" = i64, description = "Seconds before another attempt"))),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(())
)]
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
        Err(error) => return domain_error(error),
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
                return domain_error(error);
            }
            domain_error(value)
        }
    }
}

#[utoipa::path(
    post, path = "/invitations/{token}/redeem", tag = "account",
    params(("token" = String, Path, description = "One-time invitation token")),
    request_body = InvitationInput,
    responses(
        (status = 201, description = "Account and browser session created", body = crate::http::contract::SessionCreatedResponse,
            headers(("Set-Cookie" = String, description = "HTTP-only Racebin session cookie"))),
        (status = 400, description = "Invalid invitation, username, or password", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Username already exists", body = crate::http::errors::ProblemDetails),
        (status = 429, description = "Invitation-attempt rate limit exceeded", body = crate::http::errors::ProblemDetails,
            headers(("Retry-After" = i64, description = "Seconds before another attempt"))),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(())
)]
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
        Err(error) => return domain_error(error),
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
                .json(contract::SessionCreatedResponse {
                    user: user.into(),
                    csrf_token: csrf,
                }),
            Err(e) => domain_error(e),
        },
        Err(value) => {
            if let Err(error) =
                accounts::record_invitation_failure(&services.storage, &client).await
            {
                return domain_error(error);
            }
            domain_error(value)
        }
    }
}
