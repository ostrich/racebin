use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::any::AnyRow;
use sqlx::{FromRow, Row};

#[derive(Clone, Debug, Serialize)]
pub struct Paste {
    pub id: String,
    pub owner_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub title: String,
    pub content: String,
    pub document: Option<Value>,
    pub content_kind: String,
    pub language: String,
    pub visibility: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
    #[serde(skip)]
    pub consumed_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub last_read_at: Option<i64>,
    pub read_count: i64,
    pub read_limit: Option<i64>,
    pub attachment_count: i64,
    pub size_bytes: i64,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug)]
pub struct PasteRead {
    pub paste: Paste,
    pub grant_token: Option<String>,
    pub replayed: bool,
}

impl<'r> FromRow<'r, AnyRow> for Paste {
    fn from_row(row: &'r AnyRow) -> Result<Self, sqlx::Error> {
        let document_json: Option<String> = row.try_get("document_json")?;
        Ok(Self {
            id: row.try_get("id")?,
            owner_id: row.try_get("owner_id")?,
            folder_id: row.try_get("folder_id").unwrap_or(None),
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            document: document_json
                .map(|value| serde_json::from_str(&value))
                .transpose()
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            content_kind: row.try_get("content_kind")?,
            language: row.try_get("language")?,
            visibility: row.try_get("visibility")?,
            created_at: row.try_get("created_at")?,
            updated_at: row
                .try_get("updated_at")
                .unwrap_or_else(|_| row.try_get("created_at").unwrap_or(0)),
            revision: row.try_get("revision").unwrap_or(1),
            consumed_at: row.try_get("consumed_at").unwrap_or(None),
            expires_at: row.try_get("expires_at")?,
            last_read_at: row.try_get("last_read_at")?,
            read_count: row.try_get("read_count")?,
            read_limit: row.try_get("read_limit")?,
            attachment_count: row.try_get("attachment_count").unwrap_or(0),
            size_bytes: row.try_get("size_bytes").unwrap_or(0),
            attachments: Vec::new(),
        })
    }
}

#[derive(Clone, Debug, Serialize, FromRow, utoipa::ToSchema)]
pub struct Attachment {
    pub id: i64,
    #[serde(skip)]
    pub sort_order: i64,
    pub filename: String,
    #[serde(skip)]
    pub storage_key: String,
    pub size_bytes: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub document: Option<Value>,
    pub content_kind: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub expires_at: Option<Option<i64>>,
    pub read_limit: Option<Option<i64>>,
    pub folder_id: Option<Option<i64>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub visibility: Option<String>,
    pub owner_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub unfiled: Option<bool>,
    pub mine: Option<bool>,
    pub content_kind: Option<String>,
    pub language: Option<String>,
    pub has_attachments: Option<bool>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
    pub expiration: Option<String>,
    pub min_reads: Option<i64>,
    pub max_reads: Option<i64>,
    pub min_size_bytes: Option<i64>,
    pub max_size_bytes: Option<i64>,
    pub read_limit: Option<String>,
    pub sort: Option<String>,
    pub direction: Option<String>,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Folder {
    pub id: i64,
    #[serde(skip)]
    pub owner_id: i64,
    pub name: String,
    pub created_at: i64,
    pub paste_count: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FolderOverview {
    pub items: Vec<Folder>,
    pub total_count: i64,
    pub unfiled_count: i64,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total_items: i64,
}
