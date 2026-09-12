use crate::http::*;
use crate::pastes::{render_markdown, Attachment, Paste};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;

pub(crate) fn optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn optional_nullable<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BodyInput {
    /// Plain text. Racebin stores the content as text and never interprets it as HTML.
    Text {
        content: String,
        #[serde(default)]
        language: Option<String>,
    },
    /// GitHub-Flavored Markdown. Raw HTML other than editor-generated hard breaks and embedded images are not accepted.
    Markdown {
        /// Canonical Markdown source.
        content: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatePasteRequest {
    #[serde(default, deserialize_with = "optional_non_null")]
    #[schema(max_length = 200)]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub body: Option<BodyInput>,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub visibility: Option<String>,
    #[serde(default, deserialize_with = "optional_non_null")]
    #[schema(minimum = 1)]
    pub folder_id: Option<i64>,
    #[serde(default, deserialize_with = "optional_non_null")]
    #[schema(format = DateTime)]
    pub expires_at: Option<String>,
    #[serde(default, deserialize_with = "optional_non_null")]
    /// Positive lifetime in seconds from the time of creation. Cannot be combined with `expires_at`.
    #[schema(minimum = 1)]
    pub expires_in: Option<i64>,
    #[serde(default, deserialize_with = "optional_non_null")]
    #[schema(minimum = 1)]
    pub read_limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdatePasteRequest {
    /// Replace the title. Explicit `null` is invalid; omit this field to leave it unchanged.
    #[serde(default, deserialize_with = "optional_non_null")]
    #[schema(max_length = 200)]
    pub title: Option<String>,
    /// Replace the body. Explicit `null` is invalid; omit this field to leave it unchanged.
    #[serde(default, deserialize_with = "optional_non_null")]
    pub body: Option<BodyInput>,
    /// Replace the visibility. Explicit `null` is invalid; omit this field to leave it unchanged.
    #[serde(default, deserialize_with = "optional_non_null")]
    pub visibility: Option<String>,
    /// Move the paste to a folder, or use `null` to make it unfiled.
    #[serde(default, deserialize_with = "optional_nullable")]
    #[schema(minimum = 1)]
    pub folder_id: Option<Option<i64>>,
    /// Replace the expiration, or use `null` to make the paste permanent.
    #[serde(default, deserialize_with = "optional_nullable")]
    #[schema(format = DateTime)]
    pub expires_at: Option<Option<String>>,
    /// Replace the read limit, or use `null` to make reads unlimited.
    #[serde(default, deserialize_with = "optional_nullable")]
    #[schema(minimum = 1)]
    pub read_limit: Option<Option<i64>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "format", rename_all = "snake_case")]
pub(crate) enum BodyOutput {
    /// Plain text that is safe to render as text, not HTML.
    Text { content: String, language: String },
    /// Canonical Markdown and its derived safe representations.
    Markdown {
        /// Canonical Markdown source.
        content: String,
        /// Sanitized rendered HTML.
        rendered_html: String,
        /// Plain-text projection.
        plain_text: String,
    },
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct AttachmentResource {
    #[schema(minimum = 1)]
    pub id: i64,
    pub filename: String,
    #[schema(minimum = 0)]
    pub size_bytes: i64,
    #[schema(format = "uri-reference")]
    pub url: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PasteMetadataResource {
    pub id: String,
    #[schema(format = "uri-reference")]
    pub url: String,
    #[schema(format = "uri-reference")]
    pub api_url: String,
    #[schema(format = "uri-reference")]
    pub read_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(format = "uri-reference")]
    pub raw_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(format = "uri-reference")]
    pub archive_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(format = "uri-reference")]
    pub source_url: Option<String>,
    pub title: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(minimum = 1)]
    pub owner_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(minimum = 1)]
    pub folder_id: Option<i64>,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(format = DateTime)]
    pub updated_at: String,
    #[schema(format = DateTime)]
    pub modified_at: Option<String>,
    #[schema(format = DateTime)]
    pub expires_at: Option<String>,
    #[schema(format = DateTime)]
    pub last_read_at: Option<String>,
    #[schema(minimum = 0)]
    pub read_count: i64,
    #[schema(minimum = 1)]
    pub read_limit: Option<i64>,
    #[schema(minimum = 0)]
    pub attachment_count: i64,
    /// First ordered attachment filename when the paste has no textual content.
    pub attachment_only_filename: Option<String>,
    #[schema(minimum = 0)]
    pub size_bytes: i64,
    pub attachments: Vec<AttachmentResource>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PasteResource {
    #[serde(flatten)]
    pub metadata: PasteMetadataResource,
    pub body: BodyOutput,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PasteSummary {
    pub id: String,
    #[schema(format = "uri-reference")]
    pub url: String,
    pub title: String,
    pub format: String,
    pub language: Option<String>,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(minimum = 1)]
    pub owner_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(minimum = 1)]
    pub folder_id: Option<i64>,
    #[schema(format = DateTime)]
    pub created_at: String,
    #[schema(format = DateTime)]
    pub updated_at: String,
    #[schema(format = DateTime)]
    pub modified_at: Option<String>,
    #[schema(format = DateTime)]
    pub expires_at: Option<String>,
    #[schema(format = DateTime)]
    pub last_read_at: Option<String>,
    #[schema(minimum = 0)]
    pub read_count: i64,
    #[schema(minimum = 1)]
    pub read_limit: Option<i64>,
    #[schema(minimum = 0)]
    pub attachment_count: i64,
    /// First ordered attachment filename when the paste has no textual content.
    pub attachment_only_filename: Option<String>,
    #[schema(minimum = 0)]
    pub size_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct Pagination {
    #[schema(minimum = 1)]
    pub page: u32,
    #[schema(minimum = 1, maximum = 100)]
    pub page_size: u32,
    #[schema(minimum = 0)]
    pub total_items: i64,
    pub total_pages: u32,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PastePage {
    pub items: Vec<PasteSummary>,
    pub pagination: Pagination,
}

pub(crate) fn total_pages(total_items: i64, page_size: u32) -> u32 {
    if total_items <= 0 || page_size == 0 {
        return 0;
    }
    let pages = u64::try_from(total_items)
        .unwrap_or_default()
        .div_ceil(u64::from(page_size));
    u32::try_from(pages).unwrap_or(u32::MAX)
}

pub(crate) fn page_parameters(
    page: Option<u32>,
    page_size: Option<u32>,
    default_page_size: u32,
) -> Result<(u32, u32), &'static str> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(default_page_size);
    if page == 0 {
        return Err("page must be at least 1");
    }
    if !(1..=crate::limits::MAX_PAGE_SIZE).contains(&page_size) {
        return Err("page_size must be between 1 and 100");
    }
    Ok((page, page_size))
}

