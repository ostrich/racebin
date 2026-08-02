use crate::account::{api_keys::ApiKey, User};
use crate::services::Attachment;
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
    pub key: ApiKey,
    pub token: String,
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
    pub expires_at: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redeemed_by_username: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct AttachmentUploadResponse {
    pub items: Vec<Attachment>,
}
