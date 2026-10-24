use super::download_tasks_column_count;
use sqlx::{Sqlite, Transaction};

pub(super) async fn migrate_engine_state(
    transaction: &mut Transaction<'_, Sqlite>,
    trusted_legacy_schema: bool,
) -> Result<(), String> {
    let new_generation_column =
        download_tasks_column_count(transaction, "engine_generation").await? == 0;
    for (column, definition) in [
        ("engine_generation", "TEXT NOT NULL DEFAULT 'unknown' CHECK (engine_generation IN ('legacy-v1', 'native-v2', 'unknown'))"),
        ("metadata_state", "TEXT NOT NULL DEFAULT 'none'"),
        ("cleanup_state", "TEXT NOT NULL DEFAULT 'none'"),
        ("removed_from_status", "TEXT"),
    ] {
        if download_tasks_column_count(transaction, column).await? == 0 {
            sqlx::query(&format!("ALTER TABLE download_tasks ADD COLUMN {column} {definition}"))
                .execute(&mut **transaction)
                .await
                .map_err(|error| format!("添加引擎状态字段 {column} 失败：{error}"))?;
        }
    }
    if new_generation_column && trusted_legacy_schema {
        sqlx::query("UPDATE download_tasks SET engine_generation = 'legacy-v1'")
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("记录可信旧任务代际失败：{error}"))?;
    }
    for statement in [
        "CREATE TABLE IF NOT EXISTS task_file_snapshots (
            task_id INTEGER NOT NULL REFERENCES download_tasks(id) ON DELETE CASCADE,
            file_index INTEGER NOT NULL, path TEXT NOT NULL, name TEXT NOT NULL,
            length INTEGER NOT NULL, completed_length INTEGER NOT NULL,
            selected INTEGER NOT NULL, checked_at INTEGER NOT NULL,
            PRIMARY KEY (task_id, file_index))",
        "CREATE TABLE IF NOT EXISTS task_metadata_refs (
            task_id INTEGER PRIMARY KEY REFERENCES download_tasks(id) ON DELETE CASCADE,
            engine_generation TEXT NOT NULL, gid TEXT, kind TEXT NOT NULL,
            private_path TEXT NOT NULL, size_bytes INTEGER, checksum TEXT,
            available INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL)",
    ] {
        sqlx::query(statement)
            .execute(&mut **transaction)
            .await
            .map_err(|error| format!("创建任务状态引用失败：{error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
