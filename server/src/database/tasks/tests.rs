use super::*;
use crate::database::connect_database;
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

            assert!(deleted);
            assert!(tasks.is_empty());
            assert_eq!(history_count, 0);
            assert_eq!(error_count, 0);

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
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_millis()
}
