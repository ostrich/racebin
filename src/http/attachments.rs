use super::*;

#[derive(Default)]
struct UploadCleanup {
    paths: Vec<PathBuf>,
}

impl Drop for UploadCleanup {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub(crate) fn attachment_path(
    data_dir: &Path,
    paste_id: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let safe_component = |value: &str| {
        let mut components = Path::new(value).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    if !safe_component(paste_id) || !safe_component(name) || name.starts_with('.') {
        return Err("Unsafe attachment metadata".to_string());
    }
    Ok(data_dir.join("attachments").join(paste_id).join(name))
}

pub(crate) fn sanitize_upload_filename(value: &str) -> String {
    let basename = value.rsplit(['/', '\\']).next().unwrap_or("");
    let mut sanitized = String::with_capacity(basename.len().min(255));
    for character in basename.chars() {
        let character = if character.is_control() || r#"<>:"/\|?*"#.contains(character) {
            '_'
        } else {
            character
        };
        if sanitized.len() + character.len_utf8() > 255 {
            break;
        }
        sanitized.push(character);
    }
    sanitized
        .trim_matches(|character: char| character == '.' || character.is_whitespace())
        .to_string()
}

pub(super) fn configure(config: &mut web::ServiceConfig) {
    config
        .service(upload_attachments)
        .service(get_attachment)
        .service(delete_attachment)
        .service(get_archive)
        .service(get_qr);
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
struct ReadGrantQuery {
    /// Short-lived token returned by the final permitted read.
    read_token: Option<String>,
}

#[derive(utoipa::ToSchema)]
#[allow(dead_code)]
struct AttachmentUploadRequest {
    #[schema(value_type = Vec<Value>, min_items = 1)]
    file: Vec<String>,
}

#[utoipa::path(
    post, path = "/pastes/{paste_id}/attachments", tag = "attachments",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("If-Match" = String, Header, description = "Current ETag or *"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    request_body(content = AttachmentUploadRequest, content_type = "multipart/form-data",
        description = "One or more repeated file parts"),
    responses(
        (status = 201, description = "Attachments uploaded", body = crate::http::contract::AttachmentUploadResponse,
            headers(("ETag" = String, description = "New paste entity tag"))),
        (status = 400, description = "Malformed or empty upload", body = crate::http::errors::ProblemDetails),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Uploads disabled or insufficient permission", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste not found", body = crate::http::errors::ProblemDetails),
        (status = 409, description = "Attachment conflict", body = crate::http::errors::ProblemDetails),
        (status = 412, description = "ETag does not match", body = crate::http::errors::ProblemDetails),
        (status = 413, description = "Attachment limit exceeded", body = crate::http::errors::ProblemDetails),
        (status = 422, description = "Invalid multipart field", body = crate::http::errors::ProblemDetails),
        (status = 428, description = "If-Match is required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[post("/pastes/{paste_id}/attachments")]
pub(crate) async fn upload_attachments(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
    mut payload: Multipart,
) -> HttpResponse {
    if !ARGS.attachments_enabled {
        return error(
            StatusCode::FORBIDDEN,
            "uploads_disabled",
            "Attachments are disabled",
        );
    }
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.ensure_can_update(&value, &paste_id).await {
        Ok(paste) => paste,
        Err(value) => return domain_error(value),
    };
    let expected_revision = match super::pastes::require_match(&req, &paste) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let directory = services
        .storage
        .data_dir
        .join("attachments")
        .join(&paste.id);
    if let Err(e) = tokio::fs::create_dir_all(&directory).await {
        return internal(e.to_string());
    }
    let Some(limit) = ARGS.max_attachment_size_mb.checked_mul(1024 * 1024) else {
        return internal("Configured upload size is too large");
    };
    let mut staged: Vec<(PathBuf, PathBuf, String, String, i64)> = Vec::new();
    let mut cleanup = UploadCleanup::default();
    let mut total_size = 0usize;
    loop {
        let mut field = match payload.try_next().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                return error(StatusCode::BAD_REQUEST, "invalid_upload", e.to_string());
            }
        };
        if staged.len() >= 32 {
            return error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_many_attachments",
                "A paste may contain at most 32 attachments",
            );
        }
        if field.name() != Some("file") {
            return error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_attachment",
                "File parts must use the field name file",
            );
        }
        let Some(filename) = field
            .content_disposition()
            .and_then(|value| value.get_filename())
            .map(sanitize_upload_filename)
            .filter(|value| !value.is_empty())
        else {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_upload",
                "Every multipart field must contain a filename",
            );
        };
        let temporary = directory.join(format!(".upload-{}", uuid::Uuid::new_v4()));
        let mut output = match tokio::fs::File::create(&temporary).await {
            Ok(file) => file,
            Err(e) => return internal(e.to_string()),
        };
        cleanup.paths.push(temporary.clone());
        let mut size = 0usize;
        while let Some(chunk) = field.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(e) => {
                    let _ = std::fs::remove_file(&temporary);
                    return error(StatusCode::BAD_REQUEST, "invalid_upload", e.to_string());
                }
            };
            size += chunk.len();
            total_size = total_size.saturating_add(chunk.len());
            if size > limit || total_size > limit {
                let _ = std::fs::remove_file(&temporary);
                return error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "attachment_too_large",
                    "Upload exceeds configured size limit",
                );
            }
            if let Err(e) = output.write_all(&chunk).await {
                let _ = std::fs::remove_file(&temporary);
                return internal(e.to_string());
            }
        }
        let storage_key = uuid::Uuid::new_v4().simple().to_string();
        let destination = match attachment_path(&services.storage.data_dir, &paste.id, &storage_key)
        {
            Ok(path) => path,
            Err(e) => {
                let _ = std::fs::remove_file(&temporary);
                return error(StatusCode::BAD_REQUEST, "invalid_attachment", e);
            }
        };
        if tokio::fs::try_exists(&destination).await.unwrap_or(true) {
            let _ = std::fs::remove_file(&temporary);
            return error(
                StatusCode::CONFLICT,
                "attachment_exists",
                format!("{filename} already exists"),
            );
        }
        staged.push((temporary, destination, filename, storage_key, size as i64));
    }
    let mut promoted = Vec::new();
    for (temporary, destination, _, _, _) in &staged {
        if let Err(e) = tokio::fs::rename(temporary, destination).await {
            let _ = std::fs::remove_file(temporary);
            for path in promoted {
                let _ = std::fs::remove_file(path);
            }
            for (path, _, _, _, _) in &staged {
                let _ = std::fs::remove_file(path);
            }
            return internal(e.to_string());
        }
        cleanup.paths.push(destination.clone());
        promoted.push(destination.clone());
    }
    let inputs = staged
        .iter()
        .map(|(_, _, name, storage_key, size)| (name.clone(), storage_key.clone(), *size))
        .collect::<Vec<_>>();
    let attachments = match services
        .add_attachments(&value, &paste_id, &inputs, expected_revision)
        .await
    {
        Ok(attachments) => attachments,
        Err(e) => {
            for path in promoted {
                let _ = std::fs::remove_file(path);
            }
            return domain_error(e);
        }
    };
    if attachments.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_upload",
            "No valid attachments were supplied",
        );
    }
    cleanup.paths.clear();
    let current = match services.ensure_can_update(&value, &paste_id).await {
        Ok(current) => current,
        Err(value) => return domain_error(value),
    };
    HttpResponse::Created()
        .insert_header((header::ETAG, super::dto::etag(&current)))
        .json(contract::AttachmentUploadResponse { items: attachments })
}

