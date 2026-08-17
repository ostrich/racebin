use super::*;

async fn require_manageable_user(
    services: &PasteService,
    principal: &Principal,
    user_id: i64,
) -> Result<accounts::AdminUser, HttpResponse> {
    match accounts::admin_user(&services.storage, user_id).await {
        Ok(Some(user)) if (user.role == "admin" || user.is_owner) && !principal.is_owner() => {
            Err(error(
                StatusCode::FORBIDDEN,
                "owner_required",
                "Only the owner can manage an administrator account",
            ))
        }
        Ok(Some(user)) => Ok(user),
        Ok(None) => Err(error(StatusCode::NOT_FOUND, "not_found", "User not found")),
        Err(value) => Err(domain_error(value)),
    }
}

async fn require_manageable_key(
    services: &PasteService,
    principal: &Principal,
    key_id: i64,
) -> Result<(), HttpResponse> {
    let keys = api_keys::list(&services.storage)
        .await
        .map_err(domain_error)?;
    let key = keys
        .into_iter()
        .find(|key| key.id == key_id)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found", "API key not found"))?;
    if let Some(user_id) = key.user_id {
        require_manageable_user(services, principal, user_id).await?;
    }
    Ok(())
}

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(admin_users)
        .service(admin_user)
        .service(admin_update_user)
        .service(admin_update_user_role)
        .service(admin_transfer_ownership)
        .service(admin_settings)
        .service(admin_replace_settings)
        .service(admin_audit_events)
        .service(admin_create_password_reset)
        .service(admin_revoke_user_sessions)
        .service(admin_revoke_user_keys)
        .service(admin_pastes)
        .service(admin_invitations)
        .service(admin_create_invitation)
        .service(admin_update_invitation)
        .service(admin_revoke_invitation)
        .service(admin_keys)
        .service(admin_update_key)
        .service(admin_delete_key);
}