#[cfg(test)]
mod pagination_tests {
    use super::page_parameters;

    #[test]
    fn pagination_defaults_and_validates_bounds() {
        assert_eq!(page_parameters(None, None, 25), Ok((1, 25)));
        assert_eq!(page_parameters(Some(2), Some(100), 25), Ok((2, 100)));
        assert_eq!(
            page_parameters(Some(0), None, 25),
            Err("page must be at least 1")
        );
        assert_eq!(
            page_parameters(None, Some(0), 25),
            Err("page_size must be between 1 and 100")
        );
        assert_eq!(
            page_parameters(None, Some(101), 25),
            Err("page_size must be between 1 and 100")
        );
    }
}

impl CreatePasteRequest {
    pub fn into_input(self, now: i64) -> Result<PasteInput, String> {
        if self.expires_at.is_some() && self.expires_in.is_some() {
            return Err("expires_at and expires_in cannot be combined".into());
        }
        let expires_at = match (self.expires_at, self.expires_in) {
            (Some(value), None) => Some(parse_timestamp(&value)?),
            (None, Some(seconds)) if seconds > 0 => Some(now.saturating_add(seconds)),
            (None, Some(_)) => return Err("expires_in must be positive".into()),
            (None, None) => None,
            _ => unreachable!(),
        };
        let (content, content_kind, language) = body_into_internal(self.body)?;
        Ok(PasteInput {
            title: self.title,
            content: Some(content),
            content_kind: Some(content_kind),
            language: Some(language),
            visibility: self.visibility,
            expires_at: Some(expires_at),
            read_limit: Some(self.read_limit),
            folder_id: Some(self.folder_id),
        })
    }
}

