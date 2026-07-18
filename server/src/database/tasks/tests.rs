use super::*;
use crate::database::connect_database;

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

fn sample_task() -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/file.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "file.zip".to_string(),
        save_dir: "/downloads".to_string(),
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