#[utoipa::path(
    get, path = "/pastes/{paste_id}/attachments/{attachment_id}", tag = "attachments",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("attachment_id" = i64, Path, description = "Attachment ID"), ReadGrantQuery),
    responses(
        (status = 200, description = "Attachment bytes", content_type = "application/octet-stream",
            headers(("Content-Disposition" = String, description = "Attachment filename"))),
        (status = 404, description = "Attachment not found, not visible, or grant invalid", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Attachment data unavailable", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/pastes/{paste_id}/attachments/{attachment_id}")]
pub(crate) async fn get_attachment(
    req: HttpRequest,
    services: web::Data<PasteService>,
    path: web::Path<(String, i64)>,
    query: web::Query<ReadGrantQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (paste_id, attachment_id) = path.into_inner();
    let paste =
        match paste_for_download(&services, &value, &paste_id, query.read_token.as_deref()).await {
            Ok(Some(paste)) => paste,
            Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Attachment not found"),
            Err(e) => return internal(e),
        };
    let Some(attachment) = paste
        .attachments
        .iter()
        .find(|attachment| attachment.id == attachment_id)
    else {
        return error(StatusCode::NOT_FOUND, "not_found", "Attachment not found");
    };
    let path = match attachment_path(
        &services.storage.data_dir,
        &paste.id,
        &attachment.storage_key,
    ) {
        Ok(path) => path,
        Err(e) => return internal(e),
    };
    let content_type = mime_guess::from_path(&attachment.filename).first_or_octet_stream();
    let named = match NamedFile::open(path) {
        Ok(named) => named,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return error(
                StatusCode::NOT_FOUND,
                "not_found",
                "Attachment data is missing",
            )
        }
        Err(e) => return internal(e.to_string()),
    };
    named
        .set_content_type(content_type)
        .set_content_disposition(header::ContentDisposition {
            disposition: header::DispositionType::Attachment,
            parameters: vec![header::DispositionParam::Filename(
                attachment.filename.clone(),
            )],
        })
        .into_response(&req)
}

