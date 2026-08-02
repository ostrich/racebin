use super::pastes::{FlatCreateRequest, RawCreateQuery, StagedFile};
use super::*;
use crate::http::dto::CreatePasteRequest;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(super) async fn parse_non_multipart(
    content_type: &str,
    query: RawCreateQuery,
    mut payload: web::Payload,
) -> Result<CreatePasteRequest, HttpResponse> {
    let bytes = read_body(&mut payload, crate::limits::MAX_CONTENT_SIZE_BYTES).await?;
    match content_type {
        "application/json" => {
            if query != RawCreateQuery::default() {
                return Err(error(
                    StatusCode::BAD_REQUEST,
                    "invalid_query",
                    "JSON requests do not accept creation fields in the query string",
                ));
            }
            serde_json::from_slice(&bytes).map_err(|_| {
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_json",
                    "Request body is not valid JSON",
                )
            })
        }
        "application/x-www-form-urlencoded" => serde_urlencoded::from_bytes::<FlatCreateRequest>(&bytes)
            .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_form", "Request body is not valid form data"))?
            .structured()
            .map_err(|message| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message)),
        "text/plain" | "text/markdown" | "text/html" => {
            let content = String::from_utf8(bytes.to_vec())
                .map_err(|_| error(StatusCode::BAD_REQUEST, "invalid_text", "Text input must be UTF-8"))?;
            let mut query = FlatCreateRequest::from(query);
            query.content = Some(content);
            query.format = Some(if content_type == "text/html" {
                "rich_text"
            } else {
                "text"
            }.into());
            if content_type == "text/markdown" {
                query.language = Some("markdown".into());
            }
            query.structured().map_err(|message| {
                error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message)
            })
        }
        _ => Err(error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Supported content types are JSON, plain text, Markdown, HTML, form data, and multipart form data",
        )),
    }
}

pub(super) async fn parse_multipart(
    req: &HttpRequest,
    payload: web::Payload,
    services: &PasteService,
) -> Result<(CreatePasteRequest, Vec<StagedFile>), HttpResponse> {
    if !ARGS.attachments_enabled {
        return Err(error(
            StatusCode::FORBIDDEN,
            "uploads_disabled",
            "Attachments are disabled",
        ));
    }
    let mut multipart = Multipart::new(req.headers(), payload);
    let mut values = HashMap::<String, String>::new();
    let mut files = Vec::new();
    let mut total_size = 0usize;
    let limit = ARGS
        .max_attachment_size_mb
        .checked_mul(1024 * 1024)
        .ok_or_else(|| internal("Configured upload size is too large"))?;
    let staging = services
        .storage
        .data_dir
        .join("attachments")
        .join(".staging");
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(|error| internal(error.to_string()))?;
    while let Some(mut field) = multipart.try_next().await.map_err(|reason| {
        error(
            StatusCode::BAD_REQUEST,
            "invalid_multipart",
            reason.to_string(),
        )
    })? {
        let name = field.name().unwrap_or("").to_string();
        let filename = field
            .content_disposition()
            .and_then(|value| value.get_filename());
        if let Some(filename) = filename {
            if name != "file" {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_attachment",
                    "File parts must use the field name file",
                ));
            }
            if files.len() >= crate::limits::MAX_ATTACHMENTS_PER_PASTE {
                return Err(error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "too_many_attachments",
                    "A paste may contain at most 32 attachments",
                ));
            }
            let filename = super::attachments::sanitize_upload_filename(filename);
            if filename.is_empty() {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_attachment",
                    "Attachment filename is invalid",
                ));
            }
            let temporary = staging.join(format!("upload-{}", uuid::Uuid::new_v4().simple()));
            let mut output = tokio::fs::File::create(&temporary)
                .await
                .map_err(|error| internal(error.to_string()))?;
            let mut size = 0usize;
            let mut digest = Sha256::new();
            while let Some(chunk) = field.next().await {
                let chunk = chunk.map_err(|reason| {
                    error(
                        StatusCode::BAD_REQUEST,
                        "invalid_multipart",
                        reason.to_string(),
                    )
                })?;
                size = size.saturating_add(chunk.len());
                total_size = total_size.saturating_add(chunk.len());
                if size > limit || total_size > limit {
                    let _ = tokio::fs::remove_file(&temporary).await;
                    return Err(error(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "attachment_too_large",
                        "Upload exceeds configured size limit",
                    ));
                }
                digest.update(&chunk);
                output
                    .write_all(&chunk)
                    .await
                    .map_err(|error| internal(error.to_string()))?;
            }
            files.push(StagedFile {
                temporary,
                filename,
                storage_key: uuid::Uuid::new_v4().simple().to_string(),
                size_bytes: size as i64,
                digest: format!("{:x}", digest.finalize()),
            });
        } else {
            if !matches!(
                name.as_str(),
                "title"
                    | "format"
                    | "content"
                    | "language"
                    | "visibility"
                    | "folder_id"
                    | "expires_at"
                    | "expires_in"
                    | "read_limit"
            ) {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "unknown_field",
                    format!("Unknown multipart field: {name}"),
                ));
            }
            if values.contains_key(&name) {
                return Err(error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "duplicate_field",
                    format!("Multipart field appears more than once: {name}"),
                ));
            }
            let bytes = read_field(&mut field, crate::limits::MAX_CONTENT_SIZE_BYTES).await?;
            let value = String::from_utf8(bytes).map_err(|_| {
                error(
                    StatusCode::BAD_REQUEST,
                    "invalid_text",
                    "Multipart text fields must be UTF-8",
                )
            })?;
            values.insert(name, value);
        }
    }
    let mut take_numeric = |name: &str| -> Result<Option<i64>, HttpResponse> {
        values
            .remove(name)
            .map(|value| {
                value.parse::<i64>().map_err(|_| {
                    error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "invalid_field",
                        format!("{name} must be an integer"),
                    )
                })
            })
            .transpose()
    };
    let folder_id = take_numeric("folder_id")?;
    let expires_in = take_numeric("expires_in")?;
    let read_limit = take_numeric("read_limit")?;
    let flat = FlatCreateRequest {
        title: values.remove("title"),
        format: values.remove("format"),
        content: values.remove("content"),
        language: values.remove("language"),
        visibility: values.remove("visibility"),
        folder_id,
        expires_at: values.remove("expires_at"),
        expires_in,
        read_limit,
    };
    Ok((
        flat.structured()
            .map_err(|message| error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_paste", message))?,
        files,
    ))
}

