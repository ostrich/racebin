pub use crate::domain_error::{DomainError, DomainResult, ErrorKind};
mod api_keys;
mod attachments;
mod authorization;
mod folders;
mod model;
mod operations;
mod rich_text;
mod validation;

pub(crate) use api_keys::ApiKeyListOptions;
pub use authorization::{Permission, Principal, ADMIN_PERMISSIONS, OWNER_PERMISSIONS};
pub use model::{
    Attachment, Folder, FolderOverview, NewAttachment, Page, Paste, PasteInput, PasteQuery,
    PasteRead,
};
pub use operations::PasteService;
pub(crate) use rich_text::document_to_markdown;
pub use rich_text::{html_to_document, render_markdown, text_to_markdown, MarkdownOutput};
pub(crate) use validation::validate_paste_query;
