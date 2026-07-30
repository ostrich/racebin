#[cfg(test)]
mod tests {
    use super::Repository;

    #[actix_web::test]
    async fn sqlite_schema_is_repeatable() {
        let data_dir =
            std::env::temp_dir().join(format!("racebin-schema-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&data_dir).unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            data_dir.join("database.sqlite").display()
        );
        let repository = Repository::open(&url, &data_dir).await.unwrap();
        repository.migrate().await.unwrap();
        repository.migrate().await.unwrap();
        let tables: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master
             WHERE type='table' AND name IN ('app_user','pasta','pasta_file','api_key')",
        )
        .fetch_one(repository.pool())
        .await
        .unwrap();
        assert_eq!(tables, 4);
        drop(repository);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
