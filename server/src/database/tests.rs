use super::*;

#[test]
fn connect_database_creates_required_tables() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));

            let database = connect_database(path.clone())
                .await
                .expect("database should connect");

            for table in [
                "download_tasks",
                "app_config",
                "task_history",
                "task_errors",
                "web_auth_config",
                "schema_migrations",
                "task_operations",
            ] {
                let exists: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
                )
                .bind(table)
                .fetch_one(&database.pool)
                .await
                .expect("table lookup should succeed");
                assert_eq!(exists, 1, "{table} should exist");
            }

            assert_task_query_indexes(&database.pool).await;

            let ui_preferences_exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ui_preferences'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("ui preferences table lookup should succeed");
            assert_eq!(ui_preferences_exists, 0);

            for column in [
                "category",
                "source_type",
                "confirmation_required",
                "metadata_torrent_path",
                "files_deleted",
                "selected_file_indexes",
                "owned_task_dir",
            ] {
                let column_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?",
                )
                .bind(column)
                .fetch_one(&database.pool)
                .await
                .expect("column lookup should succeed");
                assert_eq!(column_count, 1, "download_tasks.{column} should exist");
            }

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn connect_database_configures_sqlite_runtime_pragmas() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-pragmas-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));

            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
                .fetch_one(&database.pool)
                .await
                .expect("busy timeout pragma should be readable");
            let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&database.pool)
                .await
                .expect("journal mode pragma should be readable");
            let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
                .fetch_one(&database.pool)
                .await
                .expect("synchronous pragma should be readable");

            assert_eq!(busy_timeout, 5_000);
            assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
            assert_eq!(synchronous, 1);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn connect_database_removes_legacy_ui_preferences_table() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-ui-preferences-migrate-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                .expect("sqlite options should build")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("legacy db should connect");
            sqlx::query("CREATE TABLE ui_preferences (key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL)")
                .execute(&pool)
                .await
                .expect("legacy ui preferences table should create");
            pool.close().await;

            let database = connect_database(path.clone())
                .await
                .expect("database should connect and migrate");
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ui_preferences'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("ui preferences table lookup should succeed");
            assert_eq!(exists, 0);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn connect_database_migrates_existing_download_tasks_category() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-migrate-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));

            {
                let options =
                    SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                        .expect("sqlite options should build")
                        .create_if_missing(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
                    .await
                    .expect("legacy db should connect");
                sqlx::query(
                    r#"
                    CREATE TABLE download_tasks (
                        id INTEGER PRIMARY KEY,
                        url TEXT NOT NULL,
                        file_name TEXT NOT NULL,
                        save_dir TEXT NOT NULL,
                        gid TEXT,
                        status TEXT NOT NULL,
                        total_length INTEGER NOT NULL DEFAULT 0,
                        completed_length INTEGER NOT NULL DEFAULT 0,
                        download_speed INTEGER NOT NULL DEFAULT 0,
                        error_code TEXT,
                        error_message TEXT,
                        file_path TEXT,
                        created_at INTEGER NOT NULL,
                        updated_at INTEGER NOT NULL
                    )
                    "#,
                )
                .execute(&pool)
                .await
                .expect("legacy table should create");
                sqlx::query(
                    r#"
                    INSERT INTO download_tasks (
                        id, url, file_name, save_dir, gid, status, created_at, updated_at
                    ) VALUES
                        (1, 'https://example.com/file.zip', 'file.zip', '/downloads', NULL, 'pending', 1, 1),
                        (2, 'torrent:example.torrent', 'example', '/downloads', NULL, 'paused', 1, 1),
                        (3, 'magnet:?xt=urn:btih:test', 'magnet', '/downloads', NULL, 'pending', 1, 1)
                    "#,
                )
                .execute(&pool)
                .await
                .expect("legacy tasks should insert");
                pool.close().await;
            }

            let database = connect_database(path.clone())
                .await
                .expect("database should connect and migrate");
            for column in [
                "category",
                "source_type",
                "confirmation_required",
                "metadata_torrent_path",
                "files_deleted",
                "selected_file_indexes",
                "owned_task_dir",
            ] {
                let column_count: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = ?",
                )
                .bind(column)
                .fetch_one(&database.pool)
                .await
                .expect("column lookup should succeed");
                assert_eq!(
                    column_count, 1,
                    "download_tasks.{column} should be migrated"
                );
            }

            let source_types: Vec<String> =
                sqlx::query_scalar("SELECT source_type FROM download_tasks ORDER BY id")
                    .fetch_all(&database.pool)
                    .await
                    .expect("migrated source types should be readable");
            assert_eq!(source_types, ["url", "torrent", "magnet"]);

            let migrations: Vec<(i64, String)> = sqlx::query_as(
                "SELECT version, name FROM schema_migrations ORDER BY version",
            )
            .fetch_all(&database.pool)
            .await
            .expect("migration records should be readable");
            assert_eq!(
                migrations,
                [
                    (1, "legacy_download_tasks_baseline".to_string()),
                    (2, "task_operations".to_string()),
                    (3, "task_query_indexes".to_string()),
                ]
            );

            database.pool.close().await;

            let reopened = connect_database(path.clone())
                .await
                .expect("migrated database should reopen");
            let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
                .fetch_one(&reopened.pool)
                .await
                .expect("migration record count should be readable");
            assert_eq!(migration_count, 3);
            assert_task_query_indexes(&reopened.pool).await;
            reopened.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

async fn assert_task_query_indexes(pool: &sqlx::SqlitePool) {
    for index in [
        "idx_download_tasks_status_updated_at",
        "idx_task_history_task_created_at",
        "idx_task_errors_task_created_at",
        "idx_task_operations_unfinished_created_at",
    ] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?",
        )
        .bind(index)
        .fetch_one(pool)
        .await
        .expect("index lookup should succeed");
        assert_eq!(count, 1, "{index} should exist exactly once");
    }
}

#[test]
fn failed_migration_does_not_record_version_or_leave_partial_columns() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-failed-migrate-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));
            let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                .expect("sqlite options should build")
                .create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("invalid legacy database should connect");
            sqlx::query("CREATE TABLE download_tasks (id INTEGER PRIMARY KEY)")
                .execute(&pool)
                .await
                .expect("invalid legacy table should create");
            pool.close().await;

            let error = connect_database(path.clone())
                .await
                .expect_err("migration should reject a table without url");
            assert!(error.contains("legacy_download_tasks_baseline"));

            let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
                .expect("sqlite options should build");
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .expect("failed database should remain readable");
            let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
                .fetch_one(&pool)
                .await
                .expect("migration table should exist");
            let source_type_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM pragma_table_info('download_tasks') WHERE name = 'source_type'",
            )
            .fetch_one(&pool)
            .await
            .expect("column lookup should succeed");
            assert_eq!(migration_count, 0);
            assert_eq!(source_type_count, 0);
            pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}
