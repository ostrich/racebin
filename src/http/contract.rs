use crate::account::{api_keys::ApiKey, AdminUser, Invitation, User};
use crate::services::{Attachment, Folder, FolderOverview};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiRootResponse {
    pub name: &'static str,
    pub version: &'static str,
    pub openapi_url: &'static str,
    pub capabilities_url: &'static str,
    pub languages_url: &'static str,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct UserResource {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub password_change_required: bool,
}

impl From<User> for UserResource {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            password_change_required: user.password_change_required,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiKeyIdentity {
    pub id: i64,
    pub name: String,
    pub scopes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SessionResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserResource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<ApiKeyIdentity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csrf_token: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct SessionCreatedResponse {
    pub user: UserResource,
    pub csrf_token: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiKeyCreatedResponse {
    pub key: ApiKeyResource,
    pub token: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiKeyResource {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub token_prefix: String,
    pub scopes: Vec<String>,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(format = DateTime)]
    pub last_used_at: Option<String>,
    pub enabled: bool,
}

impl From<ApiKey> for ApiKeyResource {
    fn from(key: ApiKey) -> Self {
        Self {
            id: key.id,
            user_id: key.user_id,
            name: key.name,
            token_prefix: key.token_prefix,
            scopes: key.scopes,
            created_at: super::dto::format_timestamp(key.created_at),
            last_used_at: key.last_used_at.map(super::dto::format_timestamp),
            enabled: key.enabled,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AdminUserResource {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub enabled: bool,
    pub password_change_required: bool,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(format = DateTime)]
    pub last_login_at: Option<String>,
    pub paste_count: i64,
    pub storage_bytes: i64,
    pub active_session_count: i64,
    pub api_key_count: i64,
    pub active_api_key_count: i64,
}

impl From<AdminUser> for AdminUserResource {
    fn from(user: AdminUser) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: user.role,
            enabled: user.enabled,
            password_change_required: user.password_change_required,
            created_at: super::dto::format_timestamp(user.created_at),
            last_login_at: user.last_login_at.map(super::dto::format_timestamp),
            paste_count: user.paste_count,
            storage_bytes: user.storage_bytes,
            active_session_count: user.active_session_count,
            api_key_count: user.api_key_count,
            active_api_key_count: user.active_api_key_count,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct FolderResource {
    pub id: i64,
    pub name: String,
    #[schema(format = DateTime)]
    pub created_at: String,
    pub paste_count: i64,
}

impl From<Folder> for FolderResource {
    fn from(folder: Folder) -> Self {
        Self {
            id: folder.id,
            name: folder.name,
            created_at: super::dto::format_timestamp(folder.created_at),
            paste_count: folder.paste_count,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct FolderOverviewResource {
    pub items: Vec<FolderResource>,
    pub total_count: i64,
    pub unfiled_count: i64,
}

impl From<FolderOverview> for FolderOverviewResource {
    fn from(overview: FolderOverview) -> Self {
        Self {
            items: overview.items.into_iter().map(Into::into).collect(),
            total_count: overview.total_count,
            unfiled_count: overview.unfiled_count,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct LinkResponse {
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct InvitationCreatedResponse {
    pub token: String,
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct InvitationResource {
    pub id: i64,
    pub token_prefix: String,
    #[schema(format = DateTime)]
    pub expires_at: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemed_by_username: Option<String>,
}

impl InvitationResource {
    pub(crate) fn from_invitation(
        invitation: Invitation,
        url: Option<String>,
        status: &'static str,
    ) -> Self {
        Self {
            id: invitation.id,
            token_prefix: invitation.token_prefix,
            expires_at: super::dto::format_timestamp(invitation.expires_at),
            status: status.to_string(),
            url,
            redeemed_by_username: invitation.redeemed_by_username,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AttachmentUploadResponse {
    pub items: Vec<Attachment>,
}
