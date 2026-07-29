extern crate core;

use actix_web::{middleware, web, App, HttpServer};
use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

use crate::args::ARGS;

pub mod api_v2;
pub mod args;
pub mod repository;
pub mod services;

pub mod util {
    pub mod accounts;
    pub mod animalnumbers;
    pub mod api_keys;
    pub mod hashids;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    match util::accounts::run_cli_if_requested() {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(error) => {
            eprintln!("account command failed: {error}");
            return Err(std::io::Error::other(error));
        }
    }

    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                Local::now().format("%Y-%m-%dT%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();

    std::fs::create_dir_all(format!("{}/public", ARGS.data_dir))?;
    let repository = repository::Repository::open(&ARGS.data_dir).map_err(std::io::Error::other)?;
    repository.migrate().map_err(std::io::Error::other)?;
    let state = web::Data::new(services::Services::new(repository));

    log::info!("Racebin starting on http://{}:{}", ARGS.bind, ARGS.port);
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Logger::default())
            .configure(api_v2::configure)
    })
    .bind((ARGS.bind, ARGS.port))?
    .workers(ARGS.threads as usize)
    .run()
    .await
}
