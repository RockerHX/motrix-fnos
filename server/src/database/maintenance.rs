use sqlx::SqlitePool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCleanupReport {
    pub history_count: i64,
    pub error_count: i64,
    pub applied: bool,
}

pub async fn cleanup_history(
    pool: &SqlitePool,
    before_timestamp_ms: i64,
    apply: bool,
) -> Result<HistoryCleanupReport, String> {
    if before_timestamp_ms < 0 {
        return Err("清理时间必须是非负毫秒时间戳".to_string());
    }

    if !apply {
        let history_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM task_history WHERE created_at < ?")
                .bind(before_timestamp_ms)
                .fetch_one(pool)
                .await
                .map_err(|error| format!("统计任务历史失败：{}", error))?;
        let error_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM task_errors WHERE created_at < ?")
                .bind(before_timestamp_ms)
                .fetch_one(pool)
                .await
                .map_err(|error| format!("统计任务错误记录失败：{}", error))?;
        return Ok(HistoryCleanupReport {
            history_count,
            error_count,
            applied: false,
        });
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动历史记录清理事务失败：{}", error))?;
    let history_deleted = sqlx::query("DELETE FROM task_history WHERE created_at < ?")
        .bind(before_timestamp_ms)
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("删除任务历史失败：{}", error))?
        .rows_affected() as i64;
    let errors_deleted = sqlx::query("DELETE FROM task_errors WHERE created_at < ?")
        .bind(before_timestamp_ms)
        .execute(&mut *transaction)
        .await
        .map_err(|error| format!("删除任务错误记录失败：{}", error))?
        .rows_affected() as i64;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交历史记录清理事务失败：{}", error))?;

    Ok(HistoryCleanupReport {
        history_count: history_deleted,
        error_count: errors_deleted,
        applied: true,
    })
}

#[cfg(test)]
mod tests;
