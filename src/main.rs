use actix_web::{middleware, web, App, HttpServer};
use env_logger::Builder;
use log::LevelFilter;

use crate::args::ARGS;

pub mod args;
pub mod http;
#[cfg(test)]
mod integration_tests;
pub mod repository;
pub mod services;

pub mod util {
    pub mod accounts;
    pub mod api_keys;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match repository::run_cli_if_requested().await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(error) => {
            eprintln!("database command failed: {error}");
            return Err(std::io::Error::other(error));
        }
    }
    match util::accounts::run_cli_if_requested().await {
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
    if ARGS.max_file_size_mb.checked_mul(1024 * 1024).is_none() {
        return Err(std::io::Error::other("--max-file-size-mb is too large"));
    }
    if ARGS.qr && ARGS.public_url.is_none() {
        return Err(std::io::Error::other(
            "--public-url is required when --qr is enabled",
        ));
    }
    std::fs::create_dir_all(&ARGS.data_dir)?;
    let database_url = ARGS.effective_database_url();
    let repository = repository::Repository::open(&database_url, &ARGS.data_dir)
        .await
        .map_err(std::io::Error::other)?;
    repository.migrate().await.map_err(std::io::Error::other)?;
    let purged = repository
        .purge_expired(services::now())
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
            if let Err(error) = cleanup_repository.purge_expired(services::now()).await {
                log::error!("expiration cleanup failed: {error}");
            }
        }
    });
    let state = web::Data::new(services::Services::new(repository));

    log::info!("Racebin starting on http://{}:{}", ARGS.bind, ARGS.port);
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(
                middleware::DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "no-referrer"))
                    .add((
                        "Content-Security-Policy",
                        "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
                    )),
            )
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Logger::default())
            .configure(http::configure)
    })
    .bind((ARGS.bind, ARGS.port))?
    .workers(ARGS.threads as usize)
    .run()
    .await
}
