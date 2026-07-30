use crate::account::{self as accounts, api_keys};
use crate::repository::{copy_database, DatabaseKind, Repository};
use crate::services::{PasteInput, PasteQuery, PasteService, Principal};
use std::path::{Path, PathBuf};

mod backend;
mod concurrency;
mod copy;
mod http;
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
        "INSERT INTO users(id,username,password_hash,role,enabled,password_change_required,created_at)
         VALUES($1,$2,$3,$4,1,0,$5)",
    )
    .bind(id)
    .bind(username)
    .bind(accounts::password_hash("correct horse battery staple").unwrap())
    .bind(role)
    .bind(crate::time::unix_timestamp())
    .execute(repo.pool())
    .await
    .unwrap();
    if repo.kind() == DatabaseKind::Postgres {
        sqlx::query(
            "SELECT setval(pg_get_serial_sequence('users','id'),
                           (SELECT max(id) FROM users),TRUE)",
        )
        .execute(repo.pool())
        .await
        .unwrap();
    }
}

fn principal(id: i64, username: &str, role: &str) -> Principal {
    Principal::Session(accounts::SessionUser {
        user: accounts::User {
            id,
            username: username.to_string(),
            role: role.to_string(),
            enabled: true,
            password_change_required: false,
        },
        csrf_token: "csrf".to_string(),
    })
}

fn paste_input(title: &str, visibility: &str) -> PasteInput {
    PasteInput {
        title: Some(title.to_string()),
        content: Some(format!("content for {title}")),
        content_kind: Some("text".to_string()),
        language: Some("plaintext".to_string()),
        visibility: Some(visibility.to_string()),
        expires_at: None,
        read_limit: None,
    }
}
