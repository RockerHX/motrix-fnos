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

            let ui_preferences_exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'ui_preferences'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("ui preferences table lookup should succeed");
            assert_eq!(ui_preferences_exists, 0);

            for column in ["category", "confirmation_required", "metadata_torrent_path"] {
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
                pool.close().await;
            }

            let database = connect_database(path.clone())
                .await
                .expect("database should connect and migrate");
            for column in ["category", "confirmation_required", "metadata_torrent_path"] {
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

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}
