use crate::account::{api_keys::ApiKey, AdminUser, Invitation, User};
use crate::services::{Attachment, Folder, FolderOverview};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum UserRole {
    User,
    Admin,
}

impl UserRole {
    fn from_storage(value: &str) -> Self {
        match value {
            "user" => Self::User,
            "admin" => Self::Admin,
            _ => unreachable!("database role constraint rejected an unknown role"),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiRootResponse {
    pub name: &'static str,
    pub version: &'static str,
    #[schema(format = "uri-reference")]
    pub openapi_url: &'static str,
    #[schema(format = "uri-reference")]
    pub capabilities_url: &'static str,
    #[schema(format = "uri-reference")]
    pub languages_url: &'static str,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct UserResource {
    #[schema(minimum = 1)]
    pub id: i64,
    pub username: String,
    pub role: UserRole,
    pub password_change_required: bool,
}

impl From<User> for UserResource {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: UserRole::from_storage(&user.role),
            password_change_required: user.password_change_required,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ApiKeyIdentity {
    #[schema(minimum = 1)]
    pub id: i64,
    pub name: String,
    #[schema(value_type = std::collections::HashSet<String>, min_items = 1)]
    pub scopes: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(untagged)]
pub(crate) enum SessionResponse {
    Browser(BrowserSessionResponse),
    Bearer(BearerSessionResponse),
    Anonymous(AnonymousSessionResponse),
}

#[derive(Serialize, ToSchema)]
pub(crate) struct BrowserSessionResponse {
    pub authenticated: bool,
    pub user: UserResource,
    pub csrf_token: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct BearerSessionResponse {
    pub authenticated: bool,
    pub api_key: ApiKeyIdentity,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AnonymousSessionResponse {
    pub authenticated: bool,
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
    #[schema(minimum = 1)]
    pub id: i64,
    #[schema(minimum = 1)]
    pub user_id: Option<i64>,
    pub name: String,
    pub token_prefix: String,
    #[schema(value_type = std::collections::HashSet<String>, min_items = 1)]
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
    #[schema(minimum = 1)]
    pub id: i64,
    pub username: String,
    pub role: UserRole,
    pub enabled: bool,
    pub password_change_required: bool,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(format = DateTime)]
    pub last_login_at: Option<String>,
    #[schema(minimum = 0)]
    pub paste_count: i64,
    #[schema(minimum = 0)]
    pub storage_bytes: i64,
    #[schema(minimum = 0)]
    pub active_session_count: i64,
    #[schema(minimum = 0)]
    pub api_key_count: i64,
    #[schema(minimum = 0)]
    pub active_api_key_count: i64,
}

impl From<AdminUser> for AdminUserResource {
    fn from(user: AdminUser) -> Self {
        Self {
            id: user.id,
            username: user.username,
            role: UserRole::from_storage(&user.role),
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
    #[schema(minimum = 1)]
    pub id: i64,
    pub name: String,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(minimum = 0)]
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
    #[schema(minimum = 0)]
    pub total_count: i64,
    #[schema(minimum = 0)]
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
    #[schema(format = "uri-reference")]
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct InvitationCreatedResponse {
    pub token: String,
    #[schema(format = "uri-reference")]
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct InvitationResource {
    #[schema(minimum = 1)]
    pub id: i64,
    pub token_prefix: String,
    #[schema(format = DateTime)]
    pub expires_at: String,
    pub status: InvitationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(format = "uri-reference")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemed_by_username: Option<String>,
}

#[derive(Clone, Copy, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InvitationStatus {
    Active,
    Redeemed,
    Revoked,
    Expired,
}

impl InvitationStatus {
    fn from_storage(value: &str) -> Self {
        match value {
            "Active" => Self::Active,
            "Redeemed" => Self::Redeemed,
            "Revoked" => Self::Revoked,
            "Expired" => Self::Expired,
            _ => unreachable!("invitation status calculation returned an unknown status"),
        }
    }
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
            status: InvitationStatus::from_storage(status),
            url,
            redeemed_by_username: invitation.redeemed_by_username,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AttachmentUploadResponse {
    #[schema(min_items = 1)]
    pub items: Vec<AttachmentUploadItem>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AttachmentUploadItem {
    #[schema(minimum = 1)]
    pub id: i64,
    pub filename: String,
    #[schema(minimum = 0)]
    pub size_bytes: i64,
}

impl From<Attachment> for AttachmentUploadItem {
    fn from(attachment: Attachment) -> Self {
        Self {
            id: attachment.id,
            filename: attachment.filename,
            size_bytes: attachment.size_bytes,
        }
    }
}
