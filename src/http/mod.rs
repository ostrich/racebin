use crate::account::{self as accounts, api_keys};
use crate::args::ARGS;
use crate::services::{text_to_document, PasteInput, PasteQuery, PasteService, Principal};
use actix_files::NamedFile;
use actix_multipart::Multipart;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::http::{header, StatusCode};
use actix_web::{delete, get, patch, post, web, HttpRequest, HttpResponse, Responder};
use auth::{principal, require_admin, require_auth, require_mutation};
use errors::{domain_error, error, internal};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use std::path::{Component, Path, PathBuf};
use tokio::io::AsyncWriteExt;
use zip::write::SimpleFileOptions;

mod account;
mod admin;
mod assets;
pub(crate) mod attachments;
mod auth;
mod contract;
mod cookies;
mod dto;
mod errors;
mod folders;
mod keys;
mod meta;
mod paste_payload;
mod pastes;

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct EnabledInput {
    enabled: bool,
}

pub fn configure(config: &mut web::ServiceConfig) {
    config
        .app_data(
            web::JsonConfig::default()
                .limit(2 * 1024 * 1024)
                .error_handler(|parse_error, _| {
                    actix_web::error::InternalError::from_response(
                        parse_error,
                        error(
                            StatusCode::BAD_REQUEST,
                            "invalid_json",
                            "Request body is not valid for this endpoint",
                        ),
                    )
                    .into()
                }),
        )
        .app_data(web::QueryConfig::default().error_handler(|parse_error, _| {
            actix_web::error::InternalError::from_response(
                parse_error,
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_query",
                    "Query parameters are not valid for this endpoint",
                ),
            )
            .into()
        }))
        .service(web::resource("/healthz").route(web::get().to(meta::health)))
        .service(web::resource("/readyz").route(web::get().to(meta::ready)))
        .service(
            web::scope("/api/v1")
                .configure(meta::configure)
                .configure(account::configure)
                .configure(folders::configure)
                .configure(pastes::configure)
                .configure(attachments::configure)
                .configure(keys::configure)
                .configure(admin::configure),
        )
        .service(web::resource("/assets/{path:.*}").route(web::get().to(assets::asset)))
        .default_service(web::route().to(assets::spa));
}
