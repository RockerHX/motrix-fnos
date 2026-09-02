use super::*;

#[test]
fn backup_database_creates_a_valid_consistent_snapshot() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let source = std::env::temp_dir().join(format!(
                "motrix-fnos-db-backup-source-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let output = source.with_file_name("motrix-fnos-db-backup-output.sqlite");
            let database = connect_database(source.clone())
                .await
                .expect("source database should connect");
            sqlx::query(
                "INSERT INTO app_config (key, value, updated_at) VALUES ('backup-test', 'saved', 1)",
            )
            .execute(&database.pool)
            .await
            .expect("source value should insert");
            database.pool.close().await;

            backup_database(source.clone(), output.clone())
                .await
                .expect("backup should complete");
            check_integrity(output.clone())
                .await
                .expect("backup should pass integrity check");
            let backup = connect_database(output.clone())
                .await
                .expect("backup database should open");
            let value: String =
                sqlx::query_scalar("SELECT value FROM app_config WHERE key = 'backup-test'")
                    .fetch_one(&backup.pool)
                    .await
                    .expect("backup value should be readable");
            assert_eq!(value, "saved");
            backup.pool.close().await;

            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(output);
        });
}

#[test]
fn backup_database_handles_writes_during_snapshot() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let source = std::env::temp_dir().join(format!(
                "motrix-fnos-db-backup-write-source-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let output = source.with_file_name("motrix-fnos-db-backup-write-output.sqlite");
            let database = connect_database(source.clone())
                .await
                .expect("source database should connect");
            let writer_pool = database.pool.clone();
            let writer = tokio::spawn(async move {
                for index in 0..100_i64 {
                    sqlx::query("INSERT INTO app_config (key, value, updated_at) VALUES (?, ?, ?)")
                        .bind(format!("backup-write-{index}"))
                        .bind("saved")
                        .bind(index)
                        .execute(&writer_pool)
                        .await
                        .expect("concurrent write should succeed");
                    tokio::task::yield_now().await;
                }
            });

            backup_database(source.clone(), output.clone())
                .await
                .expect("backup should complete during writes");
            writer.await.expect("writer should complete");
            check_integrity(output.clone())
                .await
                .expect("concurrent backup should pass integrity check");
            database.pool.close().await;

            let _ = std::fs::remove_file(source);
            let _ = std::fs::remove_file(output);
        });
}

#[test]
fn backup_database_rejects_source_or_invalid_target_paths() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let source = std::env::temp_dir().join(format!(
                "motrix-fnos-db-backup-target-source-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let database = connect_database(source.clone())
                .await
                .expect("source database should connect");
            database.pool.close().await;

            let same_path = backup_database(source.clone(), source.clone())
                .await
                .expect_err("source path should not be used as backup target");
            assert!(same_path.contains("不能覆盖当前数据库"));

            let missing_parent = source
                .parent()
                .expect("source parent should exist")
                .join("missing-backup-parent")
                .join("backup.sqlite");
            let invalid_parent = backup_database(source.clone(), missing_parent)
                .await
                .expect_err("missing target parent should fail");
            assert!(invalid_parent.contains("备份目标目录"));

            let _ = std::fs::remove_file(source);
        });
}

