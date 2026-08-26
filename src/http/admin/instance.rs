use super::*;

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

impl From<crate::instance::settings::InstanceSettings> for InstanceSettingsResource {
    fn from(v: crate::instance::settings::InstanceSettings) -> Self {
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
    match crate::instance::settings::get(&services.storage).await {
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
    let current = match crate::instance::settings::get(&services.storage).await {
        Ok(v) => v,
        Err(e) => return domain_error(e),
    };
    let next = crate::instance::settings::InstanceSettings {
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
    match crate::instance::settings::replace(&services.storage, value.user_id().unwrap(), &next)
        .await
    {
        Ok(v) => {
            let _ = crate::instance::audit::record(
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

#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct AuditEventPage {
    items: Vec<AuditEventResource>,
    pagination: contract::Pagination,
}

#[utoipa::path(get, path="/admin/audit-events", tag="administration", params(AuditEventQuery), responses((status=200,description="Paginated owner audit events",body=AuditEventPage),(status=400,description="Invalid pagination parameter",body=crate::http::errors::ProblemDetails),(status=401,description="Authentication required",body=crate::http::errors::ProblemDetails),(status=403,description="Owner browser session required",body=crate::http::errors::ProblemDetails),(status=500,description="Internal error",body=crate::http::errors::ProblemDetails)), security(("sessionCookie"=[])))]
#[get("/admin/audit-events")]
pub(crate) async fn admin_audit_events(
    req: HttpRequest,
    services: web::Data<PasteService>,
    query: web::Query<AuditEventQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await.and_then(require_auth) {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = auth::require_owner_session(&value) {
        return r;
    }
    let query = query.into_inner();
    let (page, page_size) = match contract::page_parameters(query.page, query.page_size, 25) {
        Ok(value) => value,
        Err(message) => return error(StatusCode::BAD_REQUEST, "invalid_query", message),
    };
    match crate::instance::audit::list_page(
        &services.storage,
        query.search.as_deref(),
        page,
        page_size,
    )
    .await
    {
        Ok(result) => HttpResponse::Ok().json(AuditEventPage {
            items: result
                .items
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
                    created_at: contract::format_timestamp(v.created_at),
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
pub(crate) struct AuditEventQuery {
    /// Search actor, action, target type, target label, and target ID.
    search: Option<String>,
    /// One-based result page.
    #[param(minimum = 1, default = 1)]
    page: Option<u32>,
    /// Results per page, from 1 through 100.
    #[param(minimum = 1, maximum = 100, default = 25)]
    page_size: Option<u32>,
}
