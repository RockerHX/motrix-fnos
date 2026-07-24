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
