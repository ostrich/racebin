use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct Paste {
    pub id: i64,
    pub slug: String,
    pub owner_user_id: Option<i64>,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub syntax: String,
    pub access: String,
    pub created: i64,
    pub expiration: Option<i64>,
    pub last_read: Option<i64>,
    pub read_count: i64,
    pub burn_after_reads: i64,
    #[sqlx(skip)]
    pub files: Vec<PasteFile>,
}

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct PasteFile {
    pub id: i64,
    pub position: i64,
    pub role: String,
    pub name: String,
    #[serde(skip)]
    pub storage_name: String,
    pub size: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub kind: Option<String>,
    pub syntax: Option<String>,
    pub access: Option<String>,
    pub expiration: Option<Option<i64>>,
    pub burn_after_reads: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasteQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub search: Option<String>,
    pub access: Option<String>,
    pub owner_user_id: Option<i64>,
    pub mine: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub page_size: u32,
    pub total: i64,
}
