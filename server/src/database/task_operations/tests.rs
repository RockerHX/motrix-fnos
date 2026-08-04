use super::*;
use crate::database::connect_database;
use crate::database::tasks::persist_download_task_state_with_operation;
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};

#[test]
fn task_operation_repository_tracks_lifecycle_and_unfinished_records() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-operations-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let context = TaskOperationContext {
                old_gid: Some("old-gid".to_string()),
                new_gid: None,
                aria2_request: None,
                critical_paths: vec!["/downloads/archive.zip".to_string()],
                completed_side_effects: vec!["old_task_paused".to_string()],
                task_snapshot: None,
            };
            let mut operation = TaskOperation::with_id(
                "operation-1",
                1,
                TaskOperationType::Redownload,
                "prepared",
                context,
            );

            begin_task_operation(&database.pool, &operation)
                .await
                .expect("operation should begin");
            let unfinished = list_unfinished_task_operations(&database.pool)
                .await
                .expect("unfinished operations should list");
            assert_eq!(unfinished, vec![operation.clone()]);

            operation.update_phase(
                "aria2_created",
                TaskOperationContext {
                    old_gid: Some("old-gid".to_string()),
                    new_gid: Some("new-gid".to_string()),
                    aria2_request: None,
                    critical_paths: vec!["/downloads/archive.zip".to_string()],
                    completed_side_effects: vec![
                        "old_task_paused".to_string(),
                        "new_task_created".to_string(),
                    ],
                    task_snapshot: None,
                },
            );
            update_task_operation(&database.pool, &operation)
                .await
                .expect("operation should update");
            let unfinished = list_unfinished_task_operations(&database.pool)
                .await
                .expect("updated operation should list");
            assert_eq!(unfinished, vec![operation.clone()]);

            operation.complete("completed");
            update_task_operation(&database.pool, &operation)
                .await
                .expect("operation should complete");
            assert!(list_unfinished_task_operations(&database.pool)
                .await
                .expect("finished operation list should be readable")
                .is_empty());

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

#[test]
fn task_state_and_operation_update_commit_or_rollback_together() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-operation-transaction-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut operation = TaskOperation::with_id(
                "operation-2",
                2,
                TaskOperationType::Pause,
                "started",
                TaskOperationContext::default(),
            );
            begin_task_operation(&database.pool, &operation)
                .await
                .expect("operation should begin");
            operation.complete("task_persisted");
            let mut task = sample_task(2);
            task.status = DownloadTaskStatus::Paused;

            persist_download_task_state_with_operation(&database.pool, &task, &operation)
                .await
                .expect("task and operation should commit");
            let persisted_phase: String = sqlx::query_scalar(
                "SELECT phase FROM task_operations WHERE id = 'operation-2'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("operation phase should be readable");
            let task_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM download_tasks WHERE id = 2 AND status = 'paused'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("task count should be readable");
            assert_eq!(persisted_phase, "task_persisted");
            assert_eq!(task_count, 1);

            let mut failed_operation = TaskOperation::with_id(
                "operation-3",
                3,
                TaskOperationType::Pause,
                "started",
                TaskOperationContext::default(),
            );
            begin_task_operation(&database.pool, &failed_operation)
                .await
                .expect("second operation should begin");
            failed_operation.complete("task_persisted");
            sqlx::query(
                "CREATE TRIGGER fail_task_operation_update BEFORE UPDATE ON task_operations WHEN NEW.id = 'operation-3' BEGIN SELECT RAISE(FAIL, 'forced operation update failure'); END",
            )
            .execute(&database.pool)
            .await
            .expect("failure trigger should create");

            let error = persist_download_task_state_with_operation(
                &database.pool,
                &sample_task(3),
                &failed_operation,
            )
            .await
            .expect_err("operation update should fail");
            assert!(
                error.contains("forced operation update failure"),
                "unexpected persistence error: {error}"
            );
            let missing_task_count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM download_tasks WHERE id = 3")
                    .fetch_one(&database.pool)
                    .await
                    .expect("task count should be readable");
            let original_phase: String = sqlx::query_scalar(
                "SELECT phase FROM task_operations WHERE id = 'operation-3'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("operation phase should be readable");
            assert_eq!(missing_task_count, 0);
            assert_eq!(original_phase, "started");

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
    });
}

#[test]
fn task_and_operation_roll_back_together_when_task_history_fails() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-task-history-rollback-test-{}.sqlite",
                now_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let mut operation = TaskOperation::with_id(
                "operation-history-rollback",
                4,
                TaskOperationType::Pause,
                "started",
                TaskOperationContext::default(),
            );
            begin_task_operation(&database.pool, &operation)
                .await
                .expect("operation should begin");
            operation.complete("task_persisted");

            let mut task = sample_task(4);
            task.status = DownloadTaskStatus::Error;
            task.error_code = Some("3".to_string());
            task.error_message = Some("forced history failure".to_string());
            sqlx::query(
                "CREATE TRIGGER fail_task_history BEFORE INSERT ON task_history BEGIN SELECT RAISE(FAIL, 'forced task history failure'); END",
            )
            .execute(&database.pool)
            .await
            .expect("failure trigger should create");

            let error = persist_download_task_state_with_operation(
                &database.pool,
                &task,
                &operation,
            )
            .await
            .expect_err("task history failure should roll back the transaction");
            assert!(
                error.contains("forced task history failure"),
                "unexpected persistence error: {error}"
            );

            let task_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM download_tasks WHERE id = 4",
            )
            .fetch_one(&database.pool)
            .await
            .expect("task count should be readable");
            let history_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM task_history WHERE task_id = 4",
            )
            .fetch_one(&database.pool)
            .await
            .expect("history count should be readable");
            let error_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM task_errors WHERE task_id = 4",
            )
            .fetch_one(&database.pool)
            .await
            .expect("error count should be readable");
            let persisted_phase: String = sqlx::query_scalar(
                "SELECT phase FROM task_operations WHERE id = 'operation-history-rollback'",
            )
            .fetch_one(&database.pool)
            .await
            .expect("operation phase should be readable");

            assert_eq!(task_count, 0);
            assert_eq!(history_count, 0);
            assert_eq!(error_count, 0);
            assert_eq!(persisted_phase, "started");

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
}

fn sample_task(id: u64) -> DownloadTask {
    DownloadTask {
        id,
        url: format!("https://example.com/{id}.zip"),
        source_type: DownloadTaskSourceType::Url,
        file_name: format!("{id}.zip"),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some(format!("gid-{id}")),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some(format!("/downloads/{id}.zip")),
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

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_millis()
}