impl UpdatePasteRequest {
    pub fn into_input(self) -> Result<PasteInput, String> {
        if self.title.is_none()
            && self.body.is_none()
            && self.visibility.is_none()
            && self.folder_id.is_none()
            && self.expires_at.is_none()
            && self.read_limit.is_none()
        {
            return Err("Update must contain at least one field".into());
        }
        let (content, content_kind, language) = match self.body {
            Some(body) => {
                let (content, format, language) = body_into_internal(Some(body))?;
                (Some(content), Some(format), Some(language))
            }
            None => (None, None, None),
        };
        let expires_at = self
            .expires_at
            .map(|value| value.map(|value| parse_timestamp(&value)).transpose())
            .transpose()?;
        Ok(PasteInput {
            title: self.title,
            content,
            content_kind,
            language,
            visibility: self.visibility,
            expires_at,
            read_limit: self.read_limit,
            folder_id: self.folder_id,
        })
    }
}

fn body_into_internal(body: Option<BodyInput>) -> Result<(String, String, String), String> {
    match body.unwrap_or(BodyInput::Text {
        content: String::new(),
        language: Some("auto".into()),
    }) {
        BodyInput::Text { content, language } => {
            let mut language = normalize_language(language.as_deref().unwrap_or("auto"))?;
            if language == "auto" {
                language = detect_language(&content).to_string();
            }
            Ok((content, "text".into(), language))
        }
        BodyInput::Markdown { content } => {
            render_markdown(&content)?;
            Ok((content, "markdown".into(), "plaintext".into()))
        }
    }
}

pub(crate) fn metadata_resource(
    request: &HttpRequest,
    principal: &Principal,
    paste: Paste,
    grant_token: Option<&str>,
) -> PasteMetadataResource {
    let own = principal.can("paste:manage") || principal.user_id() == paste.owner_id;
    let attachment_count = paste.attachments.len() as i64;
    let attachments = paste
        .attachments
        .iter()
        .map(|attachment| attachment_resource(request, &paste.id, attachment, grant_token))
        .collect();
    let archive_url = (attachment_count > 0).then(|| {
        let base = absolute(request, &format!("/api/v1/pastes/{}/archive", paste.id));
        match grant_token {
            Some(token) => format!("{base}?read_token={token}"),
            None => base,
        }
    });
    let raw_base = absolute(request, &format!("/api/v1/pastes/{}/raw", paste.id));
    let raw_url = if own || paste.read_limit.is_none() {
        Some(raw_base)
    } else {
        grant_token.map(|token| format!("{raw_base}?read_token={token}"))
    };
    PasteMetadataResource {
        id: paste.id.clone(),
        url: absolute(request, &format!("/pastes/{}", paste.id)),
        api_url: absolute(request, &format!("/api/v1/pastes/{}", paste.id)),
        read_url: absolute(request, &format!("/api/v1/pastes/{}/reads", paste.id)),
        raw_url,
        archive_url,
        source_url: own.then(|| absolute(request, &format!("/api/v1/pastes/{}/source", paste.id))),
        title: paste.title,
        format: paste.content_kind.clone(),
        language: (paste.content_kind == "text").then_some(paste.language),
        visibility: paste.visibility,
        owner_id: own.then_some(paste.owner_id).flatten(),
        folder_id: own.then_some(paste.folder_id).flatten(),
        created_at: format_timestamp(paste.created_at),
        updated_at: format_timestamp(paste.updated_at),
        modified_at: paste.modified_at.map(format_timestamp),
        expires_at: paste.expires_at.map(format_timestamp),
        last_read_at: paste.last_read_at.map(format_timestamp),
        read_count: paste.read_count,
        read_limit: paste.read_limit,
        attachment_count,
        attachment_only_filename: if paste.content.trim().is_empty() {
            paste
                .attachments
                .first()
                .map(|attachment| attachment.filename.clone())
        } else {
            None
        },
        size_bytes: paste.size_bytes,
        attachments,
    }
}

