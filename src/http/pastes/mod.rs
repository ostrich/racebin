use self::request_parser::{
    parse_multipart, parse_non_multipart, promote_created_files, request_fingerprint,
};
use super::*;
use crate::http::contract::{
    self, BodyInput, CreatePasteRequest, Pagination, PastePage, UpdatePasteRequest,
};

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(list_pastes)
        .service(create_paste)
        .service(convert_paste_content)
        .service(read_paste)
        .service(get_paste)
        .service(get_paste_raw)
        .service(get_paste_source)
        .service(update_paste)
        .service(delete_paste);
}

mod handlers;
mod payload;
mod query;
mod request_parser;

pub(crate) use handlers::*;
use payload::*;
pub(crate) use query::{
    ApiPasteQuery, ExpirationFilter, OwnerFilter, PasteSort, ReadLimitFilter, SortDirection,
};
