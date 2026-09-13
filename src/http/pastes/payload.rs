use super::*;

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams, utoipa::ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(super) struct FlatCreateRequest {
    #[schema(max_length = 200)]
    #[param(max_length = 200)]
    pub(super) title: Option<String>,
    pub(super) format: Option<String>,
    pub(super) content: Option<String>,
    pub(super) language: Option<String>,
    pub(super) visibility: Option<String>,
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) folder_id: Option<i64>,
    /// Absolute RFC 3339 expiration time. Cannot be combined with `expires_in`.
    #[schema(format = DateTime)]
    #[param(format = DateTime)]
    pub(super) expires_at: Option<String>,
    /// Positive lifetime in seconds from creation. Cannot be combined with `expires_at`.
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) expires_in: Option<i64>,
    #[schema(minimum = 1)]
    #[param(minimum = 1)]
    pub(super) read_limit: Option<i64>,
}

#[derive(Clone, Default, Deserialize, PartialEq, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCreateQuery {
    #[param(max_length = 200)]
    pub(super) title: Option<String>,
    /// Syntax identifier for text/plain. text/markdown always uses markdown;
    /// text/html does not accept a language.
    pub(super) language: Option<String>,
    pub(super) visibility: Option<String>,
    #[param(minimum = 1)]
    pub(super) folder_id: Option<i64>,
    /// Absolute RFC 3339 expiration time. Cannot be combined with `expires_in`.
    #[param(format = DateTime)]
    pub(super) expires_at: Option<String>,
    /// Positive lifetime in seconds from creation. Cannot be combined with `expires_at`.
    #[param(minimum = 1)]
    pub(super) expires_in: Option<i64>,
    #[param(minimum = 1)]
    pub(super) read_limit: Option<i64>,
}

impl From<RawCreateQuery> for FlatCreateRequest {
    fn from(value: RawCreateQuery) -> Self {
        Self {
            title: value.title,
            language: value.language,
            visibility: value.visibility,
            folder_id: value.folder_id,
            expires_at: value.expires_at,
            expires_in: value.expires_in,
            read_limit: value.read_limit,
            ..Self::default()
        }
    }
}

/// Schema-only representation of the atomic multipart create request.
#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
pub(super) struct MultipartCreateRequest {
    #[schema(max_length = 200)]
    title: Option<String>,
    format: Option<String>,
    content: Option<String>,
    language: Option<String>,
    visibility: Option<String>,
    #[schema(minimum = 1)]
    folder_id: Option<i64>,
    /// Absolute RFC 3339 expiration time. Cannot be combined with `expires_in`.
    #[schema(format = DateTime)]
    expires_at: Option<String>,
    /// Positive lifetime in seconds from creation. Cannot be combined with `expires_at`.
    #[schema(minimum = 1)]
    expires_in: Option<i64>,
    #[schema(minimum = 1)]
    read_limit: Option<i64>,
    #[schema(value_type = Vec<Value>, min_items = 1)]
    file: Option<Vec<String>>,
}

impl FlatCreateRequest {
    pub(super) fn structured(self) -> Result<CreatePasteRequest, String> {
        let format = self.format.as_deref().unwrap_or("text");
        let body = match format {
            "text" => Some(BodyInput::Text {
                content: self.content.unwrap_or_default(),
                language: self.language,
            }),
            "markdown" => {
                if self.language.is_some() {
                    return Err("Rich text does not accept a language".into());
                }
                Some(BodyInput::Markdown {
                    content: self.content.unwrap_or_default(),
                })
            }
            _ => return Err("format must be text or markdown".into()),
        };
        Ok(CreatePasteRequest {
            title: self.title,
            body,
            visibility: self.visibility,
            folder_id: self.folder_id,
            expires_at: self.expires_at,
            expires_in: self.expires_in,
            read_limit: self.read_limit,
        })
    }
}

pub(super) type StagedFile = crate::attachment_storage::StagedUpload;
