use super::*;

mod admin_api_keys;
mod instance;
mod invitations;
mod user_security;
mod users;

pub(crate) use admin_api_keys::*;
pub(crate) use instance::*;
pub(crate) use invitations::*;
pub(crate) use user_security::*;
pub(crate) use users::*;

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
    let key = api_keys::get(&services.storage, key_id)
        .await
        .map_err(domain_error)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found", "API key not found"))?;
    if let Some(user_id) = key.user_id {
        require_manageable_user(services, principal, user_id).await?;
    }
    Ok(())
}

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(admin_users)
        .service(admin_summary)
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
