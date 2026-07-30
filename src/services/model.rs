use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Paste {
    pub id: String,
    pub owner_id: Option<i64>,
    pub title: String,
    pub content: String,
    pub content_kind: String,
    pub language: String,
    pub visibility: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub last_read_at: Option<i64>,
    pub read_count: i64,
    pub read_limit: Option<i64>,
    #[sqlx(skip)]
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Attachment {
    pub id: i64,
    #[serde(skip)]
    pub sort_order: i64,
    pub filename: String,
    #[serde(skip)]
    pub storage_key: String,
    pub size_bytes: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub content_kind: Option<String>,
    pub language: Option<String>,
    pub visibility: Option<String>,
    pub expires_at: Option<Option<i64>>,
    pub read_limit: Option<Option<i64>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub visibility: Option<String>,
    pub owner_id: Option<i64>,
    pub mine: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total_items: i64,
}
