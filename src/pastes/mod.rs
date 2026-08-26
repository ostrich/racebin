pub use crate::domain_error::{DomainError, DomainResult, ErrorKind};
mod api_keys;
mod attachments;
mod folders;
mod model;
mod rich_text;
mod service;
mod validation;

pub(crate) use api_keys::ApiKeyListOptions;
pub use model::{
    Attachment, Folder, FolderOverview, NewAttachment, Page, Paste, PasteInput, PasteQuery,
    PasteRead,
};
pub(crate) use rich_text::document_to_markdown;
pub use rich_text::{html_to_document, render_markdown, text_to_markdown, MarkdownOutput};
pub use service::{PasteService, Permission, Principal, ADMIN_PERMISSIONS, OWNER_PERMISSIONS};
pub(crate) use validation::validate_paste_query;
