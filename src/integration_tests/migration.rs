use super::*;

#[actix_web::test]
async fn sqlite_migration_adoption_repeatability_and_checksum_validation() {
    let (repo, data_dir) = sqlite_repository("migration").await;
    repo.migrate().await.unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations")
        .execute(repo.pool())
        .await
        .unwrap();
    repo.migrate().await.unwrap();
    sqlx::query("UPDATE _sqlx_migrations SET checksum=$1")
        .bind(vec![0_u8])
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(repo.migrate().await.is_err());
    drop(repo);
    let _ = std::fs::remove_dir_all(data_dir);
}
