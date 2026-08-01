use super::*;
use crate::services::{document_to_html, html_to_document, Attachment, Paste};
use serde::{Deserialize, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(tag = "format", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BodyInput {
    Text {
        content: String,
        #[serde(default)]
        language: Option<String>,
    },
    RichText {
        content: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreatePasteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<BodyInput>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub folder_id: Option<i64>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
    #[serde(default)]
    pub read_limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdatePasteRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<BodyInput>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(default)]
    pub folder_id: Option<Option<i64>>,
    #[serde(default)]
    pub expires_at: Option<Option<String>>,
    #[serde(default)]
    pub read_limit: Option<Option<i64>>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "format", rename_all = "snake_case")]
pub(crate) enum BodyOutput {
    Text { content: String, language: String },
    RichText { content: String, plain_text: String },
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct AttachmentResource {
    pub id: i64,
    pub filename: String,
    pub size_bytes: i64,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PasteResource {
    pub id: String,
    pub url: String,
    pub api_url: String,
    pub read_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<BodyOutput>,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub last_read_at: Option<String>,
    pub read_count: i64,
    pub read_limit: Option<i64>,
    pub attachment_count: i64,
    pub size_bytes: i64,
    pub attachments: Vec<AttachmentResource>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PasteSummary {
    pub id: String,
    pub url: String,
    pub title: String,
    pub format: String,
    pub language: Option<String>,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub last_read_at: Option<String>,
    pub read_count: i64,
    pub read_limit: Option<i64>,
    pub attachment_count: i64,
    pub size_bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct Pagination {
    pub page: u32,
    pub page_size: u32,
    pub total_items: i64,
    pub total_pages: u32,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub(crate) struct PastePage {
    pub items: Vec<PasteSummary>,
    pub pagination: Pagination,
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
        let (content, document, content_kind, language) = body_into_internal(self.body)?;
        Ok(PasteInput {
            title: self.title,
            content: Some(content),
            document,
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
        let (content, document, content_kind, language) = match self.body {
            Some(body) => {
                let (content, document, format, language) = body_into_internal(Some(body))?;
                (Some(content), document, Some(format), Some(language))
            }
            None => (None, None, None, None),
        };
        let expires_at = self
            .expires_at
            .map(|value| value.map(|value| parse_timestamp(&value)).transpose())
            .transpose()?;
        Ok(PasteInput {
            title: self.title,
            content,
            document,
            content_kind,
            language,
            visibility: self.visibility,
            expires_at,
            read_limit: self.read_limit,
            folder_id: self.folder_id,
        })
    }
}

fn body_into_internal(
    body: Option<BodyInput>,
) -> Result<(String, Option<serde_json::Value>, String, String), String> {
    match body.unwrap_or(BodyInput::Text {
        content: String::new(),
        language: Some("auto".into()),
    }) {
        BodyInput::Text { content, language } => {
            let mut language = normalize_language(language.as_deref().unwrap_or("auto"))?;
            if language == "auto" {
                language = detect_language(&content).to_string();
            }
            Ok((content, None, "text".into(), language))
        }
        BodyInput::RichText { content } => {
            let document = html_to_document(&content)?;
            Ok((
                String::new(),
                Some(document),
                "rich_text".into(),
                "plaintext".into(),
            ))
        }
    }
}

pub(crate) fn resource(
    request: &HttpRequest,
    principal: &Principal,
    paste: Paste,
    include_body: bool,
    grant_token: Option<&str>,
) -> PasteResource {
    let own = principal.can("paste:manage") || principal.user_id() == paste.owner_id;
    let body = include_body.then(|| {
        if paste.content_kind == "rich_text" {
            BodyOutput::RichText {
                content: paste
                    .document
                    .as_ref()
                    .map(document_to_html)
                    .unwrap_or_default(),
                plain_text: paste.content.clone(),
            }
        } else {
            BodyOutput::Text {
                content: paste.content.clone(),
                language: paste.language.clone(),
            }
        }
    });
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
    PasteResource {
        id: paste.id.clone(),
        url: absolute(request, &format!("/pastes/{}", paste.id)),
        api_url: absolute(request, &format!("/api/v1/pastes/{}", paste.id)),
        read_url: absolute(request, &format!("/api/v1/pastes/{}/reads", paste.id)),
        archive_url,
        source_url: own.then(|| absolute(request, &format!("/api/v1/pastes/{}/source", paste.id))),
        title: paste.title,
        body,
        format: paste.content_kind.clone(),
        language: (paste.content_kind == "text").then_some(paste.language),
        visibility: paste.visibility,
        owner_id: own.then_some(paste.owner_id).flatten(),
        folder_id: own.then_some(paste.folder_id).flatten(),
        created_at: format_timestamp(paste.created_at),
        updated_at: format_timestamp(paste.updated_at),
        expires_at: paste.expires_at.map(format_timestamp),
        last_read_at: paste.last_read_at.map(format_timestamp),
        read_count: paste.read_count,
        read_limit: paste.read_limit,
        attachment_count,
        size_bytes: paste.size_bytes,
        attachments,
    }
}

pub(crate) fn summary(
    request: &HttpRequest,
    principal: &Principal,
    paste: Paste,
    administrative: bool,
) -> PasteSummary {
    let own =
        administrative || principal.can("paste:manage") || principal.user_id() == paste.owner_id;
    let excerpt = (own || paste.read_limit.is_none()).then(|| paste.content.clone());
    PasteSummary {
        id: paste.id.clone(),
        url: absolute(request, &format!("/pastes/{}", paste.id)),
        title: paste.title,
        format: paste.content_kind.clone(),
        language: (paste.content_kind == "text").then_some(paste.language),
        visibility: paste.visibility,
        owner_id: own.then_some(paste.owner_id).flatten(),
        folder_id: own.then_some(paste.folder_id).flatten(),
        created_at: format_timestamp(paste.created_at),
        updated_at: format_timestamp(paste.updated_at),
        expires_at: paste.expires_at.map(format_timestamp),
        last_read_at: paste.last_read_at.map(format_timestamp),
        read_count: paste.read_count,
        read_limit: paste.read_limit,
        attachment_count: paste.attachment_count,
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
    format!("\"paste-{}-{}\"", paste.id, paste.revision)
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
