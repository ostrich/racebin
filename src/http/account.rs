use super::*;

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(get_session)
        .service(login)
        .service(logout)
        .service(change_password)
        .service(accept_invite);
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
                    .cookie(cookies::session_cookie(
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
                .cookie(cookies::session_cookie(session, false))
                .json(json!({"user": {"id": user.id, "username": user.username, "role": user.role}, "csrf_token": csrf})),
            Err(e) => internal(e),
        },
        Err(e) => error(StatusCode::BAD_REQUEST, "invalid_invitation", e),
    }
}
