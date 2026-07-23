use crate::tasks::{TaskOperation, TaskOperationContext, TaskOperationStatus, TaskOperationType};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

pub async fn begin_task_operation(
    pool: &SqlitePool,
    operation: &TaskOperation,
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动创建任务操作事务失败：{}", error))?;
    insert_task_operation_in_transaction(&mut transaction, operation).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交创建任务操作事务失败：{}", error))?;
    Ok(())
}

pub async fn update_task_operation(
    pool: &SqlitePool,
    operation: &TaskOperation,
) -> Result<(), String> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| format!("启动更新任务操作事务失败：{}", error))?;
    update_task_operation_in_transaction(&mut transaction, operation).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交更新任务操作事务失败：{}", error))?;
    Ok(())
}

pub async fn list_unfinished_task_operations(
    pool: &SqlitePool,
) -> Result<Vec<TaskOperation>, String> {
    let rows = sqlx::query(
        r#"
        SELECT id, task_id, operation_type, phase, context_json, error_message, status,
               created_at, updated_at
        FROM task_operations
        WHERE status IN ('in_progress', 'manual_review')
        ORDER BY created_at, id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取未完成任务操作失败：{}", error))?;

    rows.into_iter().map(row_to_task_operation).collect()
}

pub(crate) async fn insert_task_operation_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    operation: &TaskOperation,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO task_operations (
            id, task_id, operation_type, phase, context_json, error_message, status,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&operation.id)
    .bind(u64_to_i64(operation.task_id, "任务操作任务 ID")?)
    .bind(operation.operation_type.as_storage_value())
    .bind(&operation.phase)
    .bind(serialize_context(&operation.context)?)
    .bind(&operation.error_message)
    .bind(operation.status.as_storage_value())
    .bind(u64_to_i64(operation.created_at, "任务操作创建时间")?)
    .bind(u64_to_i64(operation.updated_at, "任务操作更新时间")?)
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("创建任务操作记录失败：{}", error))?;
    Ok(())
}

pub(crate) async fn update_task_operation_in_transaction(
    transaction: &mut Transaction<'_, Sqlite>,
    operation: &TaskOperation,
) -> Result<(), String> {
    let result = sqlx::query(
        r#"
        UPDATE task_operations
        SET phase = ?, context_json = ?, error_message = ?, status = ?, updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(&operation.phase)
    .bind(serialize_context(&operation.context)?)
    .bind(&operation.error_message)
    .bind(operation.status.as_storage_value())
    .bind(u64_to_i64(operation.updated_at, "任务操作更新时间")?)
    .bind(&operation.id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| format!("更新任务操作记录失败：{}", error))?;
    if result.rows_affected() != 1 {
        return Err(format!("任务操作不存在：{}", operation.id));
    }
    Ok(())
}

fn row_to_task_operation(row: sqlx::sqlite::SqliteRow) -> Result<TaskOperation, String> {
    let operation_type: String = get(&row, "operation_type")?;
    let status: String = get(&row, "status")?;
    let context_json: String = get(&row, "context_json")?;
    let context = serde_json::from_str(&context_json)
        .map_err(|error| format!("读取任务操作上下文失败：{}", error))?;
    Ok(TaskOperation {
        id: get(&row, "id")?,
        task_id: i64_to_u64(get(&row, "task_id")?, "任务操作任务 ID")?,
        operation_type: TaskOperationType::from_storage_value(&operation_type)?,
        phase: get(&row, "phase")?,
        context,
        error_message: get(&row, "error_message")?,
        status: TaskOperationStatus::from_storage_value(&status)?,
        created_at: i64_to_u64(get(&row, "created_at")?, "任务操作创建时间")?,
        updated_at: i64_to_u64(get(&row, "updated_at")?, "任务操作更新时间")?,
    })
}

fn serialize_context(context: &TaskOperationContext) -> Result<String, String> {
    serde_json::to_string(context).map_err(|error| format!("序列化任务操作上下文失败：{}", error))
}

fn get<'row, T>(row: &'row sqlx::sqlite::SqliteRow, column: &str) -> Result<T, String>
where
    T: sqlx::Decode<'row, Sqlite> + sqlx::Type<Sqlite>,
{
    row.try_get(column)
        .map_err(|error| format!("读取任务操作字段 {} 失败：{}", column, error))
}

fn u64_to_i64(value: u64, label: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{} 超出 SQLite INTEGER 范围", label))
}

fn i64_to_u64(value: i64, label: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{} 不能为负数", label))
}

#[cfg(test)]
mod tests;
