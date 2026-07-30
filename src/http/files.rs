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

pub(crate) fn attachment_path(data_dir: &Path, slug: &str, name: &str) -> Result<PathBuf, String> {
    let safe_component = |value: &str| {
        let mut components = Path::new(value).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    if !safe_component(slug) || !safe_component(name) || name.starts_with('.') {
        return Err("Unsafe attachment metadata".to_string());
    }
    Ok(data_dir.join("attachments").join(slug).join(name))
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
        .service(upload_files)
        .service(get_file)
        .service(delete_file)
        .service(get_archive)
        .service(get_qr);
}

#[post("/pastes/{slug}/files")]
async fn upload_files(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
    mut payload: Multipart,
) -> HttpResponse {
    if ARGS.no_file_upload {
        return error(
            StatusCode::FORBIDDEN,
            "uploads_disabled",
            "File uploads are disabled",
        );
    }
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.ensure_can_update(&value, &slug).await {
        Ok(paste) => paste,
        Err(e) => return error(StatusCode::NOT_FOUND, "not_found", e),
    };
    let directory = services.repo.data_dir.join("attachments").join(&paste.slug);
    if let Err(e) = tokio::fs::create_dir_all(&directory).await {
        return internal(e.to_string());
    }
    let Some(limit) = ARGS.max_file_size_mb.checked_mul(1024 * 1024) else {
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
                "too_many_files",
                "A paste may contain at most 32 files",
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
                    "file_too_large",
                    "Upload exceeds configured size limit",
                );
            }
            if let Err(e) = output.write_all(&chunk).await {
                let _ = std::fs::remove_file(&temporary);
                return internal(e.to_string());
            }
        }
        let storage_name = uuid::Uuid::new_v4().simple().to_string();
        let destination = match attachment_path(&services.repo.data_dir, &paste.slug, &storage_name)
        {
            Ok(path) => path,
            Err(e) => {
                let _ = std::fs::remove_file(&temporary);
                return error(StatusCode::BAD_REQUEST, "invalid_file", e);
            }
        };
        if tokio::fs::try_exists(&destination).await.unwrap_or(true) {
            let _ = std::fs::remove_file(&temporary);
            return error(
                StatusCode::CONFLICT,
                "file_exists",
                format!("{filename} already exists"),
            );
        }
        staged.push((temporary, destination, filename, storage_name, size as i64));
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
        .map(|(_, _, name, storage_name, size)| (name.clone(), storage_name.clone(), *size))
        .collect::<Vec<_>>();
    let created = match services.add_files(&value, &slug, &inputs).await {
        Ok(files) => files,
        Err(e) => {
            for path in promoted {
                let _ = std::fs::remove_file(path);
            }
            return error(StatusCode::BAD_REQUEST, "invalid_file", e);
        }
    };
    if created.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_upload",
            "No valid files were supplied",
        );
    }
    cleanup.paths.clear();
    HttpResponse::Created().json(json!({"items": created}))
}

#[get("/pastes/{slug}/files/{file_id}")]
async fn get_file(
    req: HttpRequest,
    services: web::Data<Services>,
    path: web::Path<(String, i64)>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    let paste = match services.get_paste(&value, &slug).await {
        Ok(Some(paste)) => paste,
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    };
    let Some(file) = paste.files.iter().find(|file| file.id == file_id) else {
        return error(StatusCode::NOT_FOUND, "not_found", "File not found");
    };
    let path = match attachment_path(&services.repo.data_dir, &paste.slug, &file.storage_name) {
        Ok(path) => path,
        Err(e) => return internal(e),
    };
    let content_type = mime_guess::from_path(&file.name).first_or_octet_stream();
    let named = match NamedFile::open(path) {
        Ok(named) => named,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return error(StatusCode::NOT_FOUND, "not_found", "File data is missing")
        }
        Err(e) => return internal(e.to_string()),
    };
    named
        .set_content_type(content_type)
        .set_content_disposition(header::ContentDisposition {
            disposition: header::DispositionType::Attachment,
            parameters: vec![header::DispositionParam::Filename(file.name.clone())],
        })
        .into_response(&req)
}

#[delete("/pastes/{slug}/files/{file_id}")]
async fn delete_file(
    req: HttpRequest,
    services: web::Data<Services>,
    path: web::Path<(String, i64)>,
) -> HttpResponse {
    let value = match principal(&services, &req)
        .await
        .and_then(|p| require_mutation(&services, &req, p))
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (slug, file_id) = path.into_inner();
    match services.delete_file(&value, &slug, file_id).await {
        Ok(true) => HttpResponse::NoContent().finish(),
        Ok(false) => error(StatusCode::NOT_FOUND, "not_found", "File not found"),
        Err(e) if e == "You do not own this paste" || e.starts_with("Missing ") => {
            error(StatusCode::FORBIDDEN, "forbidden", e)
        }
        Err(e) => internal(e),
    }
}

#[get("/pastes/{slug}/archive")]
async fn get_archive(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let paste = match services.get_paste(&value, &slug).await {
        Ok(Some(paste)) => paste,
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    };
    const MAX_ARCHIVE_INPUT: u64 = 64 * 1024 * 1024;
    let archive_input = paste.content.len() as u64
        + paste
            .files
            .iter()
            .map(|file| file.size.max(0) as u64)
            .sum::<u64>();
    if archive_input > MAX_ARCHIVE_INPUT {
        return error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "archive_too_large",
            "Archive input exceeds 64 MiB",
        );
    }
    let data_dir = services.repo.data_dir.clone();
    let archive_slug = paste.slug.clone();
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
        for file in &paste.files {
            let path = attachment_path(&data_dir, &paste.slug, &file.storage_name)?;
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            zip.start_file(&file.name, options)
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
                format!("attachment; filename=\"{archive_slug}.zip\""),
            ))
            .body(bytes),
        Ok(Err(e)) => internal(e),
        Err(e) => internal(e.to_string()),
    }
}

#[get("/pastes/{slug}/qr")]
async fn get_qr(
    req: HttpRequest,
    services: web::Data<Services>,
    slug: web::Path<String>,
) -> HttpResponse {
    if !ARGS.qr {
        return error(StatusCode::NOT_FOUND, "not_found", "QR codes are disabled");
    }
    let value = match principal(&services, &req).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    match services.get_paste(&value, &slug).await {
        Ok(Some(_)) => {}
        Ok(None) => return error(StatusCode::NOT_FOUND, "not_found", "Paste not found"),
        Err(e) => return internal(e),
    }
    let origin = ARGS
        .public_url
        .as_ref()
        .expect("validated at startup")
        .as_str()
        .trim_end_matches('/');
    match qrcode_generator::to_png_to_vec(
        format!("{origin}/pastes/{slug}"),
        qrcode_generator::QrCodeEcc::Low,
        512,
    ) {
        Ok(bytes) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, "image/png"))
            .body(bytes),
        Err(e) => internal(e.to_string()),
    }
}
