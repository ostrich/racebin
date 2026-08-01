use actix_web::{middleware, web, App, HttpServer};
use env_logger::Builder;
use log::LevelFilter;
use std::time::Duration;

use crate::args::ARGS;

const ACCESS_LOG_FORMAT: &str = "%a \"%{METHOD}xi\" %s %b \"%{User-Agent}i\" %T";

pub mod account;
pub mod args;
mod cli;
pub mod domain_error;
pub mod http;
#[cfg(test)]
mod integration_tests;
pub mod repository;
pub mod services;
pub mod time;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match cli::database::run_if_requested().await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(error) => {
            eprintln!("database command failed: {error}");
            return Err(std::io::Error::other(error));
        }
    }
    match cli::account::run_if_requested().await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(error) => {
            eprintln!("account command failed: {error}");
            return Err(std::io::Error::other(error));
        }
    }

    Builder::new().filter(None, LevelFilter::Info).init();

    if ARGS.threads == 0 {
        return Err(std::io::Error::other("--threads must be at least 1"));
    }
    if ARGS
        .max_attachment_size_mb
        .checked_mul(1024 * 1024)
        .is_none()
    {
        return Err(std::io::Error::other(
            "--max-attachment-size-mb is too large",
        ));
    }
    if ARGS.qr_codes && ARGS.public_url.is_none() {
        return Err(std::io::Error::other(
            "--public-url is required when --qr is enabled",
        ));
    }
    prepare_data_dir(std::path::Path::new(&ARGS.data_dir))?;
    let database_url = ARGS.effective_database_url();
    let repository = repository::Repository::open(&database_url, &ARGS.data_dir)
        .await
        .map_err(std::io::Error::other)?;
    repository.migrate().await.map_err(std::io::Error::other)?;
    let purged = repository
        .purge_expired(time::unix_timestamp())
        .await
        .map_err(std::io::Error::other)?;
    if purged != 0 {
        log::info!("removed {purged} expired pastes");
    }
    let cleanup_repository = repository.clone();
    actix_web::rt::spawn(async move {
        let mut interval = actix_web::rt::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(error) = cleanup_repository
                .purge_expired(time::unix_timestamp())
                .await
            {
                log::error!("expired-record cleanup failed: {error}");
            }
        }
    });
    let state = web::Data::new(services::PasteService::new(repository));

    log::info!("Racebin starting on http://{}:{}", ARGS.bind, ARGS.port);
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(
                middleware::DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "no-referrer"))
                    .add((
                        "Permissions-Policy",
                        "camera=(), microphone=(), geolocation=(), payment=(), usb=()",
                    ))
                    .add((
                        "Content-Security-Policy",
                        "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
                    )),
            )
            .wrap(middleware::NormalizePath::trim())
            .wrap(
                middleware::Logger::new(ACCESS_LOG_FORMAT)
                    .custom_request_replace("METHOD", |request| request.method().to_string()),
            )
            .configure(http::configure)
    })
    .workers(ARGS.threads as usize)
    .client_request_timeout(Duration::from_secs(15))
    .client_disconnect_timeout(Duration::from_secs(5))
    .max_connections(1024)
    .bind((ARGS.bind, ARGS.port))?;
    server.run().await
}

fn prepare_data_dir(path: &std::path::Path) -> std::io::Result<()> {
    let created = !path.exists();
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    if created {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{prepare_data_dir, ACCESS_LOG_FORMAT};

    #[test]
    fn access_log_format_never_contains_request_targets() {
        for unsafe_directive in ["%r", "%U", "%q", "%{Referer}i"] {
            assert!(!ACCESS_LOG_FORMAT.contains(unsafe_directive));
        }
        assert!(ACCESS_LOG_FORMAT.contains("%{METHOD}xi"));
    }

    #[cfg(unix)]
    #[test]
    fn newly_created_data_directories_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("racebin-mode-{}", uuid::Uuid::new_v4()));
        prepare_data_dir(&path).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        std::fs::remove_dir(path).unwrap();
    }

    #[test]
    fn packaged_service_enforces_private_process_and_state_modes() {
        let unit = include_str!("../packaging/racebin.service");
        assert!(unit.contains("UMask=0077"));
        assert!(unit.contains("StateDirectoryMode=0700"));
    }
}
