use motrix_fnos_server::database::connect_database;
use std::path::PathBuf;
use std::process::Command;

fn temp_app_data_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-{name}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}

#[test]
fn database_check_binary_uses_success_and_failure_exit_codes() {
    let app_data_dir = temp_app_data_dir("database-cli");
    std::fs::create_dir_all(&app_data_dir).expect("app data directory should create");
    let database_path = app_data_dir.join("motrix-fnos.sqlite");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should create");
    runtime.block_on(async {
        let database = connect_database(database_path.clone())
            .await
            .expect("database should connect");
        database.pool.close().await;
    });

    let binary = env!("CARGO_BIN_EXE_motrix-fnos-server");
    let valid = Command::new(binary)
        .arg("database-check")
        .env("MOTRIX_FNOS_APP_DATA_DIR", &app_data_dir)
        .output()
        .expect("database-check should execute");
    assert!(
        valid.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&valid.stderr)
    );

    for suffix in ["-wal", "-shm"] {
        let sidecar = std::path::PathBuf::from(format!("{}{}", database_path.display(), suffix));
        let _ = std::fs::remove_file(sidecar);
    }
    std::fs::write(&database_path, b"not a sqlite database")
        .expect("corrupted database should write");
    let invalid = Command::new(binary)
        .arg("database-check")
        .env("MOTRIX_FNOS_APP_DATA_DIR", &app_data_dir)
        .output()
        .expect("database-check should execute");
    assert_eq!(invalid.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&invalid.stderr).contains("执行 SQLite 完整性检查失败"),
        "stderr: {}",
        String::from_utf8_lossy(&invalid.stderr)
    );

    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[test]
fn database_backup_binary_creates_snapshot() {
    let app_data_dir = temp_app_data_dir("database-backup-cli");
    std::fs::create_dir_all(&app_data_dir).expect("app data directory should create");
    let database_path = app_data_dir.join("motrix-fnos.sqlite");
    let output_path = app_data_dir.join("backup.sqlite");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should create");
    runtime.block_on(async {
        let database = connect_database(database_path.clone())
            .await
            .expect("database should connect");
        database.pool.close().await;
    });

    let binary = env!("CARGO_BIN_EXE_motrix-fnos-server");
    let result = Command::new(binary)
        .args([
            "database-backup",
            output_path.to_str().expect("path should be utf8"),
        ])
        .env("MOTRIX_FNOS_APP_DATA_DIR", &app_data_dir)
        .output()
        .expect("database-backup should execute");
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(output_path.is_file());

    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[test]
fn database_cleanup_history_binary_defaults_to_dry_run() {
    let app_data_dir = temp_app_data_dir("database-cleanup-cli");
    std::fs::create_dir_all(&app_data_dir).expect("app data directory should create");
    let database_path = app_data_dir.join("motrix-fnos.sqlite");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime should create");
    runtime.block_on(async {
        let database = connect_database(database_path.clone())
            .await
            .expect("database should connect");
        sqlx::query(
            "INSERT INTO task_history (task_id, status, message, created_at) VALUES (1, 'complete', 'old', 100), (1, 'paused', 'boundary', 200)",
        )
        .execute(&database.pool)
        .await
        .expect("history rows should insert");
        sqlx::query(
            "INSERT INTO task_errors (task_id, error_code, error_message, created_at) VALUES (1, 'old', 'old', 100), (1, 'boundary', 'boundary', 200)",
        )
        .execute(&database.pool)
        .await
        .expect("error rows should insert");
        database.pool.close().await;
    });

    let binary = env!("CARGO_BIN_EXE_motrix-fnos-server");
    let preview = Command::new(binary)
        .args(["database-cleanup-history", "200"])
        .env("MOTRIX_FNOS_APP_DATA_DIR", &app_data_dir)
        .output()
        .expect("cleanup preview should execute");
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("预览"));

    let apply = Command::new(binary)
        .args(["database-cleanup-history", "200", "--apply"])
        .env("MOTRIX_FNOS_APP_DATA_DIR", &app_data_dir)
        .output()
        .expect("cleanup apply should execute");
    assert!(apply.status.success());
    assert!(String::from_utf8_lossy(&apply.stdout).contains("清理完成"));

    let database = runtime.block_on(async {
        connect_database(database_path.clone())
            .await
            .expect("database should reconnect")
    });
    let history_count: i64 = runtime.block_on(async {
        sqlx::query_scalar("SELECT COUNT(*) FROM task_history")
            .fetch_one(&database.pool)
            .await
            .expect("history count should read")
    });
    assert_eq!(history_count, 1);
    runtime.block_on(database.pool.close());
    let _ = std::fs::remove_dir_all(app_data_dir);
}
