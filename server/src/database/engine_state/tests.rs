use super::super::*;

async fn old_database(trusted: bool) -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for statement in SCHEMA_STATEMENTS {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    let mut transaction = pool.begin().await.unwrap();
    for migration in &SCHEMA_MIGRATIONS[..7] {
        apply_schema_migration(&mut transaction, migration, false)
            .await
            .unwrap();
        if trusted {
            sqlx::query("INSERT INTO schema_migrations VALUES (?, ?, 1)")
                .bind(migration.version)
                .bind(migration.name)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }
    }
    transaction.commit().await.unwrap();
    sqlx::query("INSERT INTO download_tasks(id, url, file_name, save_dir, status, created_at, updated_at) VALUES (1, 'https://example.test/a', 'a', '/downloads', 'paused', 1, 1)")
        .execute(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn generation_migration_requires_prior_schema_proof_and_never_relabels_new_tasks() {
    for trusted in [false, true] {
        let pool = old_database(trusted).await;
        migrate_schema(&pool).await.unwrap();
        let generation: String =
            sqlx::query_scalar("SELECT engine_generation FROM download_tasks WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(generation, if trusted { "legacy-v1" } else { "unknown" });
        sqlx::query("UPDATE download_tasks SET engine_generation = 'native-v2' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        migrate_schema(&pool).await.unwrap();
        let generation: String =
            sqlx::query_scalar("SELECT engine_generation FROM download_tasks WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(generation, "native-v2");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 8);
        pool.close().await;
    }
}

#[tokio::test]
async fn interrupted_migration_rolls_back_columns_and_retries_without_losing_tasks() {
    let pool = old_database(true).await;
    sqlx::query("CREATE TRIGGER fail_v8 BEFORE INSERT ON schema_migrations WHEN NEW.version = 8 BEGIN SELECT RAISE(FAIL, 'forced migration failure'); END")
        .execute(&pool).await.unwrap();
    assert!(migrate_schema(&pool)
        .await
        .unwrap_err()
        .contains("forced migration failure"));
    let mut transaction = pool.begin().await.unwrap();
    assert_eq!(
        download_tasks_column_count(&mut transaction, "engine_generation")
            .await
            .unwrap(),
        0
    );
    transaction.rollback().await.unwrap();
    sqlx::query("DROP TRIGGER fail_v8")
        .execute(&pool)
        .await
        .unwrap();
    migrate_schema(&pool).await.unwrap();
    let row: (String, String) =
        sqlx::query_as("SELECT url, engine_generation FROM download_tasks WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row, ("https://example.test/a".into(), "legacy-v1".into()));
    pool.close().await;
}