#[utoipa::path(
    get, path = "/admin/users", tag = "administration",
    responses(
        (status = 200, description = "Administrative user summaries", body = [crate::http::contract::AdminUserResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/users")]
pub(crate) async fn admin_users(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    match accounts::list_admin_users(&services.storage).await {
        Ok(users) => HttpResponse::Ok().json(
            users
                .into_iter()
                .map(contract::AdminUserResource::from)
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    get, path = "/admin/users/{id}", tag = "administration",
    params(("id" = i64, Path, description = "User ID")),
    responses(
        (status = 200, description = "Administrative user detail", body = crate::http::contract::AdminUserResource),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/users/{id}")]
pub(crate) async fn admin_user(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "user:manage") {
        return response;
    }
    match accounts::admin_user(&services.storage, *id).await {
        Ok(Some(user)) => HttpResponse::Ok().json(contract::AdminUserResource::from(user)),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "User not found"),
        Err(error) => domain_error(error),
    }
}

#[utoipa::path(
    get, path = "/admin/pastes", tag = "administration",
    params(super::pastes::ApiPasteQuery, AdminPasteOwnerQuery),
    responses(
        (status = 200, description = "Canonical paginated paste summaries including ownership", body = crate::http::dto::PastePage),
        (status = 400, description = "Invalid filter", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with paste:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/pastes")]
pub(crate) async fn admin_pastes(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<super::pastes::ApiPasteQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = require_admin(&value, "paste:manage") {
        return response;
    }
    let query = match query.into_inner().into_admin_internal() {
        Ok(query) => query,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    if let Err(error) = crate::services::validate_paste_query(&query) {
        return domain_error(error);
    }
    match services.list_pastes(&value, &query, true).await {
        Ok(page) => {
            let total_pages = dto::total_pages(page.total_items, page.page_size);
            HttpResponse::Ok().json(dto::PastePage {
                items: page
                    .items
                    .into_iter()
                    .map(|paste| dto::summary(&req, &value, paste, true))
                    .collect(),
                pagination: dto::Pagination {
                    page: page.page,
                    page_size: page.page_size,
                    total_items: page.total_items,
                    total_pages,
                },
            })
        }
        Err(e) => domain_error(e),
    }
}

#[derive(utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)] // This DTO contributes the admin-only parameter to OpenAPI.
struct AdminPasteOwnerQuery {
    /// Restrict results to pastes owned by this positive user ID.
    #[param(minimum = 1)]
    owner_id: Option<i64>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct UserUpdate {
    #[serde(default, deserialize_with = "dto::optional_non_null")]
    enabled: Option<bool>,
}

#[cfg(test)]
mod user_update_tests {
    use super::UserUpdate;
    use serde_json::json;

    #[test]
    fn user_update_fields_may_not_be_null() {
        assert!(serde_json::from_value::<UserUpdate>(json!({ "enabled": null })).is_err());
    }
}

#[utoipa::path(
    patch, path = "/admin/users/{id}", tag = "administration",
    params(("id" = i64, Path, description = "User ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = UserUpdate,
    responses(
        (status = 204, description = "User updated"),
        (status = 400, description = "Invalid update or last-administrator invariant", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with user:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "User not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/admin/users/{id}")]
pub(crate) async fn admin_update_user(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<UserUpdate>,
) -> HttpResponse {
    if body.enabled.is_none() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_update",
            "Update must contain at least one field",
        );
    }
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "user:manage") {
        return r;
    }
    if let Err(response) = require_manageable_user(&services, &value, *id).await {
        return response;
    }
    let result = accounts::update_user(&services.storage, *id, body.enabled, None).await;
    match result {
        Ok(()) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "user.access_changed",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({"enabled":body.enabled}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct RoleUpdate {
    role: contract::UserRole,
}

#[utoipa::path(patch, path="/admin/users/{id}/role", tag="administration", params(("id"=i64,Path)), request_body=RoleUpdate, responses((status=204,description="Role updated"),(status=403,description="Owner browser session and recent authentication required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[patch("/admin/users/{id}/role")]
pub(crate) async fn admin_update_user_role(
    req: HttpRequest,
    services: web::Data<PasteService>,
    id: web::Path<i64>,
    body: web::Json<RoleUpdate>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value)
        .and_then(|_| auth::require_recent_authentication(&value))
    {
        return r;
    }
    let admin = match body.role {
        contract::UserRole::Admin => true,
        contract::UserRole::User => false,
        contract::UserRole::Owner => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_role",
                "Use ownership transfer to assign the owner role",
            )
        }
    };
    match accounts::set_role(&services.storage, *id, admin).await {
        Ok(()) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "user.role_changed",
                "user",
                Some(id.to_string()),
                None,
                serde_json::json!({"role": if admin {"admin"} else {"user"}}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct OwnershipTransfer {
    user_id: i64,
}

#[utoipa::path(post, path="/admin/ownership-transfer", tag="administration", request_body=OwnershipTransfer, responses((status=204,description="Ownership transferred"),(status=403,description="Owner browser session and recent authentication required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[post("/admin/ownership-transfer")]
pub(crate) async fn admin_transfer_ownership(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<OwnershipTransfer>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value)
        .and_then(|_| auth::require_recent_authentication(&value))
    {
        return r;
    }
    match accounts::transfer_ownership(&services.storage, value.user_id().unwrap(), body.user_id)
        .await
    {
        Ok(()) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "ownership.transferred",
                "user",
                Some(body.user_id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstanceSettingsResource {
    site_name: String,
    home_mode: String,
    public_explore_enabled: bool,
    invitations_enabled: bool,
    attachments_enabled: bool,
    qr_codes_enabled: bool,
    default_format: String,
    default_language: String,
    default_visibility: String,
    default_expiration_seconds: Option<i64>,
}

impl From<crate::services::settings::InstanceSettings> for InstanceSettingsResource {
    fn from(v: crate::services::settings::InstanceSettings) -> Self {
        Self {
            site_name: v.site_name,
            home_mode: v.home_mode,
            public_explore_enabled: v.public_explore_enabled,
            invitations_enabled: v.invitations_enabled,
            attachments_enabled: v.attachments_enabled,
            qr_codes_enabled: v.qr_codes_enabled,
            default_format: v.default_format,
            default_language: v.default_language,
            default_visibility: v.default_visibility,
            default_expiration_seconds: v.default_expiration_seconds,
        }
    }
}

#[utoipa::path(get, path="/admin/settings", tag="administration", responses((status=200,description="Instance settings",body=InstanceSettingsResource),(status=403,description="Owner browser session required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[get("/admin/settings")]
pub(crate) async fn admin_settings(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value) {
        return r;
    }
    match crate::services::settings::get(&services.storage).await {
        Ok(v) => HttpResponse::Ok().json(InstanceSettingsResource::from(v)),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(put, path="/admin/settings", tag="administration", request_body=InstanceSettingsResource, responses((status=200,description="Updated instance settings",body=InstanceSettingsResource),(status=403,description="Owner browser session required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[put("/admin/settings")]
pub(crate) async fn admin_replace_settings(
    req: HttpRequest,
    services: web::Data<PasteService>,
    body: web::Json<InstanceSettingsResource>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value) {
        return r;
    }
    if body.qr_codes_enabled && ARGS.public_url.is_none() {
        return error(
            StatusCode::BAD_REQUEST,
            "public_url_required",
            "Configure a public URL before enabling QR codes",
        );
    }
    let current = match crate::services::settings::get(&services.storage).await {
        Ok(v) => v,
        Err(e) => return domain_error(e),
    };
    let next = crate::services::settings::InstanceSettings {
        site_name: body.site_name.clone(),
        home_mode: body.home_mode.clone(),
        public_explore_enabled: body.public_explore_enabled,
        invitations_enabled: body.invitations_enabled,
        attachments_enabled: body.attachments_enabled,
        qr_codes_enabled: body.qr_codes_enabled,
        default_format: body.default_format.clone(),
        default_language: body.default_language.clone(),
        default_visibility: body.default_visibility.clone(),
        default_expiration_seconds: body.default_expiration_seconds,
        updated_at: current.updated_at,
        updated_by_user_id: current.updated_by_user_id,
    };
    match crate::services::settings::replace(&services.storage, value.user_id().unwrap(), &next)
        .await
    {
        Ok(v) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "instance.settings_changed",
                "instance",
                Some("1".into()),
                None,
                &body.0,
            )
            .await;
            HttpResponse::Ok().json(InstanceSettingsResource::from(v))
        }
        Err(e) => domain_error(e),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct AuditEventResource {
    id: i64,
    actor_username: String,
    actor_api_key_id: Option<i64>,
    action: String,
    target_type: String,
    target_id: Option<String>,
    target_label: Option<String>,
    details: serde_json::Value,
    #[schema(format = DateTime)]
    created_at: String,
}

#[utoipa::path(get, path="/admin/audit-events", tag="administration", responses((status=200,description="Recent owner audit events",body=[AuditEventResource]),(status=403,description="Owner browser session required",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[get("/admin/audit-events")]
pub(crate) async fn admin_audit_events(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value) {
        return r;
    }
    match crate::services::audit::list(&services.storage, 100).await {
        Ok(items) => HttpResponse::Ok().json(
            items
                .into_iter()
                .map(|v| AuditEventResource {
                    id: v.id,
                    actor_username: v.actor_username,
                    actor_api_key_id: v.actor_api_key_id,
                    action: v.action,
                    target_type: v.target_type,
                    target_id: v.target_id,
                    target_label: v.target_label,
                    details: serde_json::from_str(&v.details).unwrap_or(serde_json::Value::Null),
                    created_at: dto::format_timestamp(v.created_at),
                })
                .collect::<Vec<_>>(),
        ),
        Err(e) => domain_error(e),
    }
}

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
            let _ = crate::services::audit::record(
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
                url: super::dto::absolute(&req, &format!("/password-reset/{token}")),
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
            let _ = crate::services::audit::record(
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
                let _ = crate::services::audit::record(
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

#[utoipa::path(
    get, path = "/admin/invitations", tag = "administration",
    params(InvitationQuery),
    responses(
        (status = 200, description = "Paginated active invitations or terminal invitation history", body = crate::http::contract::InvitationPage),
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
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(25).clamp(1, 100);
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
                        i.token.as_ref().map(|token| {
                            super::dto::absolute(&req, &format!("/invitations/{token}"))
                        })
                    } else {
                        None
                    };
                    contract::InvitationResource::from_invitation(i, url, status)
                })
                .collect(),
            pagination: dto::Pagination {
                page: result.page,
                page_size: result.page_size,
                total_items: result.total_items,
                total_pages: dto::total_pages(result.total_items, result.page_size),
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
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
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
    let invitations_enabled = match crate::services::settings::get(&services.storage).await {
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
            let url = super::dto::absolute(&req, &format!("/invitations/{token}"));
            let _ = crate::services::audit::record(
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
            let _ = crate::services::audit::record(
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
            let _ = crate::services::audit::record(
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

#[utoipa::path(
    get, path = "/admin/api-keys", tag = "administration",
    responses(
        (status = 200, description = "All API keys", body = [crate::http::contract::ApiKeyResource]),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/admin/api-keys")]
pub(crate) async fn admin_keys(
    req: HttpRequest,
    services: web::Data<PasteService>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    match api_keys::list(&services.storage).await {
        Ok(mut keys) => {
            if !value.is_owner() {
                let privileged_users = match accounts::list_users(&services.storage).await {
                    Ok(users) => users
                        .into_iter()
                        .filter(|user| user.is_admin())
                        .map(|user| user.id)
                        .collect::<std::collections::HashSet<_>>(),
                    Err(error) => return domain_error(error),
                };
                keys.retain(|key| {
                    key.user_id
                        .is_none_or(|user_id| !privileged_users.contains(&user_id))
                });
            }
            HttpResponse::Ok().json(
                keys.into_iter()
                    .map(contract::ApiKeyResource::from)
                    .collect::<Vec<_>>(),
            )
        }
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    patch, path = "/admin/api-keys/{id}", tag = "administration",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body = EnabledInput,
    responses(
        (status = 204, description = "API key state updated"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[patch("/admin/api-keys/{id}")]
pub(crate) async fn admin_update_key(
    req: HttpRequest,
    services: web::Data<PasteService>,
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
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    if let Err(response) = require_manageable_key(&services, &value, *id).await {
        return response;
    }
    match api_keys::set_enabled(&services.storage, *id, body.enabled).await {
        Ok(true) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "api_key.access_changed",
                "api_key",
                Some(id.to_string()),
                None,
                serde_json::json!({"enabled":body.enabled}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}

#[utoipa::path(
    delete, path = "/admin/api-keys/{id}", tag = "administration",
    params(("id" = i64, Path, description = "API key ID"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "API key deleted"),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Administrator with api_key:manage required", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "API key not found", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ), security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/admin/api-keys/{id}")]
pub(crate) async fn admin_delete_key(
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
    if let Err(r) = require_admin(&value, "api_key:manage") {
        return r;
    }
    if let Err(response) = require_manageable_key(&services, &value, *id).await {
        return response;
    }
    match api_keys::delete(&services.storage, *id).await {
        Ok(true) => {
            let _ = crate::services::audit::record(
                &services.storage,
                &value,
                "api_key.deleted",
                "api_key",
                Some(id.to_string()),
                None,
                serde_json::json!({}),
            )
            .await;
            HttpResponse::NoContent().finish()
        }
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "API key not found"),
        Err(e) => domain_error(e),
    }
}
