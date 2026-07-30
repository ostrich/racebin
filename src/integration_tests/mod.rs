use crate::account::{self as accounts, api_keys};
use crate::repository::{copy_database, DatabaseKind, Repository};
use crate::services::{now, PasteInput, PasteQuery, Principal, Services};
use std::path::{Path, PathBuf};

mod backend;
mod concurrency;
mod copy;
mod migration;
mod runners;

fn sqlite_url(data_dir: &Path) -> String {
    format!(
        "sqlite://{}?mode=rwc",
        data_dir.join("database.sqlite").display()
    )
}

async fn sqlite_repository(label: &str) -> (Repository, PathBuf) {
    let data_dir = std::env::temp_dir().join(format!("racebin-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let repository = Repository::open(&sqlite_url(&data_dir), &data_dir)
        .await
        .unwrap();
    repository.migrate().await.unwrap();
    (repository, data_dir)
}

async fn insert_user(repo: &Repository, id: i64, username: &str, role: &str) {
    sqlx::query(
        "INSERT INTO app_user(id,username,password_hash,role,enabled,force_password_change,created)
         VALUES($1,$2,$3,$4,1,0,$5)",
    )
    .bind(id)
    .bind(username)
    .bind(accounts::password_hash("correct horse battery staple").unwrap())
    .bind(role)
    .bind(now())
    .execute(repo.pool())
    .await
    .unwrap();
    if repo.kind() == DatabaseKind::Postgres {
        sqlx::query(
            "SELECT setval(pg_get_serial_sequence('app_user','id'),
                           (SELECT max(id) FROM app_user),TRUE)",
        )
        .execute(repo.pool())
        .await
        .unwrap();
    }
}

fn principal(id: i64, username: &str, role: &str) -> Principal {
    Principal::User(accounts::SessionUser {
        user: accounts::User {
            id,
            username: username.to_string(),
            role: role.to_string(),
            enabled: true,
            force_password_change: false,
        },
        csrf_token: "csrf".to_string(),
    })
}

fn paste_input(title: &str, access: &str) -> PasteInput {
    PasteInput {
        title: Some(title.to_string()),
        content: Some(format!("content for {title}")),
        kind: Some("text".to_string()),
        syntax: Some("none".to_string()),
        access: Some(access.to_string()),
        expiration: None,
        burn_after_reads: None,
    }
}
