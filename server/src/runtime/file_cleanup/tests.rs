use super::*;
use crate::app::{HttpAppState, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR};
use crate::database::task_operations::{begin_task_operation, list_unfinished_task_operations};
use crate::database::{connect_database, DATABASE_FILE_NAME};
use crate::state::ServerState;
use crate::tasks::{TaskOperation, TaskOperationContext, TaskOperationStatus, TaskOperationType};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

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
async fn running_worker_is_notified_to_retry_failed_cleanup() {
    let root = temp_dir("worker-notify");
    let backup = root.join(".motrix-redownload-backup-11-500");
    std::fs::create_dir_all(&root).expect("root should create");
    std::fs::write(&backup, b"not a directory").expect("invalid backup path should write");
    let state = state_with_operation(&root, 11, backup.clone()).await;

    spawn_file_cleanup_worker(Arc::clone(&state)).await;
    wait_for_cleanup_error(&state).await;

    std::fs::remove_file(&backup).expect("invalid backup path should remove");
    std::fs::create_dir(&backup).expect("backup directory should recreate");
    spawn_file_cleanup_worker(Arc::clone(&state)).await;
    wait_for_worker_completion(&state, &backup).await;

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

#[tokio::test]
async fn permanent_delete_keeps_failed_record_and_finishes_after_restart() {
    let root = temp_dir("permanent-restart");
    let (state, backup) = pending_permanent_delete(&root).await;
    let metadata = root.join("task-metadata/task-17/source.torrent");
    assert!(metadata.exists());
    let parked = root.join("parked-backup");
    std::fs::rename(&backup, &parked).unwrap();
    std::fs::write(&backup, b"invalid backup").unwrap();
    assert!(run_file_cleanup_once(&state).await.is_err());
    assert_eq!(state.core.download_tasks.list().unwrap().len(), 1);
    assert_eq!(
        crate::database::tasks::list_download_tasks(&state.core.database.pool)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(metadata.exists());
    let unfinished = list_unfinished_task_operations(&state.core.database.pool)
        .await
        .unwrap();
    assert_eq!(unfinished.len(), 1);
    assert!(unfinished[0].is_file_cleanup_pending());
    assert!(unfinished[0].error_message.is_some());
    let config = state.aria2_config();
    let service = crate::api::build_task_service(&state);
    assert!(service
        .restore_removed_task(&config, 17, None, false)
        .await
        .unwrap_err()
        .contains("file_cleanup_pending"));
    assert!(service
        .permanently_delete_removed_task(&config, 17, false)
        .await
        .unwrap_err()
        .contains("file_cleanup_pending"));
    drop(service);
    std::fs::remove_file(&backup).unwrap();
    std::fs::rename(&parked, &backup).unwrap();
    state.core.database.pool.close().await;
    drop(state);

    let restarted = state_without_operation(&root).await;
    let tasks = crate::database::tasks::list_download_tasks(&restarted.core.database.pool)
        .await
        .unwrap();
    restarted
        .core
        .download_tasks
        .with_tasks_mut(|stored| *stored = tasks)
        .unwrap();
    crate::runtime::reconcile_unfinished_task_operations(&restarted)
        .await
        .unwrap();
    assert!(restarted.aria2_runtime_snapshot().is_none());
    run_file_cleanup_once(&restarted).await.unwrap();
    assert!(!backup.exists());
    assert!(!metadata.exists());
    assert_eq!(std::fs::read(root.join("unrelated.bin")).unwrap(), b"keep");
    assert!(restarted.core.download_tasks.list().unwrap().is_empty());
    assert!(
        crate::database::tasks::list_download_tasks(&restarted.core.database.pool)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        list_unfinished_task_operations(&restarted.core.database.pool)
            .await
            .unwrap()
            .is_empty()
    );
    close_state(restarted, &root).await;
}

#[tokio::test]
async fn permanent_delete_retries_record_commit_after_files_are_cleaned() {
    let root = temp_dir("permanent-commit-retry");
    let (state, backup) = pending_permanent_delete(&root).await;
    sqlx::query("CREATE TRIGGER fail_task_delete BEFORE DELETE ON download_tasks BEGIN SELECT RAISE(ABORT, 'injected record delete failure'); END")
        .execute(&state.core.database.pool).await.unwrap();
    assert!(run_file_cleanup_once(&state).await.is_err());
    assert!(!backup.exists());
    assert_eq!(state.core.download_tasks.list().unwrap().len(), 1);
    assert!(list_unfinished_task_operations(&state.core.database.pool)
        .await
        .unwrap()[0]
        .is_file_cleanup_pending());
    sqlx::query("DROP TRIGGER fail_task_delete")
        .execute(&state.core.database.pool)
        .await
        .unwrap();
    run_file_cleanup_once(&state).await.unwrap();
    assert!(state.core.download_tasks.list().unwrap().is_empty());
    close_state(state, &root).await;
}

async fn pending_permanent_delete(root: &Path) -> (Arc<HttpAppState>, PathBuf) {
    let state = state_without_operation(root).await;
    let download_dir = root.join("downloads");
    std::fs::create_dir_all(&download_dir).unwrap();
    let payload = download_dir.join("file.bin");
    std::fs::write(&payload, b"partial bytes").unwrap();
    std::fs::write(root.join("unrelated.bin"), b"keep").unwrap();
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&serde_json::json!({"paths": [root]})).unwrap(),
    )
    .unwrap();
    let mut task: crate::tasks::DownloadTask = serde_json::from_value(serde_json::json!({
        "id": 17, "url": "https://example.com/file.bin", "sourceType": "url",
        "fileName": "file.bin", "saveDir": download_dir, "category": "默认",
        "status": "removed", "removedFromStatus": "paused", "totalLength": 100,
        "completedLength": 13, "downloadSpeed": 0, "filePath": payload,
        "useProxy": false, "filesDeleted": false, "confirmationRequired": false,
        "files": [], "createdAt": 1, "updatedAt": 1,
    }))
    .unwrap();
    task.engine_generation = state.aria2_config().engine_generation();
    let metadata = root.join("task-metadata/task-17/source.torrent");
    std::fs::create_dir_all(metadata.parent().unwrap()).unwrap();
    std::fs::write(&metadata, b"metadata").unwrap();
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .unwrap();
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .unwrap();
    let pending = crate::api::build_task_service(&state)
        .permanently_delete_removed_task(&state.aria2_config(), 17, true)
        .await
        .unwrap()
        .unwrap();
    assert!(pending.files_deleted);
    assert!(!payload.exists());
    assert_eq!(state.core.download_tasks.list().unwrap().len(), 1);
    let operation = list_unfinished_task_operations(&state.core.database.pool)
        .await
        .unwrap()
        .remove(0);
    let backup = PathBuf::from(&operation.context.file_cleanup_paths[0]);
    (state, backup)
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

async fn wait_for_cleanup_error(state: &HttpAppState) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let operations = list_unfinished_task_operations(&state.core.database.pool)
                .await
                .expect("unfinished operations should list");
            if operations
                .first()
                .and_then(|operation| operation.error_message.as_deref())
                .is_some()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("worker should record the first cleanup failure");
}

async fn wait_for_worker_completion(state: &Arc<HttpAppState>, backup: &Path) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let unfinished = list_unfinished_task_operations(&state.core.database.pool)
                .await
                .expect("unfinished operations should list");
            if !backup.exists() && unfinished.is_empty() && Arc::strong_count(state) == 1 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("notified worker should complete cleanup");
}

fn temp_dir(label: &str) -> PathBuf {
    let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("motrix-fnos-file-cleanup-{label}-{id}"));
    std::fs::create_dir_all(&path).expect("temporary directory should create");
    path
}