pub(crate) fn resource(
    request: &HttpRequest,
    principal: &Principal,
    paste: Paste,
    grant_token: Option<&str>,
) -> crate::pastes::DomainResult<PasteResource> {
    let body = if paste.content_kind == "markdown" {
        let rendered = render_markdown(&paste.content).map_err(|error| {
            crate::pastes::DomainError::internal(format!(
                "Stored Markdown for paste {} is invalid: {error}",
                paste.id
            ))
        })?;
        BodyOutput::Markdown {
            content: paste.content.clone(),
            rendered_html: rendered.html,
            plain_text: rendered.plain_text,
        }
    } else {
        BodyOutput::Text {
            content: paste.content.clone(),
            language: paste.language.clone(),
        }
    };
    Ok(PasteResource {
        metadata: metadata_resource(request, principal, paste, grant_token),
        body,
    })
}

pub(crate) fn summary(
    request: &HttpRequest,
    principal: &Principal,
    paste: Paste,
    administrative: bool,
) -> PasteSummary {
    let own =
        administrative || principal.can("paste:manage") || principal.user_id() == paste.owner_id;
    let excerpt = (own || paste.read_limit.is_none()).then(|| {
        if paste.content_kind == "markdown" {
            render_markdown(&paste.content)
                .map(|value| value.plain_text)
                .unwrap_or_else(|_| paste.content.clone())
        } else {
            paste.content.clone()
        }
    });
    PasteSummary {
        id: paste.id.clone(),
        url: absolute(request, &format!("/pastes/{}", paste.id)),
        title: paste.title,
        format: paste.content_kind.clone(),
        language: (paste.content_kind == "text").then_some(paste.language),
        visibility: paste.visibility,
        owner_id: own.then_some(paste.owner_id).flatten(),
        owner_username: None,
        folder_id: own.then_some(paste.folder_id).flatten(),
        created_at: format_timestamp(paste.created_at),
        updated_at: format_timestamp(paste.updated_at),
        modified_at: paste.modified_at.map(format_timestamp),
        expires_at: paste.expires_at.map(format_timestamp),
        last_read_at: paste.last_read_at.map(format_timestamp),
        read_count: paste.read_count,
        read_limit: paste.read_limit,
        attachment_count: paste.attachment_count,
        attachment_only_filename: paste.attachment_only_filename,
        size_bytes: paste.size_bytes,
        excerpt,
    }
}

fn attachment_resource(
    request: &HttpRequest,
    paste_id: &str,
    attachment: &Attachment,
    grant_token: Option<&str>,
) -> AttachmentResource {
    let path = format!("/api/v1/pastes/{paste_id}/attachments/{}", attachment.id);
    let url = match grant_token {
        Some(token) => format!("{}?read_token={}", absolute(request, &path), token),
        None => absolute(request, &path),
    };
    AttachmentResource {
        id: attachment.id,
        filename: attachment.filename.clone(),
        size_bytes: attachment.size_bytes,
        url,
    }
}

pub(crate) fn etag(paste: &Paste) -> String {
    etag_revision(&paste.id, paste.revision)
}

pub(crate) fn etag_revision(paste_id: &str, revision: i64) -> String {
    format!("\"paste-{paste_id}-{revision}\"")
}

