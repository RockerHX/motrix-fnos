use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
use sqlx::{Decode, Row, Sqlite, SqlitePool, Type};

pub async fn upsert_download_task(pool: &SqlitePool, task: &DownloadTask) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO download_tasks (
            id, url, source_type, file_name, save_dir, category, gid, status, total_length, completed_length,
            download_speed, error_code, error_message, file_path, metadata_torrent_path, files_deleted,
            selected_file_indexes, confirmation_required, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            source_type = excluded.source_type,
            file_name = excluded.file_name,
            save_dir = excluded.save_dir,
            category = excluded.category,
            gid = excluded.gid,
            status = excluded.status,
            total_length = excluded.total_length,
            completed_length = excluded.completed_length,
            download_speed = excluded.download_speed,
            error_code = excluded.error_code,
            error_message = excluded.error_message,
            file_path = excluded.file_path,
            metadata_torrent_path = excluded.metadata_torrent_path,
            files_deleted = excluded.files_deleted,
            selected_file_indexes = excluded.selected_file_indexes,
            confirmation_required = excluded.confirmation_required,
            updated_at = excluded.updated_at
        "#,
    )
    .bind(u64_to_i64(task.id, "任务 ID")?)
    .bind(&task.url)
    .bind(task.source_type.as_storage_value())
    .bind(&task.file_name)
    .bind(&task.save_dir)
    .bind(&task.category)
    .bind(&task.gid)
    .bind(task.status.as_storage_value())
    .bind(u64_to_i64(task.total_length, "总大小")?)
    .bind(u64_to_i64(task.completed_length, "已下载大小")?)
    .bind(u64_to_i64(task.download_speed, "下载速度")?)
    .bind(&task.error_code)
    .bind(&task.error_message)
    .bind(&task.file_path)
    .bind(&task.metadata_torrent_path)
    .bind(if task.files_deleted { 1_i64 } else { 0_i64 })
    .bind(serde_json::to_string(&task.selected_file_indexes).map_err(|error| {
        format!("序列化任务文件选择失败：{}", error)
    })?)
    .bind(if task.confirmation_required { 1_i64 } else { 0_i64 })
    .bind(u64_to_i64(task.created_at, "创建时间")?)
    .bind(u64_to_i64(task.updated_at, "更新时间")?)
    .execute(pool)
    .await
    .map_err(|error| format!("保存下载任务失败：{}", error))?;

    Ok(())
}

pub async fn persist_download_task_state(
    pool: &SqlitePool,
    task: &DownloadTask,
) -> Result<(), String> {
    upsert_download_task(pool, task).await?;

    match task.status {
        DownloadTaskStatus::Complete
        | DownloadTaskStatus::Paused
        | DownloadTaskStatus::Error
        | DownloadTaskStatus::Removed => {
            record_task_history(pool, task, task.error_message.as_deref()).await?;
        }
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active => {}
    }

    if task.status == DownloadTaskStatus::Error {
        record_task_error(pool, task).await?;
    }

    Ok(())
}

pub async fn persist_download_task_states(
    pool: &SqlitePool,
    tasks: &[DownloadTask],
) -> Result<(), String> {
    for task in tasks {
        persist_download_task_state(pool, task).await?;
    }

    Ok(())
}

pub async fn list_download_tasks(pool: &SqlitePool) -> Result<Vec<DownloadTask>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, url, source_type, file_name, save_dir, gid, status, total_length, completed_length,
               category, download_speed, error_code, error_message, file_path,
               metadata_torrent_path, files_deleted, selected_file_indexes, confirmation_required,
               created_at, updated_at
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

pub async fn delete_download_task_record(pool: &SqlitePool, task_id: u64) -> Result<bool, String> {
    let task_id = u64_to_i64(task_id, "任务 ID")?;

    sqlx::query("DELETE FROM task_history WHERE task_id = ?")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|error| format!("删除任务历史失败：{}", error))?;
    sqlx::query("DELETE FROM task_errors WHERE task_id = ?")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|error| format!("删除任务错误记录失败：{}", error))?;
    let result = sqlx::query("DELETE FROM download_tasks WHERE id = ?")
        .bind(task_id)
        .execute(pool)
        .await
        .map_err(|error| format!("删除下载任务记录失败：{}", error))?;

    Ok(result.rows_affected() > 0)
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
    let Some(message) = task
        .error_message
        .as_deref()
        .filter(|message| !message.trim().is_empty())
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
    let source_type: String = get(&row, "source_type")?;
    Ok(DownloadTask {
        id: i64_to_u64(get(&row, "id")?, "任务 ID")?,
        url: get(&row, "url")?,
        source_type: DownloadTaskSourceType::from_storage_value(&source_type),
        file_name: get(&row, "file_name")?,
        save_dir: get(&row, "save_dir")?,
        category: get(&row, "category")?,
        gid: get(&row, "gid")?,
        status: DownloadTaskStatus::from_storage_value(&status),
        total_length: i64_to_u64(get(&row, "total_length")?, "总大小")?,
        completed_length: i64_to_u64(get(&row, "completed_length")?, "已下载大小")?,
        download_speed: i64_to_u64(get(&row, "download_speed")?, "下载速度")?,
        error_code: get(&row, "error_code")?,
        error_message: get(&row, "error_message")?,
        file_path: get(&row, "file_path")?,
        metadata_torrent_path: get(&row, "metadata_torrent_path")?,
        files_deleted: get::<i64>(&row, "files_deleted")? != 0,
        selected_file_indexes: serde_json::from_str(&get::<String>(&row, "selected_file_indexes")?)
            .map_err(|error| format!("读取任务文件选择字段失败：{}", error))?,
        confirmation_required: get::<i64>(&row, "confirmation_required")? != 0,
        files: Vec::new(),
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
mod tests;