pub(super) async fn promote_created_files(
    services: &PasteService,
    principal: &Principal,
    paste: &mut crate::services::Paste,
    staged: &mut [StagedFile],
) -> crate::services::DomainResult<()> {
    let directory = services
        .storage
        .data_dir
        .join("attachments")
        .join(&paste.id);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| crate::services::DomainError::internal(error.to_string()))?;
    let mut promoted = Vec::new();
    for file in staged.iter_mut() {
        let destination = super::attachments::attachment_path(
            &services.storage.data_dir,
            &paste.id,
            &file.storage_key,
        )
        .map_err(crate::services::DomainError::internal)?;
        if let Err(error) = tokio::fs::rename(&file.temporary, &destination).await {
            for path in promoted {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(crate::services::DomainError::internal(error.to_string()));
        }
        file.temporary = PathBuf::new();
        promoted.push(destination);
    }
    let inputs = staged
        .iter()
        .map(|file| {
            (
                file.filename.clone(),
                file.storage_key.clone(),
                file.size_bytes,
            )
        })
        .collect::<Vec<_>>();
    match services
        .add_attachments(principal, &paste.id, &inputs, None)
        .await
    {
        Ok(_) => {
            *paste = services.ensure_can_update(principal, &paste.id).await?;
            remove_unreferenced_attachment_files(services, paste).await;
            Ok(())
        }
        Err(message) => {
            for path in promoted {
                let _ = tokio::fs::remove_file(path).await;
            }
            Err(message)
        }
    }
}

async fn remove_unreferenced_attachment_files(
    services: &PasteService,
    paste: &crate::services::Paste,
) {
    let directory = services
        .storage
        .data_dir
        .join("attachments")
        .join(&paste.id);
    let referenced = paste
        .attachments
        .iter()
        .map(|attachment| attachment.storage_key.as_str())
        .collect::<std::collections::HashSet<_>>();
    let Ok(mut entries) = tokio::fs::read_dir(directory).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with('.') && !referenced.contains(name) {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }
}

pub(super) fn request_fingerprint(request: &CreatePasteRequest, files: &[StagedFile]) -> String {
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(request).unwrap_or_default());
    for file in files {
        digest.update(file.filename.as_bytes());
        digest.update(file.size_bytes.to_be_bytes());
        digest.update(file.digest.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

async fn read_body(payload: &mut web::Payload, limit: usize) -> Result<web::Bytes, HttpResponse> {
    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk
            .map_err(|reason| error(StatusCode::BAD_REQUEST, "invalid_body", reason.to_string()))?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Request body exceeds 2 MiB",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body.freeze())
}

async fn read_field(
    field: &mut actix_multipart::Field,
    limit: usize,
) -> Result<Vec<u8>, HttpResponse> {
    let mut body = Vec::new();
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|reason| {
            error(
                StatusCode::BAD_REQUEST,
                "invalid_multipart",
                reason.to_string(),
            )
        })?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "field_too_large",
                "Multipart text field exceeds 2 MiB",
            ));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