pub(crate) fn absolute(_request: &HttpRequest, path: &str) -> String {
    if let Some(base) = ARGS.public_url.as_ref() {
        return base.join(path.trim_start_matches('/')).map_or_else(
            |_| format!("{}{}", base.as_str().trim_end_matches('/'), path),
            |url| url.to_string(),
        );
    }
    path.to_string()
}

pub(crate) fn format_timestamp(timestamp: i64) -> String {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

pub(crate) fn parse_timestamp(value: &str) -> Result<i64, String> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map(OffsetDateTime::unix_timestamp)
        .map_err(|_| "Timestamp must be RFC 3339".into())
}

pub(crate) fn normalize_language(value: &str) -> Result<String, String> {
    let value = value.trim().to_ascii_lowercase();
    let normalized = match value.as_str() {
        "text" | "txt" | "none" => "plaintext",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "sh" | "shell" | "zsh" => "bash",
        "cs" => "csharp",
        "md" => "markdown",
        "html" | "htm" => "html",
        "yml" => "yaml",
        "c++" => "cpp",
        "c#" => "csharp",
        _ => value.as_str(),
    };
    if normalized.is_empty() || normalized.len() > 64 {
        Err("Language identifier is invalid".into())
    } else {
        Ok(normalized.to_string())
    }
}

fn detect_language(content: &str) -> &'static str {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "plaintext";
    }
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return "json";
    }
    if trimmed.starts_with("<!DOCTYPE html") || trimmed.starts_with("<html") {
        return "html";
    }
    if trimmed.starts_with("#!/bin/")
        || trimmed.starts_with("#!/usr/bin/env sh")
        || trimmed.starts_with("#!/usr/bin/env bash")
    {
        return "bash";
    }
    if ["function ", "const ", "let ", "console.", "=>", "import "]
        .iter()
        .any(|needle| trimmed.contains(needle))
    {
        return "javascript";
    }
    if ["fn main", "pub fn ", "impl ", "use std::", "let mut "]
        .iter()
        .any(|needle| trimmed.contains(needle))
    {
        return "rust";
    }
    if ["def ", "from ", "import ", "if __name__"]
        .iter()
        .any(|needle| trimmed.contains(needle))
        && trimmed.lines().any(|line| line.trim_end().ends_with(':'))
    {
        return "python";
    }
    "plaintext"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn create_fields_may_be_omitted_but_not_null() {
        assert!(serde_json::from_value::<CreatePasteRequest>(json!({})).is_ok());
        for field in [
            "title",
            "body",
            "visibility",
            "folder_id",
            "expires_at",
            "expires_in",
            "read_limit",
        ] {
            assert!(
                serde_json::from_value::<CreatePasteRequest>(json!({(field): null})).is_err(),
                "create field {field} accepted null"
            );
        }
    }

    #[test]
    fn update_nulls_are_reserved_for_clearable_fields() {
        for field in ["title", "body", "visibility"] {
            assert!(
                serde_json::from_value::<UpdatePasteRequest>(json!({(field): null})).is_err(),
                "update field {field} accepted null"
            );
        }
        let update: UpdatePasteRequest = serde_json::from_value(json!({
            "folder_id": null,
            "expires_at": null,
            "read_limit": null
        }))
        .unwrap();
        assert_eq!(update.folder_id, Some(None));
        assert_eq!(update.expires_at, Some(None));
        assert_eq!(update.read_limit, Some(None));
    }

    #[test]
    fn empty_create_is_an_empty_text_paste() {
        let input = serde_json::from_value::<CreatePasteRequest>(json!({}))
            .unwrap()
            .into_input(1_700_000_000)
            .unwrap();
        assert_eq!(input.content.as_deref(), Some(""));
        assert_eq!(input.content_kind.as_deref(), Some("text"));
    }

    #[test]
    fn pagination_arithmetic_is_bounded() {
        assert_eq!(total_pages(0, 30), 0);
        assert_eq!(total_pages(31, 30), 2);
        assert_eq!(total_pages(i64::MAX, 1), u32::MAX);
        assert_eq!(total_pages(1, 0), 0);
    }
}
