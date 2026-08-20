use super::*;
use crate::database::connect_database;
use sqlx::SqlitePool;
use std::path::PathBuf;

fn test_database_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-{name}-{}.sqlite",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}

async fn insert_history_and_errors(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO task_history (task_id, status, message, created_at) VALUES (1, 'complete', 'old', 100), (1, 'paused', 'boundary', 200)",
    )
    .execute(pool)
    .await
    .expect("history rows should insert");
    sqlx::query(
        "INSERT INTO task_errors (task_id, error_code, error_message, created_at) VALUES (1, 'old', 'old', 100), (1, 'boundary', 'boundary', 200)",
    )
    .execute(pool)
    .await
    .expect("error rows should insert");
}

#[tokio::test]
async fn cleanup_history_defaults_to_dry_run_and_respects_boundary() {
    let path = test_database_path("history-dry-run");
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    insert_history_and_errors(&database.pool).await;
    sqlx::query(
        "INSERT INTO download_tasks (id, url, file_name, save_dir, status, created_at, updated_at) VALUES (9, 'https://example.com/file', 'file', '/downloads', 'complete', 100, 100)",
    )
    .execute(&database.pool)
    .await
    .expect("task should insert");
    sqlx::query(
        "INSERT INTO task_operations (id, task_id, operation_type, phase, context_json, status, created_at, updated_at) VALUES ('operation-9', 9, 'create', 'done', '{}', 'completed', 100, 100)",
    )
    .execute(&database.pool)
    .await
    .expect("operation should insert");
    let user_file = path.with_file_name("history-dry-run-user-file.txt");
    std::fs::write(&user_file, b"keep").expect("user file should write");

    let preview = cleanup_history(&database.pool, 200, false)
        .await
        .expect("dry-run should succeed");
    assert_eq!(
        preview,
        HistoryCleanupReport {
            history_count: 1,
            error_count: 1,
            applied: false,
        }
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_history")
            .fetch_one(&database.pool)
            .await
            .expect("history count should read"),
        2
    );

    let applied = cleanup_history(&database.pool, 200, true)
        .await
        .expect("apply should succeed");
    assert_eq!(applied.history_count, 1);
    assert_eq!(applied.error_count, 1);
    assert!(applied.applied);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_history")
            .fetch_one(&database.pool)
            .await
            .expect("history count should read"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_errors")
            .fetch_one(&database.pool)
            .await
            .expect("error count should read"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM download_tasks")
            .fetch_one(&database.pool)
            .await
            .expect("task count should read"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_operations")
            .fetch_one(&database.pool)
            .await
            .expect("operation count should read"),
        1
    );
    assert!(user_file.is_file());

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(user_file);
}

#[tokio::test]
async fn cleanup_history_rolls_back_when_error_delete_fails() {
    let path = test_database_path("history-rollback");
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    insert_history_and_errors(&database.pool).await;
    sqlx::query(
        "CREATE TRIGGER fail_task_error_cleanup BEFORE DELETE ON task_errors BEGIN SELECT RAISE(ABORT, 'test cleanup failure'); END",
    )
    .execute(&database.pool)
    .await
    .expect("cleanup trigger should create");

    let error = cleanup_history(&database.pool, 200, true)
        .await
        .expect_err("cleanup should fail");
    assert!(error.contains("删除任务错误记录失败"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_history")
            .fetch_one(&database.pool)
            .await
            .expect("history count should read"),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM task_errors")
            .fetch_one(&database.pool)
            .await
            .expect("error count should read"),
        2
    );

    sqlx::query("DROP TRIGGER fail_task_error_cleanup")
        .execute(&database.pool)
        .await
        .expect("cleanup trigger should drop");
    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}