#[utoipa::path(
    delete, path = "/pastes/{paste_id}/attachments/{attachment_id}", tag = "attachments",
    params(("paste_id" = String, Path, description = "Paste ID"),
        ("attachment_id" = i64, Path, description = "Attachment ID"),
        ("If-Match" = String, Header, description = "Current ETag or *"),
        ("X-CSRF-Token" = Option<String>, Header, description = "Required for session-cookie mutations")),
    responses(
        (status = 204, description = "Attachment deleted",
            headers(("ETag" = String, description = "New paste entity tag"))),
        (status = 401, description = "Authentication required", body = crate::http::errors::ProblemDetails),
        (status = 403, description = "Not permitted to update this paste", body = crate::http::errors::ProblemDetails),
        (status = 404, description = "Paste or attachment not found", body = crate::http::errors::ProblemDetails),
        (status = 412, description = "ETag does not match", body = crate::http::errors::ProblemDetails),
        (status = 428, description = "If-Match is required", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Internal error", body = crate::http::errors::ProblemDetails)
    ),
    security(("bearerAuth" = []), ("sessionCookie" = []))
)]
#[delete("/pastes/{paste_id}/attachments/{attachment_id}")]
pub(crate) async fn delete_attachment(
    req: HttpRequest,
    services: web::Data<PasteService>,
    path: web::Path<(String, i64)>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (paste_id, attachment_id) = path.into_inner();
    let current = match services.ensure_can_update(&value, &paste_id).await {
        Ok(current) => current,
        Err(value) => return domain_error(value),
    };
    let expected_revision = match super::pastes::require_match(&req, &current) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services
        .delete_attachment(&value, &paste_id, attachment_id, expected_revision)
        .await
    {
        Ok(Some(revision)) => HttpResponse::NoContent()
            .insert_header((header::ETAG, dto::etag_revision(&paste_id, revision)))
            .finish(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "Attachment not found"),
        Err(value) => domain_error(value),
    }
}

