use super::Database;

#[actix_web::test]
async fn sqlite_schema_is_repeatable() {
    let data_dir = std::env::temp_dir().join(format!("racebin-schema-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&data_dir).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        data_dir.join("database.sqlite").display()
    );
    let repository = Database::open(&url, &data_dir).await.unwrap();
    repository.migrate().await.unwrap();
    repository.migrate().await.unwrap();
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master
             WHERE type='table' AND name IN ('users','pastes','attachments','api_keys')",
    )
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(tables, 4);
    drop(repository);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[actix_web::test]
async fn legacy_rich_documents_migrate_without_losing_dependent_rows() {
    let data_dir = std::env::temp_dir().join(format!(
        "racebin-markdown-migration-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&data_dir).unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        data_dir.join("database.sqlite").display()
    );
    let repository = Database::open(&url, &data_dir).await.unwrap();
    sqlx::raw_sql("CREATE TABLE _sqlx_migrations(version BIGINT PRIMARY KEY,description TEXT NOT NULL,installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,success BOOLEAN NOT NULL,checksum BLOB NOT NULL,execution_time BIGINT NOT NULL)")
            .execute(repository.pool()).await.unwrap();
    for migration in crate::database::SQLITE_MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 10)
    {
        sqlx::raw_sql(migration.sql.clone())
            .execute(repository.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES($1,$2,1,$3,0)")
                .bind(migration.version).bind(migration.description.as_ref()).bind(migration.checksum.as_ref())
                .execute(repository.pool()).await.unwrap();
    }
    sqlx::query("INSERT INTO users(id,username,password_hash,role,created_at) VALUES(1,'migration-user','hash','admin',1)")
            .execute(repository.pool()).await.unwrap();
    let document = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1,"textAlign":null},"content":[{"type":"text","text":"Scene","marks":[{"type":"bold"}]}]},{"type":"paragraph","attrs":{"textAlign":null},"content":[{"type":"text","text":"Dialogue"}]}]}"#;
    sqlx::query("INSERT INTO pastes(id,owner_id,title,content,document_json,content_kind,language,visibility,created_at,updated_at,revision,read_count) VALUES('legacy',1,'','Scene Dialogue',$1,'rich_text','plaintext','private',1,1,1,0)")
            .bind(document).execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO attachments(id,paste_id,sort_order,filename,storage_key,size_bytes) VALUES(1,'legacy',0,'note.txt','stored',4)").execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO idempotency_records(user_id,operation,key_hash,request_hash,paste_id,created_at,expires_at) VALUES(1,'create_paste','key','request','legacy',1,999)").execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO paste_read_receipts(paste_id,key_hash,expires_at) VALUES('legacy','receipt',999)").execute(repository.pool()).await.unwrap();
    sqlx::query("INSERT INTO paste_read_grants(token_hash,paste_id,expires_at) VALUES('grant','legacy',999)").execute(repository.pool()).await.unwrap();

    repository.migrate().await.unwrap();
    let migrated: (String, String, i64) =
        sqlx::query_as("SELECT content_kind,content,revision FROM pastes WHERE id='legacy'")
            .fetch_one(repository.pool())
            .await
            .unwrap();
    assert_eq!(migrated.0, "markdown");
    assert!(migrated.1.contains("# **Scene**"));
    assert_eq!(migrated.2, 2);
    for table in [
        "attachments",
        "idempotency_records",
        "paste_read_receipts",
        "paste_read_grants",
    ] {
        let count: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(repository.pool())
                .await
                .unwrap();
        assert_eq!(count, 1, "{table}");
    }
    let legacy_column: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pragma_table_info('pastes') WHERE name='document_json'",
    )
    .fetch_one(repository.pool())
    .await
    .unwrap();
    assert_eq!(legacy_column, 0);
    drop(repository);
    let _ = std::fs::remove_dir_all(data_dir);
}
