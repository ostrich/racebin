pub use crate::domain_error::{DomainError, DomainResult, ErrorKind};
mod attachments;
mod folders;
mod model;
mod rich_text;
mod rich_text_import;
mod service;
mod validation;

pub use model::{
    Attachment, Folder, FolderOverview, Page, Paste, PasteInput, PasteQuery, PasteRead,
};
pub use rich_text::{document_to_html, document_to_text, text_to_document, validate_document};
pub use rich_text_import::html_to_document;
pub use service::{PasteService, Principal};
pub(crate) use validation::validate_paste_query;
