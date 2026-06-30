use crate::tasks::{DownloadTask, DownloadTaskStatus};
use sqlx::{Decode, Row, Sqlite, SqlitePool, Type};

pub async fn upsert_download_task(pool: &SqlitePool, task: &DownloadTask) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO download_tasks (
            id, url, file_name, save_dir, gid, status, total_length, completed_length,
            download_speed, error_code, error_message, file_path, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            file_name = excluded.file_name,
            save_dir = excluded.save_dir,
            gid = excluded.gid,
            status = excluded.status,
            total_length = excluded.total_length,
            completed_length = excluded.completed_length,
            download_speed = excluded.download_speed,
            error_code = excluded.error_code,
            error_message = excluded.error_message,
            file_path = excluded.file_path,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.url)
    .bind(&task.file_name)
    .bind(&task.save_dir)
    .bind(&task.gid)
    .bind(task.status.as_storage_value())
    .bind(u64_to_i64(task.total_length, "总大小")?)
    .bind(u64_to_i64(task.completed_length, "已下载大小")?)
    .bind(u64_to_i64(task.download_speed, "下载速度")?)
    .bind(&task.error_code)
    .bind(&task.error_message)
    .bind(&task.file_path)
    .bind(u64_to_i64(task.created_at, "创建时间")?)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .execute(pool)
    .await
    .map_err(|error| format!("保存下载任务失败：{}", error))?;

    Ok(())
}

pub async fn list_download_tasks(pool: &SqlitePool) -> Result<Vec<DownloadTask>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, url, file_name, save_dir, gid, status, total_length, completed_length,
               download_speed, error_code, error_message, file_path, created_at, updated_at
        FROM download_tasks
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取下载任务失败：{}", error))?;

    rows.into_iter().map(row_to_task).collect()
}

pub async fn max_download_task_id(pool: &SqlitePool) -> Result<u64, String> {
    let max_id: Option<i64> = sqlx::query_scalar("SELECT MAX(id) FROM download_tasks")
        .fetch_one(pool)
        .await
        .map_err(|error| format!("读取最大任务 ID 失败：{}", error))?;

    Ok(max_id.unwrap_or_default().max(0) as u64)
}

pub async fn record_task_history(
    pool: &SqlitePool,
    task: &DownloadTask,
    message: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO task_history (task_id, status, message, created_at)
        SELECT ?, ?, ?, ?
        WHERE NOT EXISTS (
            SELECT 1
            FROM task_history
            WHERE task_id = ?
              AND status = ?
            ORDER BY created_at DESC
            LIMIT 1
        )
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(task.status.as_storage_value())
    .bind(message)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(task.status.as_storage_value())
    .execute(pool)
    .await
    .map_err(|error| format!("保存任务历史失败：{}", error))?;

    Ok(())
}

pub async fn record_task_error(pool: &SqlitePool, task: &DownloadTask) -> Result<(), String> {
    let Some(message) = task.error_message.as_deref().filter(|message| !message.trim().is_empty())
    else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO task_errors (task_id, error_code, error_message, created_at)
        SELECT ?, ?, ?, ?
        WHERE NOT EXISTS (
            SELECT 1
            FROM task_errors
            WHERE task_id = ?
              AND COALESCE(error_code, '') = COALESCE(?, '')
              AND error_message = ?
            LIMIT 1
        )
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.error_code)
    .bind(message)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.error_code)
    .bind(message)
    .execute(pool)
    .await
    .map_err(|error| format!("保存任务错误记录失败：{}", error))?;

    Ok(())
}

fn row_to_task(row: sqlx::sqlite::SqliteRow) -> Result<DownloadTask, String> {
    let status: String = get(&row, "status")?;
    Ok(DownloadTask {
        id: i64_to_u64(get(&row, "id")?, "任务 ID")?,
        url: get(&row, "url")?,
        file_name: get(&row, "file_name")?,
        save_dir: get(&row, "save_dir")?,
        gid: get(&row, "gid")?,
        status: DownloadTaskStatus::from_storage_value(&status),
        total_length: i64_to_u64(get(&row, "total_length")?, "总大小")?,
        completed_length: i64_to_u64(get(&row, "completed_length")?, "已下载大小")?,
        download_speed: i64_to_u64(get(&row, "download_speed")?, "下载速度")?,
        error_code: get(&row, "error_code")?,
        error_message: get(&row, "error_message")?,
        file_path: get(&row, "file_path")?,
        created_at: i64_to_u64(get(&row, "created_at")?, "创建时间")?,
        updated_at: i64_to_u64(get(&row, "updated_at")?, "更新时间")?,
    })
}

fn get<'row, T>(row: &'row sqlx::sqlite::SqliteRow, column: &str) -> Result<T, String>
where
    T: Decode<'row, Sqlite> + Type<Sqlite>,
{
    row.try_get(column)
        .map_err(|error| format!("读取下载任务字段 {} 失败：{}", column, error))
}

fn u64_to_i64(value: u64, label: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{} 超出 SQLite INTEGER 范围", label))
}

fn i64_to_u64(value: i64, label: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{} 不能为负数", label))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::connect_database;

    #[test]
    fn repository_inserts_updates_and_lists_tasks() {
        tauri::async_runtime::block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-repository-test-{}.sqlite",
                now_ms()
            ));
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
            assert_eq!(max_id, task.id);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
    }

    #[test]
    fn repository_records_history_and_error() {
        tauri::async_runtime::block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-history-test-{}.sqlite",
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

    fn sample_task() -> DownloadTask {
        DownloadTask {
            id: 1,
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            gid: Some("abc123".to_string()),
            status: DownloadTaskStatus::Active,
            total_length: 100,
            completed_length: 40,
            download_speed: 20,
            error_code: None,
            error_message: None,
            file_path: Some("/downloads/file.zip".to_string()),
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
}
