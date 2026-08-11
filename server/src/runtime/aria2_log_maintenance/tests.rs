use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::config::aria2::Aria2BinarySource;
use crate::runtime::ManagedAria2Process;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn maintenance_reads_only_the_tail_of_a_large_sparse_current_log() {
    let root = temp_dir("sparse");
    let log_path = aria2_log_path(&root);
    let sparse_size = 50_u64 * 1024 * 1024 * 1024;
    let marker = b"latest aria2 diagnostic tail";
    let mut file = create_file(&log_path);
    file.set_len(sparse_size).expect("sparse log should size");
    file.seek(SeekFrom::Start(sparse_size - marker.len() as u64))
        .expect("tail should seek");
    file.write_all(marker).expect("tail should write");
    drop(file);

    let report = maintain_aria2_log_files(&root).expect("maintenance should succeed");

    assert_eq!(
        fs::metadata(&log_path).expect("log should exist").len(),
        ARIA2_LOG_MAX_BYTES
    );
    assert_eq!(report.truncated_bytes, sparse_size - ARIA2_LOG_MAX_BYTES);
    let mut retained = File::open(&log_path).expect("trimmed log should open");
    retained
        .seek(SeekFrom::End(-(marker.len() as i64)))
        .expect("trimmed tail should seek");
    let mut actual = vec![0; marker.len()];
    retained
        .read_exact(&mut actual)
        .expect("trimmed tail should read");
    assert_eq!(actual, marker);
    assert_no_maintenance_temp_files(&root);
    remove_temp_dir(root);
}

#[test]
fn maintenance_keeps_two_newest_known_history_logs_across_both_naming_schemes() {
    let root = temp_dir("history");
    let log_dir = root.join(ARIA2_RUNTIME_DIR_NAME);
    fs::create_dir_all(&log_dir).expect("log directory should create");
    for name in [
        "aria2.1.log",
        "aria2.2.log",
        "aria2.3.log",
        "aria2.log.1",
        "aria2.log.2",
        "aria2.log.3",
    ] {
        fs::write(log_dir.join(name), name).expect("history log should write");
    }
    fs::write(log_dir.join("user-file.log"), "do not remove").expect("user file should write");

    let report = maintain_aria2_log_files(&root).expect("maintenance should succeed");
    let remaining = fs::read_dir(&log_dir)
        .expect("log directory should read")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(ToOwned::to_owned))
        .filter(|name| parse_history_name(name).is_some())
        .count();

    assert_eq!(remaining, ARIA2_LOG_HISTORY_RETENTION);
    assert_eq!(report.removed_history_files, 4);
    assert!(log_dir.join("aria2.1.log").is_file());
    assert!(log_dir.join("aria2.2.log").is_file());
    assert!(!log_dir.join("aria2.3.log").exists());
    assert!(!log_dir.join("aria2.log.1").exists());
    assert!(log_dir.join("user-file.log").is_file());
    remove_temp_dir(root);
}

#[cfg(unix)]
#[test]
fn maintenance_refuses_to_follow_symbolic_links() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("symlink");
    let outside = root.join("outside.log");
    let outside_file = create_file(&outside);
    outside_file
        .set_len(ARIA2_LOG_MAX_BYTES + 1)
        .expect("outside file should size");
    drop(outside_file);
    let log_dir = root.join(ARIA2_RUNTIME_DIR_NAME);
    fs::create_dir_all(&log_dir).expect("log directory should create");
    symlink(&outside, log_dir.join(ARIA2_LOG_FILE_NAME)).expect("symlink should create");

    let error = maintain_aria2_log_files(&root).expect_err("symbolic log should be rejected");

    assert!(error.contains("普通文件"));
    assert_eq!(
        fs::metadata(&outside)
            .expect("outside file should exist")
            .len(),
        ARIA2_LOG_MAX_BYTES + 1
    );
    remove_temp_dir(root);
}

#[cfg(unix)]
#[tokio::test]
async fn maintenance_refuses_running_or_unverified_aria2_state() {
    let root = temp_dir("in-use");
    let runtime = runtime(&root);
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    let log_path = aria2_log_path(&root);
    let log = create_file(&log_path);
    log.set_len(ARIA2_LOG_MAX_BYTES + 1)
        .expect("log should size");
    drop(log);
    let child = Command::new("sh")
        .args(["-c", "sleep 30"])
        .spawn()
        .expect("sleep child should spawn");
    *state
        .aria2_process
        .lock()
        .expect("process lock should succeed") =
        Some(ManagedAria2Process::new(child, Aria2BinarySource::Sidecar));

    assert_eq!(
        maintain_aria2_logs(&state)
            .await
            .expect("maintenance should inspect"),
        Aria2LogMaintenanceOutcome::Skipped(Aria2LogMaintenanceSkipReason::ProcessRunning)
    );
    assert_eq!(
        fs::metadata(&log_path).expect("log should exist").len(),
        ARIA2_LOG_MAX_BYTES + 1
    );

    if let Some(mut process) = state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .take()
    {
        process.kill().expect("sleep child should stop");
    }
    fs::write(&state.core.aria2_runtime_path, "{}")
        .expect("unverified runtime record should write");
    assert_eq!(
        maintain_aria2_logs(&state)
            .await
            .expect("maintenance should inspect"),
        Aria2LogMaintenanceOutcome::Skipped(Aria2LogMaintenanceSkipReason::RuntimeRecordPresent)
    );
    assert_eq!(
        fs::metadata(&log_path).expect("log should exist").len(),
        ARIA2_LOG_MAX_BYTES + 1
    );

    state.core.database.pool.close().await;
    drop(state);
    remove_temp_dir(root);
}

fn runtime(app_data_dir: &Path) -> ServerRuntimeConfig {
    ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.to_path_buf(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("address should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("address should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    }
}

fn aria2_log_path(root: &Path) -> PathBuf {
    root.join(ARIA2_RUNTIME_DIR_NAME).join(ARIA2_LOG_FILE_NAME)
}

fn create_file(path: &Path) -> File {
    fs::create_dir_all(path.parent().expect("file should have parent"))
        .expect("parent directory should create");
    File::create(path).expect("file should create")
}

fn assert_no_maintenance_temp_files(root: &Path) {
    let temp_prefix = format!(".{ARIA2_LOG_FILE_NAME}.motrix-maintenance-");
    assert!(fs::read_dir(root.join(ARIA2_RUNTIME_DIR_NAME))
        .expect("log directory should read")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(&temp_prefix)));
}

fn temp_dir(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-aria2-log-maintenance-{label}-{}-{id}",
        std::process::id()
    ))
}

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
