pub use crate::domain_error::{DomainError, DomainResult, ErrorKind};
mod api_keys;
mod attachments;
mod folders;
mod html_import;
mod legacy_document;
mod markdown;
mod model;
mod service;
mod validation;

pub use html_import::html_to_document;
pub use legacy_document::document_to_markdown;
pub use markdown::{render_markdown, text_to_markdown, MarkdownOutput};
pub use model::{
    Attachment, Folder, FolderOverview, Page, Paste, PasteInput, PasteQuery, PasteRead,
};
pub use service::{PasteService, Principal};
pub(crate) use validation::validate_paste_query;
