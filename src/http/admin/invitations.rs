use super::*;

#[utoipa::path(
    get, path = "/admin/invitations", tag = "administration",
    params(InvitationQuery),
    responses(
        (status = 200, description = "Paginated active invitations or terminal invitation history", body = crate::http::contract::InvitationPage),
        (status = 400, description = "Invalid view, status, search, or pagination parameter", body = crate::http::errors::ProblemDetails),
        (status = 400, description = "Invalid invitation filter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/invitations")]
pub(crate) async fn admin_invitations(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<InvitationQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    let query = query.into_inner();
    let history = match query.view.as_deref().unwrap_or("active") {
        "active" => false,
        "history" => true,
        _ => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_query",
                "View must be active or history",
            )
        }
    };
    if query.status.is_some() && !history {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Status is available only for invitation history",
        );
    }
    if !matches!(
        query.status.as_deref(),
        None | Some("redeemed" | "revoked" | "expired")
    ) {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "Unknown invitation status",
        );
    }
    let (page, page_size) = match contract::page_parameters(query.page, query.page_size, 25) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    match accounts::list_invitations(
        &services.storage,
        history,
        query.search.as_deref(),
        query.status.as_deref(),
        page,
        page_size,
    )
    .await
    {
        Ok(result) => HttpResponse::Ok().json(contract::InvitationPage {
            items: result
                .items
                .into_iter()
                .map(|i| {
                    let status = i.status();
                    let url = if i.is_active() {
                        i.token
                            .as_ref()
                            .map(|token| contract::absolute(&req, &format!("/invitations/{token}")))
                    } else {
                        None
                    };
                    contract::InvitationResource::from_invitation(i, url, status)
                })
                .collect(),
            pagination: contract::Pagination {
                page: result.page,
                page_size: result.page_size,
                total_items: result.total_items,
                total_pages: contract::total_pages(result.total_items, result.page_size),
            },
        }),
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct InvitationQuery {
    /// Select currently usable invitations or terminal history.
    #[param(value_type = String, default = "active")]
    view: Option<String>,
    /// Search comments, creators, recipients, and token prefixes.
    search: Option<String>,
    /// Restrict history to one terminal status.
    #[param(value_type = String)]
    status: Option<String>,
    /// One-based result page.
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
    /// Results per page, from 1 through 100.
    #[param(minimum = 1, maximum = 100, default = 25)]
    page_size: Option<u32>,
}

#[utoipa::path(
    post, path = "/admin/invitations", tag = "administration",
    params(("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body(content = crate::http::contract::InvitationCreateInput, description = "Optional private administrative comment"),
    responses(
        (status = 201, description = "Invitation created", body = crate::http::contract::InvitationCreatedResponse),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage and user identity required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/admin/invitations")]
pub(crate) async fn admin_create_invitation(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<contract::InvitationCreateInput>,
) -> HttpResponse {
    let invitations_enabled = match crate::instance::settings::get(&services.storage).await {
        Ok(settings) => settings.invitations_enabled,
        Err(value) => return domain_error(value),
    };
    if !invitations_enabled {
        return error(
            StatusCode::FORBIDDEN,
            "invitations_disabled",
            "Invitations are disabled",
        );
    }
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    let Some(user_id) = value.user_id() else {
        return error(StatusCode::FORBIDDEN, "forbidden", "User identity required");
    };
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if comment.is_some_and(|value| value.chars().count() > 200) {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_comment",
            "Invitation comments must be 200 characters or fewer",
        );
    }
    match accounts::create_invitation(&services.storage, user_id, comment).await {
        Ok(token) => {
            let url = contract::absolute(&req, &format!("/invitations/{token}"));
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "invitation.created",
                "invitation",
                None,
                None,
                serde_json::json!({"comment": comment}),
            )
            .await;
            HttpResponse::Created().json(contract::InvitationCreatedResponse { token, url })
        }
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    patch, path = "/admin/invitations/{id}", tag = "administration",
    params(("id" = i64, Path, description = "Invitation ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body(content = crate::http::contract::InvitationCreateInput, description = "Replacement private administrative comment"),
    responses(
        (status = 204, description = "Invitation comment updated"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Invitation not found", body = crate::http::errors::ProblemDetails),
        (status = 422, description = "Invalid comment", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/admin/invitations/{id}")]
pub(crate) async fn admin_update_invitation(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<contract::InvitationCreateInput>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|principal| require_mutation(&services, &req, principal))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "invitation:manage") {
        return response;
    }
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|comment| !comment.is_empty())
        .map(str::to_owned);
    if comment
        .as_deref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_comment",
            "Invitation comments must be 200 characters or fewer",
        );
    }
    match accounts::update_invitation_comment(&services.storage, *id, comment.as_deref()).await {
        Ok(true) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "invitation.comment_updated",
                "invitation",
                Some(id.to_string()),
                comment,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Invitation not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    delete, path = "/admin/invitations/{id}", tag = "administration",
    params(("id" = i64, Path, description = "Invitation ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "Invitation revoked"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with invitation:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Invitation not found or already redeemed", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/invitations/{id}")]
pub(crate) async fn admin_revoke_invitation(
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
    if let Err(r) = require_admin(&value, "invitation:manage") {
        return r;
    }
    match accounts::revoke_invitation(&services.storage, *id).await {
        Ok(true) => {
            let _ = crate::instance::audit::record(
                &services.storage,
                &value,
                "invitation.revoked",
                "invitation",
                Some(id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "Invitation not found"),
        Err(e) => domain_error(e),
    }
}
