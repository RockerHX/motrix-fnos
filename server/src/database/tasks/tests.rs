use super::*;
use crate::database::connect_database;
use crate::database::settings::{
    set_app_config_value, StoredDownloadProxyConfig, DOWNLOAD_PROXY_CONFIG_KEY,
};
use crate::database::task_operations::begin_task_operation;
use crate::tasks::{TaskOperationContext, TaskOperationType};

#[test]
fn repository_inserts_updates_and_lists_tasks() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir()
                .join(format!("motrix-fnos-repository-test-{}.sqlite", now_ms()));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut task = sample_task();
            task.owned_task_dir = Some("/downloads/task-1".to_string());
            task.files_deleted = true;
            task.selected_file_indexes = vec![1, 3];

            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be inserted");
            task.status = DownloadTaskStatus::Paused;
            task.updated_at += 1;
            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be updated");

            let tasks = list_download_tasks(&database.pool)
                .await
                .expect("tasks should be listed");
            let max_id = max_download_task_id(&database.pool)
                .await
                .expect("max id should be read");

            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].status, DownloadTaskStatus::Paused);
            assert_eq!(tasks[0].source_type, DownloadTaskSourceType::Url);
            assert!(tasks[0].files_deleted);
            assert_eq!(
                tasks[0].owned_task_dir.as_deref(),
                Some("/downloads/task-1")
            );
            assert_eq!(tasks[0].selected_file_indexes, [1, 3]);
            assert_eq!(max_id, task.id);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn repository_persists_private_override_and_injects_profile_without_serializing_urls() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-proxy-persistence-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            set_app_config_value(
                &database.pool,
                DOWNLOAD_PROXY_CONFIG_KEY,
                &StoredDownloadProxyConfig {
                    proxy_url: Some(
                        "http://profile-user:profile-pass@profile.example:7890".to_string(),
                    ),
                    revision: 1,
                    updated_at: 1,
                    bt_use_proxy: false,
                    bt_runtime_apply: crate::database::settings::BtRuntimeApplyStatus::Deferred,
                },
            )
            .await
            .expect("profile should save");

            let mut task = sample_task();
            task.use_proxy = true;
            task.proxy_binding = TaskProxyBinding::override_url(
                "socks5://override-user:override-pass@override.example:1080".to_string(),
            );
            upsert_download_task(&database.pool, &task)
                .await
                .expect("override task should persist");

            let stored_override: String =
                sqlx::query_scalar("SELECT proxy_url FROM task_proxy_overrides WHERE task_id = ?")
                    .bind(task.id as i64)
                    .fetch_one(&database.pool)
                    .await
                    .expect("private override should exist");
            assert_eq!(
                stored_override,
                "socks5://override-user:override-pass@override.example:1080"
            );

            let loaded = list_download_tasks(&database.pool)
                .await
                .expect("override task should load")
                .remove(0);
            assert!(loaded.use_proxy);
            assert_eq!(loaded.proxy_binding.source(), TaskProxySource::Override);
            assert_eq!(
                loaded.proxy_binding.effective_proxy_url(),
                Some("socks5://override-user:override-pass@override.example:1080")
            );
            let public_json = serde_json::to_string(&loaded).expect("task should serialize");
            assert!(public_json.contains("\"useProxy\":true"));
            assert!(!public_json.contains("override-user"));
            assert!(!public_json.contains("proxyBinding"));
            assert!(!format!("{loaded:?}").contains("override-pass"));

            task.proxy_binding = TaskProxyBinding::profile(Some(
                "http://profile-user:profile-pass@profile.example:7890".to_string(),
            ));
            task.updated_at += 1;
            upsert_download_task(&database.pool, &task)
                .await
                .expect("profile task should persist");
            let override_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM task_proxy_overrides WHERE task_id = ?")
                    .bind(task.id as i64)
                    .fetch_one(&database.pool)
                    .await
                    .expect("override count should be readable");
            assert_eq!(override_count, 0);
            let loaded = list_download_tasks(&database.pool)
                .await
                .expect("profile task should load")
                .remove(0);
            assert_eq!(loaded.proxy_binding.source(), TaskProxySource::Profile);
            assert_eq!(
                loaded.proxy_binding.effective_proxy_url(),
                Some("http://profile-user:profile-pass@profile.example:7890")
            );

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[tokio::test]
async fn legacy_bt_proxy_state_is_normalized_when_tasks_are_loaded() {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-legacy-bt-proxy-normalization-{}.sqlite",
        now_ms()
    ));
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    let mut task = sample_task();
    task.source_type = DownloadTaskSourceType::Magnet;
    task.use_proxy = false;
    upsert_download_task(&database.pool, &task)
        .await
        .expect("BT task should persist");
    sqlx::query("UPDATE download_tasks SET use_proxy = 1, proxy_source = 'override' WHERE id = ?")
        .bind(task.id as i64)
        .execute(&database.pool)
        .await
        .expect("legacy proxy state should be writable");
    sqlx::query(
        "INSERT INTO task_proxy_overrides (task_id, proxy_url, updated_at) VALUES (?, ?, ?)",
    )
    .bind(task.id as i64)
    .bind("socks5://legacy.example:1080")
    .bind(task.updated_at as i64)
    .execute(&database.pool)
    .await
    .expect("legacy override should be writable");

    let loaded = list_download_tasks(&database.pool)
        .await
        .expect("legacy task should load")
        .remove(0);
    assert!(!loaded.use_proxy);
    assert_eq!(loaded.proxy_binding.source(), TaskProxySource::Profile);
    assert_eq!(loaded.proxy_binding.effective_proxy_url(), None);

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[test]
fn repository_rolls_back_task_when_private_override_persistence_fails() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-proxy-rollback-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            sqlx::query(
                "CREATE TRIGGER fail_proxy_override BEFORE INSERT ON task_proxy_overrides BEGIN SELECT RAISE(FAIL, 'forced proxy override failure'); END",
            )
            .execute(&database.pool)
            .await
            .expect("failure trigger should create");
            let mut task = sample_task();
            task.use_proxy = true;
            task.proxy_binding =
                TaskProxyBinding::override_url("http://proxy.example:7890".to_string());

            let error = upsert_download_task(&database.pool, &task)
                .await
                .expect_err("override failure should roll back task");
            assert!(error.contains("forced proxy override failure"));
            let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(&database.pool)
                .await
                .expect("task count should be readable");
            assert_eq!(task_count, 0);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn repository_records_history_and_error() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path =
                std::env::temp_dir().join(format!("motrix-fnos-history-test-{}.sqlite", now_ms()));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut task = sample_task();
            task.status = DownloadTaskStatus::Error;
            task.error_code = Some("3".to_string());
            task.error_message = Some("Resource not found".to_string());

            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be inserted");
            record_task_history(&database.pool, &task, Some("failed"))
                .await
                .expect("history should be inserted");
            record_task_error(&database.pool, &task)
                .await
                .expect("error should be inserted");

            let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_history")
                .fetch_one(&database.pool)
                .await
                .expect("history count should be read");
            let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_errors")
                .fetch_one(&database.pool)
                .await
                .expect("error count should be read");

            assert_eq!(history_count, 1);
            assert_eq!(error_count, 1);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn persist_task_state_rolls_back_when_error_recording_fails() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-state-rollback-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut task = sample_task();
            task.status = DownloadTaskStatus::Error;
            task.error_code = Some("3".to_string());
            task.error_message = Some("Resource not found".to_string());
            sqlx::query(
                "CREATE TRIGGER fail_task_error BEFORE INSERT ON task_errors BEGIN SELECT RAISE(FAIL, 'forced task error failure'); END",
            )
            .execute(&database.pool)
            .await
            .expect("failure trigger should create");

            let error = persist_download_task_state(&database.pool, &task)
                .await
                .expect_err("persistence should fail at the error record step");
            assert!(
                error.contains("forced task error failure"),
                "unexpected persistence error: {error}"
            );

            let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(&database.pool)
                .await
                .expect("task count should be readable");
            let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_history")
                .fetch_one(&database.pool)
                .await
                .expect("history count should be readable");
            let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_errors")
                .fetch_one(&database.pool)
                .await
                .expect("error count should be readable");
            assert_eq!(task_count, 0);
            assert_eq!(history_count, 0);
            assert_eq!(error_count, 0);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn concurrent_task_state_persistence_keeps_related_records_consistent() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-state-concurrent-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut first = sample_task();
            first.status = DownloadTaskStatus::Error;
            first.error_code = Some("3".to_string());
            first.error_message = Some("first failure".to_string());
            let mut second = first.clone();
            second.id = 2;
            second.url = "https://example.com/second.zip".to_string();
            second.file_name = "second.zip".to_string();
            second.gid = Some("def456".to_string());
            second.error_message = Some("second failure".to_string());

            let (first_result, second_result) = tokio::join!(
                persist_download_task_state(&database.pool, &first),
                persist_download_task_state(&database.pool, &second),
            );
            first_result.expect("first task should persist");
            second_result.expect("second task should persist");

            let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(&database.pool)
                .await
                .expect("task count should be readable");
            let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_history")
                .fetch_one(&database.pool)
                .await
                .expect("history count should be readable");
            let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_errors")
                .fetch_one(&database.pool)
                .await
                .expect("error count should be readable");
            assert_eq!(task_count, 2);
            assert_eq!(history_count, 2);
            assert_eq!(error_count, 2);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn repository_deletes_task_record_history_and_errors() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-delete-record-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut task = sample_task();
            task.status = DownloadTaskStatus::Error;
            task.error_code = Some("3".to_string());
            task.error_message = Some("Resource not found".to_string());
            task.use_proxy = true;
            task.proxy_binding =
                TaskProxyBinding::override_url("http://proxy.example:7890".to_string());

            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be inserted");
            record_task_history(&database.pool, &task, Some("failed"))
                .await
                .expect("history should be inserted");
            record_task_error(&database.pool, &task)
                .await
                .expect("error should be inserted");

            let deleted = delete_download_task_record(&database.pool, task.id)
                .await
                .expect("task record should be deleted");
            let tasks = list_download_tasks(&database.pool)
                .await
                .expect("tasks should be listed");
            let history_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_history")
                .fetch_one(&database.pool)
                .await
                .expect("history count should be read");
            let error_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_errors")
                .fetch_one(&database.pool)
                .await
                .expect("error count should be read");
            let proxy_override_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM task_proxy_overrides")
                    .fetch_one(&database.pool)
                    .await
                    .expect("proxy override count should be read");

            assert!(deleted);
            assert!(tasks.is_empty());
            assert_eq!(history_count, 0);
            assert_eq!(error_count, 0);
            assert_eq!(proxy_override_count, 0);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn repository_deletes_task_record_and_completes_operation_together() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-delete-record-operation-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let task = sample_task();
            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be inserted");
            let mut operation = TaskOperation::with_id(
                "delete-operation",
                task.id,
                TaskOperationType::PermanentDelete,
                "prepared",
                TaskOperationContext::default(),
            );
            begin_task_operation(&database.pool, &operation)
                .await
                .expect("operation should be inserted");
            operation.complete("record_deleted");

            let deleted =
                delete_download_task_record_with_operation(&database.pool, task.id, &operation)
                    .await
                    .expect("task record and operation should update together");
            let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(&database.pool)
                .await
                .expect("task count should be read");
            let operation_status: String = sqlx::query_scalar(
                "SELECT status FROM task_operations WHERE id = 'delete-operation'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("operation status should be read");

            assert!(deleted);
            assert_eq!(task_count, 0);
            assert_eq!(operation_status, "completed");

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn delete_task_record_rolls_back_when_operation_completion_fails() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-delete-record-operation-rollback-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let task = sample_task();
            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should be inserted");
            let mut operation = TaskOperation::with_id(
                "failing-delete-operation",
                task.id,
                TaskOperationType::PermanentDelete,
                "prepared",
                TaskOperationContext::default(),
            );
            begin_task_operation(&database.pool, &operation)
                .await
                .expect("operation should be inserted");
            operation.complete("record_deleted");
            sqlx::query(
                "CREATE TRIGGER fail_permanent_delete_operation BEFORE UPDATE ON task_operations WHEN NEW.id = 'failing-delete-operation' BEGIN SELECT RAISE(FAIL, 'forced operation completion failure'); END",
            )
            .execute(&database.pool)
            .await
            .expect("failure trigger should create");

            let error = delete_download_task_record_with_operation(
                &database.pool,
                task.id,
                &operation,
            )
            .await
            .expect_err("operation completion failure should roll back deletion");
            let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks")
                .fetch_one(&database.pool)
                .await
                .expect("task count should be read");

            assert!(error.contains("forced operation completion failure"));
            assert_eq!(task_count, 1);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

fn sample_task() -> DownloadTask {
    DownloadTask {
        id: 1,
        engine_generation: crate::config::aria2::EngineGeneration::LegacyV1,
        removed_from_status: None,
        url: "https://example.com/file.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "file.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("abc123".to_string()),
        status: DownloadTaskStatus::Active,
        total_length: 100,
        completed_length: 40,
        download_speed: 20,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/file.zip".to_string()),
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}

#[tokio::test]
async fn native_file_snapshot_survives_reload_and_invalidates_deleted_progress() {
    let database = connect_database(
        std::env::temp_dir().join(format!("motrix-native-snapshot-{}.sqlite", now_ms())),
    )
    .await
    .unwrap();
    let mut task = native_snapshot_task(1);
    upsert_download_task(&database.pool, &task).await.unwrap();
    let stored_path: String =
        sqlx::query_scalar("SELECT path FROM task_file_snapshots WHERE task_id = 1")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(stored_path, "file.bin");
    assert_eq!(
        list_download_tasks(&database.pool).await.unwrap()[0].files,
        task.files
    );
    task.files_deleted = true;
    upsert_download_task(&database.pool, &task).await.unwrap();
    let loaded = list_download_tasks(&database.pool).await.unwrap().remove(0);
    assert_eq!(loaded.completed_length, 0);
    assert_eq!(loaded.files[0].completed_length, 0);
    assert_eq!(loaded.selected_file_indexes, [1]);
    let path = database.path.clone();
    database.pool.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn snapshot_failure_and_generation_change_roll_back_the_entire_task_update() {
    let database = connect_database(
        std::env::temp_dir().join(format!("motrix-native-atomic-{}.sqlite", now_ms())),
    )
    .await
    .unwrap();
    let task = native_snapshot_task(1);
    upsert_download_task(&database.pool, &task).await.unwrap();
    let mut changed = task.clone();
    changed.engine_generation = crate::config::aria2::EngineGeneration::LegacyV1;
    changed.status = DownloadTaskStatus::Removed;
    assert!(upsert_download_task(&database.pool, &changed)
        .await
        .unwrap_err()
        .contains("代际"));
    changed.engine_generation = task.engine_generation;
    sqlx::query("CREATE TRIGGER fail_snapshot BEFORE INSERT ON task_file_snapshots BEGIN SELECT RAISE(FAIL, 'snapshot failure'); END").execute(&database.pool).await.unwrap();
    assert!(upsert_download_task(&database.pool, &changed)
        .await
        .unwrap_err()
        .contains("snapshot failure"));
    let loaded = list_download_tasks(&database.pool).await.unwrap().remove(0);
    assert_eq!(loaded, task);
    let path = database.path.clone();
    database.pool.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn removing_one_native_reference_preserves_other_tasks_and_the_shared_state() {
    let root = std::env::temp_dir().join(format!("motrix-native-references-{}", now_ms()));
    std::fs::create_dir_all(root.join("engine/native-v2/state")).unwrap();
    std::fs::write(
        root.join("engine/native-v2/state/checkpoint.db"),
        b"engine-owned",
    )
    .unwrap();
    let database = connect_database(root.join("motrix-fnos.sqlite"))
        .await
        .unwrap();
    for id in [1, 2] {
        let mut task = native_snapshot_task(id);
        task.source_type = DownloadTaskSourceType::Torrent;
        upsert_download_task(&database.pool, &task).await.unwrap();
    }
    delete_download_task_record(&database.pool, 1)
        .await
        .unwrap();
    let references: Vec<(i64, String, String, i64)> =
        sqlx::query_as("SELECT task_id, gid, private_path, available FROM task_metadata_refs")
            .fetch_all(&database.pool)
            .await
            .unwrap();
    assert_eq!(
        references,
        [(2, "native-2".into(), "engine/native-v2/state".into(), 0)]
    );
    assert_eq!(
        std::fs::read(root.join("engine/native-v2/state/checkpoint.db")).unwrap(),
        b"engine-owned"
    );
    assert_eq!(
        list_download_tasks(&database.pool).await.unwrap()[0]
            .files
            .len(),
        1
    );
    database.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn private_torrent_reference_records_actual_size_hash_and_missing_file_fact() {
    let root = std::env::temp_dir().join(format!("motrix-torrent-reference-{}", now_ms()));
    std::fs::create_dir_all(&root).unwrap();
    let database = connect_database(root.join("motrix-fnos.sqlite"))
        .await
        .unwrap();
    let path =
        crate::tasks::files::save_restore_torrent_metadata(&root, 1, b"torrent-bytes").unwrap();
    let mut task = native_snapshot_task(1);
    task.source_type = DownloadTaskSourceType::Torrent;
    task.metadata_torrent_path = Some(path.display().to_string());
    upsert_download_task(&database.pool, &task).await.unwrap();
    let reference: (i64, String, i64) = sqlx::query_as(
        "SELECT size_bytes, checksum, available FROM task_metadata_refs WHERE task_id = 1",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert_eq!(reference.0, 13);
    assert_eq!(
        reference.1,
        format!("{:x}", Sha256::digest(b"torrent-bytes"))
    );
    assert_eq!(reference.2, 1);
    validate_native_metadata_refs(&database.pool, &root, std::slice::from_ref(&task))
        .await
        .unwrap();
    std::fs::write(&path, b"altered-bytes").unwrap();
    assert!(
        validate_native_metadata_refs(&database.pool, &root, std::slice::from_ref(&task))
            .await
            .unwrap_err()
            .contains("task_resume_state_invalid")
    );
    std::fs::remove_file(path).unwrap();
    upsert_download_task(&database.pool, &task).await.unwrap();
    let available: i64 =
        sqlx::query_scalar("SELECT available FROM task_metadata_refs WHERE task_id = 1")
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(available, 0);
    database.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

fn native_snapshot_task(id: u64) -> DownloadTask {
    let mut task = sample_task();
    task.id = id;
    task.engine_generation = crate::config::aria2::EngineGeneration::NativeV2;
    task.gid = Some(format!("native-{id}"));
    task.status = DownloadTaskStatus::Paused;
    task.files = vec![DownloadTaskFile {
        index: 1,
        path: "/downloads/file.bin".into(),
        name: "file.bin".into(),
        length: 100,
        completed_length: 40,
        selected: true,
    }];
    task.selected_file_indexes = vec![1];
    task
}

#[tokio::test]
async fn batch_snapshot_failure_rolls_back_every_task_and_file_list() {
    let database = connect_database(
        std::env::temp_dir().join(format!("motrix-batch-state-{}.sqlite", now_ms())),
    )
    .await
    .unwrap();
    let mut tasks = vec![native_snapshot_task(1), native_snapshot_task(2)];
    persist_download_task_states(&database.pool, &tasks)
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_second_snapshot BEFORE INSERT ON task_file_snapshots WHEN NEW.task_id = 2 BEGIN SELECT RAISE(FAIL, 'batch failure'); END").execute(&database.pool).await.unwrap();
    for task in &mut tasks {
        task.completed_length = 90;
        task.files[0].completed_length = 90;
    }
    assert!(persist_download_task_states(&database.pool, &tasks)
        .await
        .unwrap_err()
        .contains("batch failure"));
    let loaded = list_download_tasks(&database.pool).await.unwrap();
    assert!(loaded
        .iter()
        .all(|task| task.completed_length == 40 && task.files[0].completed_length == 40));
    sqlx::query("UPDATE download_tasks SET status = 'unrecognized' WHERE id = 1")
        .execute(&database.pool)
        .await
        .unwrap();
    assert!(list_download_tasks(&database.pool)
        .await
        .unwrap_err()
        .contains("未知状态"));
    let path = database.path.clone();
    database.pool.close().await;
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn snapshot_io_failures_preserve_the_previous_transaction_and_allow_retry() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    for readonly in [false, true] {
        let root = std::env::temp_dir().join(format!(
            "motrix-snapshot-io-{}-{}-{readonly}",
            std::process::id(),
            now_ms()
        ));
        let database = connect_database(root.join("motrix-fnos.sqlite"))
            .await
            .unwrap();
        let mut tasks = vec![native_snapshot_task(1), native_snapshot_task(2)];
        persist_download_task_states(&database.pool, &tasks)
            .await
            .unwrap();
        let previous = list_download_tasks(&database.pool).await.unwrap();
        let path = database.path.clone();
        database.pool.close().await;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .read_only(readonly),
            )
            .await
            .unwrap();
        if !readonly {
            let pages: i64 = sqlx::query_scalar("PRAGMA page_count")
                .fetch_one(&pool)
                .await
                .unwrap();
            sqlx::query(&format!("PRAGMA max_page_count = {pages}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        for task in &mut tasks {
            task.completed_length = 90;
            task.files[0].completed_length = 90;
        }
        tasks[1].files[0].name = "large snapshot ".repeat(16_384);
        let error = persist_download_task_states(&pool, &tasks)
            .await
            .unwrap_err();
        assert!(
            error.contains(if readonly { "readonly" } else { "full" }),
            "{error}"
        );
        assert_eq!(list_download_tasks(&pool).await.unwrap(), previous);
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA quick_check")
                .fetch_one(&pool)
                .await
                .unwrap(),
            "ok"
        );
        pool.close().await;
        let database = connect_database(path).await.unwrap();
        tasks[1].files[0].name = "file.bin".to_string();
        persist_download_task_states(&database.pool, &tasks)
            .await
            .unwrap();
        let recovered = list_download_tasks(&database.pool).await.unwrap();
        assert!(recovered
            .iter()
            .all(|task| task.completed_length == 90 && task.files[0].completed_length == 90));
        database.pool.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_millis()
}

#[tokio::test]
async fn native_reader_filters_legacy_before_decoding_and_rejects_unattributed_snapshots() {
    use crate::config::aria2::EngineGeneration;
    let path = std::env::temp_dir().join(format!("motrix-generation-reader-{}.sqlite", now_ms()));
    let database = connect_database(path.clone()).await.unwrap();
    let pool = &database.pool;
    let task = sample_task();
    upsert_download_task(pool, &task).await.unwrap();
    sqlx::query("UPDATE download_tasks SET selected_file_indexes = 'broken'")
        .execute(pool)
        .await
        .unwrap();
    assert!(list_download_tasks(pool).await.is_err());
    assert!(
        list_download_tasks_for_engine(pool, EngineGeneration::NativeV2)
            .await
            .unwrap()
            .is_empty()
    );
    for generation in ["native-v2", "unknown"] {
        sqlx::query("UPDATE download_tasks SET engine_generation = ?")
            .bind(generation)
            .execute(pool)
            .await
            .unwrap();
        assert!(
            list_download_tasks_for_engine(pool, EngineGeneration::NativeV2)
                .await
                .is_err()
        );
    }
    sqlx::query("UPDATE download_tasks SET engine_generation = 'legacy-v1'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO task_file_snapshots (task_id, file_index, path, name, length, completed_length, selected, checked_at) VALUES (?, -1, 'old', 'old', -1, -1, 1, 1)").bind(task.id as i64).execute(pool).await.unwrap();
    assert!(
        list_download_tasks_for_engine(pool, EngineGeneration::NativeV2)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query("DELETE FROM download_tasks")
        .execute(pool)
        .await
        .unwrap();
    // 外键通常清理关联快照；关闭约束模拟无法确定归属的历史损坏数据。
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await
        .unwrap();
    sqlx::query("INSERT INTO task_file_snapshots (task_id, file_index, path, name, length, completed_length, selected, checked_at) VALUES (999, -1, 'orphan', 'orphan', -1, -1, 1, 1)").execute(&mut *connection).await.unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    assert!(
        list_download_tasks_for_engine(pool, EngineGeneration::NativeV2)
            .await
            .is_err()
    );
    database.pool.close().await;
    std::fs::remove_file(path).unwrap();
}