#[test]
fn validate_backup_rejects_corrupted_output() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-backup-invalid-output-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            std::fs::write(&path, b"invalid backup").expect("invalid backup should write");

            let error = validate_backup(&path)
                .await
                .expect_err("corrupted backup should fail validation");
            assert!(error.contains("执行 SQLite 完整性检查失败"));
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn check_integrity_accepts_valid_database() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-check-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            database.pool.close().await;

            check_integrity(path.clone())
                .await
                .expect("valid database should pass integrity check");
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn check_integrity_rejects_corrupted_database() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-check-corrupt-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_nanos()
            ));
            std::fs::write(&path, b"not a sqlite database").expect("corrupted file should write");

            let error = check_integrity(path.clone())
                .await
                .expect_err("corrupted database should fail integrity check");
            assert!(error.contains("执行 SQLite 完整性检查失败"));
            let _ = std::fs::remove_file(path);
        });
}

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
                "task_proxy_overrides",
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
                "use_proxy",
                "proxy_source",
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
                "use_proxy",
                "proxy_source",
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
            let proxy_states: Vec<(i64, String)> = sqlx::query_as(
                "SELECT use_proxy, proxy_source FROM download_tasks ORDER BY id",
            )
            .fetch_all(&database.pool)
            .await
            .expect("migrated proxy states should be readable");
            assert_eq!(
                proxy_states,
                [
                    (0, "profile".to_string()),
                    (0, "profile".to_string()),
                    (0, "profile".to_string()),
                ]
            );

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
                    (4, "task_proxy_state".to_string()),
                    (5, "web_auth_jwt_secret".to_string()),
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
            assert_eq!(migration_count, 5);
            assert_task_query_indexes(&reopened.pool).await;
            reopened.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn connect_database_migrates_1_8_x_task_schema_to_proxy_state_v4() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-v3-proxy-migrate-test-{}.sqlite",
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
                .expect("1.8.x database should connect");
            sqlx::query(
                r#"
                CREATE TABLE download_tasks (
                    id INTEGER PRIMARY KEY,
                    url TEXT NOT NULL,
                    source_type TEXT NOT NULL DEFAULT 'url',
                    file_name TEXT NOT NULL,
                    save_dir TEXT NOT NULL,
                    owned_task_dir TEXT,
                    category TEXT NOT NULL DEFAULT '默认',
                    gid TEXT,
                    status TEXT NOT NULL,
                    total_length INTEGER NOT NULL DEFAULT 0,
                    completed_length INTEGER NOT NULL DEFAULT 0,
                    download_speed INTEGER NOT NULL DEFAULT 0,
                    error_code TEXT,
                    error_message TEXT,
                    file_path TEXT,
                    confirmation_required INTEGER NOT NULL DEFAULT 0,
                    metadata_torrent_path TEXT,
                    files_deleted INTEGER NOT NULL DEFAULT 0,
                    selected_file_indexes TEXT NOT NULL DEFAULT '[]',
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("1.8.x download tasks table should create");
            sqlx::query(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL)",
            )
            .execute(&pool)
            .await
            .expect("migration table should create");
            for (version, name) in [
                (1_i64, "legacy_download_tasks_baseline"),
                (2_i64, "task_operations"),
                (3_i64, "task_query_indexes"),
            ] {
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, 1)",
                )
                .bind(version)
                .bind(name)
                .execute(&pool)
                .await
                .expect("existing migration should insert");
            }
            sqlx::query(
                r#"
                INSERT INTO download_tasks (
                    id, url, file_name, save_dir, status, created_at, updated_at
                ) VALUES (7, 'https://example.com/archive.zip', 'archive.zip', '/downloads', 'paused', 1, 2)
                "#,
            )
            .execute(&pool)
            .await
            .expect("existing task should insert");
            pool.close().await;

            let database = connect_database(path.clone())
                .await
                .expect("1.8.x database should migrate");
            let task: (i64, String, String, i64, String) = sqlx::query_as(
                "SELECT id, url, status, use_proxy, proxy_source FROM download_tasks WHERE id = 7",
            )
            .fetch_one(&database.pool)
            .await
            .expect("migrated task should remain readable");
            assert_eq!(
                task,
                (
                    7,
                    "https://example.com/archive.zip".to_string(),
                    "paused".to_string(),
                    0,
                    "profile".to_string(),
                )
            );
            let migration_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 4 AND name = 'task_proxy_state'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("v4 migration should be recorded");
            assert_eq!(migration_count, 1);
            database.pool.close().await;

            let reopened = connect_database(path.clone())
                .await
                .expect("migrated database should reopen");
            let migration_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 4",
            )
            .fetch_one(&reopened.pool)
            .await
            .expect("v4 migration count should be readable");
            assert_eq!(migration_count, 1);
            reopened.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn connect_database_migrates_and_persists_web_auth_jwt_secret() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            use base64::Engine;

            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-db-web-auth-jwt-migrate-{}.sqlite",
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
                .expect("legacy database should connect");
            sqlx::query(
                r#"
                CREATE TABLE web_auth_config (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                    password_hash TEXT,
                    password_updated_at INTEGER,
                    auth_version INTEGER NOT NULL CHECK (auth_version > 0)
                )
                "#,
            )
            .execute(&pool)
            .await
            .expect("legacy web auth table should create");
            sqlx::query(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, applied_at INTEGER NOT NULL)",
            )
            .execute(&pool)
            .await
            .expect("migration table should create");
            for (version, name) in [
                (1_i64, "legacy_download_tasks_baseline"),
                (2_i64, "task_operations"),
                (3_i64, "task_query_indexes"),
                (4_i64, "task_proxy_state"),
            ] {
                sqlx::query(
                    "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, 1)",
                )
                .bind(version)
                .bind(name)
                .execute(&pool)
                .await
                .expect("legacy migration should insert");
            }
            sqlx::query(
                "INSERT INTO web_auth_config (id, enabled, password_hash, password_updated_at, auth_version) VALUES (1, 1, 'hash', 1, 7)",
            )
            .execute(&pool)
            .await
            .expect("legacy web auth row should insert");
            pool.close().await;

            let database = connect_database(path.clone())
                .await
                .expect("legacy web auth database should migrate");
            let secret: String = sqlx::query_scalar(
                "SELECT jwt_secret FROM web_auth_config WHERE id = 1",
            )
            .fetch_one(&database.pool)
            .await
            .expect("JWT secret should be generated");
            assert_eq!(
                base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(&secret)
                    .expect("JWT secret should be base64url")
                    .len(),
                32
            );
            let migration_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 5 AND name = 'web_auth_jwt_secret'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("JWT migration should be recorded");
            assert_eq!(migration_count, 1);
            database.pool.close().await;

            let reopened = connect_database(path.clone())
                .await
                .expect("migrated database should reopen");
            let reopened_secret: String = sqlx::query_scalar(
                "SELECT jwt_secret FROM web_auth_config WHERE id = 1",
            )
            .fetch_one(&reopened.pool)
            .await
            .expect("JWT secret should persist");
            assert_eq!(reopened_secret, secret);
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
