use super::backend::backend_contract;
use super::concurrency::concurrency_contract;
use super::copy::database_copy_contract;
use super::*;

#[actix_web::test]
async fn sqlite_backend_contract_and_concurrency() {
    let (repo, data_dir) = sqlite_repository("contract").await;
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn postgres_backend_contract_concurrency_and_copy() {
    let Ok(url) = std::env::var("RACEBIN_TEST_POSTGRES_URL") else {
        return;
    };
    let scratch = std::env::temp_dir().join(format!("racebin-postgres-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&scratch).unwrap();
    let bootstrap = Repository::open(&url, &scratch).await.unwrap();
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    sqlx::query("CREATE SCHEMA public")
        .execute(bootstrap.pool())
        .await
        .unwrap();
    drop(bootstrap);

    let repo = Repository::open(&url, &scratch).await.unwrap();
    repo.migrate().await.unwrap();
    backend_contract(repo.clone()).await;
    concurrency_contract(repo.clone()).await;
    drop(repo);

    sqlx::query(
        "TRUNCATE attachments,pastes,api_key_scopes,api_keys,sessions,invitations,users
         RESTART IDENTITY CASCADE",
    )
    .execute(Repository::open(&url, &scratch).await.unwrap().pool())
    .await
    .unwrap();
    database_copy_contract(&url, &scratch).await;
    let _ = std::fs::remove_dir_all(scratch);
}
