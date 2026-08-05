use super::*;
use crate::app::{HttpAppState, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR};
use crate::database::task_operations::{begin_task_operation, list_unfinished_task_operations};
use crate::database::{connect_database, DATABASE_FILE_NAME};
use crate::state::ServerState;
use crate::tasks::{TaskOperation, TaskOperationContext, TaskOperationStatus, TaskOperationType};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn worker_removes_persisted_task_backup_and_completes_operation() {
    let root = temp_dir("success");
    let backup = root.join(".motrix-redownload-backup-7-100");
    std::fs::create_dir_all(backup.join("nested")).expect("backup directory should create");
    std::fs::write(backup.join("nested/payload.bin"), b"payload")
        .expect("backup file should write");
    let state = state_with_operation(&root, 7, backup.clone()).await;

    run_file_cleanup_once(&state)
        .await
        .expect("worker should clean persisted backup");

    assert!(!backup.exists());
    assert!(list_unfinished_task_operations(&state.core.database.pool)
        .await
        .expect("unfinished operations should list")
        .is_empty());
    close_state(state, &root).await;
}

#[tokio::test]
async fn worker_keeps_failed_cleanup_in_progress_and_retries_later() {
    let root = temp_dir("retry");
    let backup = root.join(".motrix-redownload-backup-8-200");
    std::fs::create_dir_all(&root).expect("root should create");
    std::fs::write(&backup, b"not a directory").expect("invalid backup path should write");
    let state = state_with_operation(&root, 8, backup.clone()).await;

    assert!(run_file_cleanup_once(&state).await.is_err());
    let unfinished = list_unfinished_task_operations(&state.core.database.pool)
        .await
        .expect("unfinished operation should list");
    assert_eq!(unfinished.len(), 1);
    assert_eq!(unfinished[0].status, TaskOperationStatus::InProgress);
    assert_eq!(unfinished[0].phase, "file_cleanup_pending");
    assert!(unfinished[0]
        .error_message
        .as_deref()
        .is_some_and(|message| message.contains("不是目录")));

    std::fs::remove_file(&backup).expect("invalid backup path should remove");
    std::fs::create_dir(&backup).expect("backup directory should recreate");
    run_file_cleanup_once(&state)
        .await
        .expect("worker should retry failed cleanup");
    assert!(!backup.exists());
    assert!(list_unfinished_task_operations(&state.core.database.pool)
        .await
        .expect("unfinished operations should list")
        .is_empty());
    close_state(state, &root).await;
}

#[tokio::test]
async fn persisted_cleanup_is_recovered_by_a_new_state_after_restart() {
    let root = temp_dir("restart");
    let backup = root.join(".motrix-redownload-backup-9-300");
    std::fs::create_dir_all(&backup).expect("backup directory should create");
    std::fs::write(backup.join("payload.bin"), b"payload").expect("backup file should write");

    let first_state = state_with_operation(&root, 9, backup.clone()).await;
    first_state.core.database.pool.close().await;
    drop(first_state);

    let second_state = state_without_operation(&root).await;
    run_file_cleanup_once(&second_state)
        .await
        .expect("new state should recover persisted cleanup");
    assert!(!backup.exists());
    assert!(
        list_unfinished_task_operations(&second_state.core.database.pool)
            .await
            .expect("unfinished operations should list")
            .is_empty()
    );
    close_state(second_state, &root).await;
}

#[tokio::test]
async fn aria2_operation_reconcile_skips_file_cleanup_operations() {
    let root = temp_dir("reconcile");
    let backup = root.join(".motrix-redownload-backup-10-400");
    std::fs::create_dir_all(&backup).expect("backup directory should create");
    let state = state_with_operation(&root, 10, backup.clone()).await;

    crate::runtime::reconcile_unfinished_task_operations(&state)
        .await
        .expect("Aria2 reconciliation should skip file cleanup");

    assert!(state.aria2_runtime_snapshot().is_none());
    assert_eq!(
        list_unfinished_task_operations(&state.core.database.pool)
            .await
            .expect("unfinished operations should list")
            .len(),
        1
    );
    run_file_cleanup_once(&state)
        .await
        .expect("file cleanup worker should process the operation");
    close_state(state, &root).await;
}

async fn state_with_operation(root: &Path, task_id: u64, backup: PathBuf) -> Arc<HttpAppState> {
    let state = state_without_operation(root).await;
    let operation = TaskOperation::with_id(
        format!("file-cleanup-{task_id}"),
        task_id,
        TaskOperationType::Delete,
        "file_cleanup_pending",
        TaskOperationContext {
            file_cleanup_paths: vec![backup.display().to_string()],
            ..TaskOperationContext::default()
        },
    );
    begin_task_operation(&state.core.database.pool, &operation)
        .await
        .expect("cleanup operation should persist");
    state
}

async fn state_without_operation(root: &Path) -> Arc<HttpAppState> {
    let runtime = ServerRuntimeConfig {
        app_data_dir: root.to_path_buf(),
        database_path: root.join(DATABASE_FILE_NAME),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("address should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("address should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        aria2_path: None,
        accessible_paths_path: root.join("accessible-paths.json"),
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let database = connect_database(runtime.database_path.clone())
        .await
        .expect("database should connect");
    Arc::new(HttpAppState::new(
        ServerState::new(database, Vec::new(), 1),
        runtime,
    ))
}

async fn close_state(state: Arc<HttpAppState>, root: &Path) {
    state.core.database.pool.close().await;
    drop(state);
    let _ = std::fs::remove_dir_all(root);
}

fn temp_dir(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("motrix-fnos-file-cleanup-{label}-{id}"));
    std::fs::create_dir_all(&path).expect("temporary directory should create");
    path
}