#[utoipa::path(
    get, path = "/pastes/{paste_id}/archive", tag = "attachments",
    params(("paste_id" = String, Path, description = "Paste ID"), ReadGrantQuery),
    responses(
        (status = 200, description = "ZIP containing paste text and attachments", content_type = "application/zip",
            headers(("Content-Disposition" = String, description = "Archive filename"))),
        (status = 404, description = "Paste not found, not visible, or grant invalid", body = crate::http::errors::ProblemDetails),
        (status = 413, description = "Archive input exceeds 64 MiB", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "Archive could not be generated", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/pastes/{paste_id}/archive")]
pub(crate) async fn get_archive(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
    query: web::Query<ReadGrantQuery>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste =
        match paste_for_download(&services, &value, &paste_id, query.read_token.as_deref()).await {
            Ok(Some(paste)) => paste,
            Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
            Err(e) => return internal(e),
        };
    const MAX_ARCHIVE_INPUT: u64 = 64 * 1024 * 1024;
    let archive_input = paste.content.len() as u64
        + paste
            .attachments
            .iter()
            .map(|attachment| attachment.size_bytes.max(0) as u64)
            .sum::<u64>();
    if archive_input > MAX_ARCHIVE_INPUT {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "archive_too_large",
            "Archive input exceeds 64 MiB",
        );
    }
    let data_dir = services.storage.data_dir.clone();
    let archive_id = paste.id.clone();
    let archive = web::block(move || {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        if !paste.content.is_empty() {
            zip.start_file("paste.txt", options)
                .map_err(|e| e.to_string())?;
            zip.write_all(paste.content.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        for attachment in &paste.attachments {
            let path = attachment_path(&data_dir, &paste.id, &attachment.storage_key)?;
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            zip.start_file(&attachment.filename, options)
                .map_err(|e| e.to_string())?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.finish()
            .map(|cursor| cursor.into_inner())
            .map_err(|e| e.to_string())
    })
    .await;
    match archive {
        Ok(Ok(bytes)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "application/zip"))
            .insert_header((
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{archive_id}.zip\""),
            ))
            .body(bytes),
        Ok(Err(e)) => internal(e),
        Err(e) => internal(e.to_string()),
    }
}

async fn paste_for_download(
    services: &PasteService,
    principal: &Principal,
    paste_id: &str,
    read_token: Option<&str>,
) -> crate::services::DomainResult<Option<crate::services::Paste>> {
    if let Some(paste) = services.get_paste(principal, paste_id).await? {
        let owner = principal.is_admin() || principal.user_id() == paste.owner_id;
        if paste.read_limit.is_none() || owner {
            return Ok(Some(paste));
        }
    }
    match read_token {
        Some(token) => services.get_paste_with_grant(paste_id, token).await,
        None => Ok(None),
    }
}

#[utoipa::path(
    get, path = "/pastes/{paste_id}/qr", tag = "attachments",
    params(("paste_id" = String, Path, description = "Paste ID")),
    responses(
        (status = 200, description = "PNG QR code for the public paste URL", content_type = "image/png"),
        (status = 404, description = "Paste not found, not visible, or QR disabled", body = crate::http::errors::ProblemDetails),
        (status = 500, description = "QR generation failed", body = crate::http::errors::ProblemDetails)
    ),
    security((), ("bearerAuth" = []), ("sessionCookie" = []))
)]
#[get("/pastes/{paste_id}/qr")]
pub(crate) async fn get_qr(
    req: HttpRequest,
    services: web::Data<PasteService>,
    paste_id: web::Path<String>,
) -> HttpResponse {
    if !ARGS.qr_codes {
        return error(StatusCode::NOT_FOUND, "not_found", "QR codes are disabled");
    }
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_paste(&value, &paste_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return domain_error(e),
    }
    let origin = ARGS
        .public_url
        .as_ref()
        .expect("validated at startup")
        .as_str()
        .trim_end_matches('/');
    match qrcode_generator::to_png_to_vec(
        format!("{origin}/pastes/{paste_id}"),
        qrcode_generator::QrCodeEcc::Low,
        512,
    ) {
        Ok(bytes) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "image/png"))
            .body(bytes),
        Err(e) => internal(e.to_string()),
    }
}
