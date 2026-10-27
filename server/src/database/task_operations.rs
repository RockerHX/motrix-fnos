use super::{i64_to_u64, u64_to_i64};
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
        WHERE status NOT IN ('completed', 'failed')
        ORDER BY created_at, id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|error| format!("读取未完成任务操作失败：{}", error))?;

    rows.into_iter().map(row_to_task_operation).collect()
}

pub(crate) async fn list_unfinished_task_operations_for_engine(
    pool: &SqlitePool,
    generation: crate::config::aria2::EngineGeneration,
) -> Result<Vec<TaskOperation>, String> {
    use crate::config::aria2::EngineGeneration;
    if generation != EngineGeneration::NativeV2 {
        return list_unfinished_task_operations(pool).await;
    }
    let rows = sqlx::query(
        "SELECT task_operations.*, download_tasks.engine_generation AS owner_generation
         FROM task_operations LEFT JOIN download_tasks ON download_tasks.id = task_operations.task_id
         WHERE task_operations.status NOT IN ('completed', 'failed')
         AND (download_tasks.engine_generation IS NULL OR download_tasks.engine_generation != 'legacy-v1')
         ORDER BY task_operations.created_at, task_operations.id",
    ).fetch_all(pool).await.map_err(|error| format!("读取未完成任务操作失败：{error}"))?;
    let mut operations = Vec::new();
    for row in rows {
        let owner: Option<String> = get(&row, "owner_generation")?;
        let context: serde_json::Value =
            serde_json::from_str(&get::<String>(&row, "context_json")?)
                .map_err(|error| format!("读取任务操作上下文失败：{error}"))?;
        let hints = [
            context.get("engineGeneration"),
            context
                .get("taskSnapshot")
                .and_then(|snapshot| snapshot.get("engineGeneration")),
        ];
        let mut identified = owner.as_deref();
        for hint in hints.into_iter().flatten() {
            let hint = hint.as_str().ok_or("任务操作代际无法核对")?;
            if identified.is_some_and(|known| known != hint) {
                return Err("任务操作代际冲突，已保留状态".to_string());
            }
            identified = Some(hint);
        }
        match identified {
            Some("legacy-v1") => continue,
            Some("native-v2") | None => {}
            _ => return Err("任务操作归属无法核对，已保留状态".to_string()),
        }
        let mut operation = row_to_task_operation(row)?;
        if identified.is_none() {
            // 旧契约允许无任务行、无代际字段的合法上下文；损坏记录必须先通过完整解码。
            if operation.engine_generation() == EngineGeneration::LegacyV1 {
                continue;
            }
            return Err("任务操作归属无法核对，已保留状态".to_string());
        }
        operation.context.engine_generation = Some(EngineGeneration::NativeV2);
        operations.push(operation);
    }
    Ok(operations)
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

#[cfg(test)]
mod tests;
