use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn get_app_config_value<T>(pool: &SqlitePool, key: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    get_json_value(pool, "app_config", key).await
}

pub async fn set_app_config_value<T>(pool: &SqlitePool, key: &str, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    set_json_value(pool, "app_config", key, value).await
}

async fn get_json_value<T>(pool: &SqlitePool, table: &str, key: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    let value: Option<String> =
        sqlx::query_scalar(&format!("SELECT value FROM {table} WHERE key = ?"))
            .bind(key)
            .fetch_optional(pool)
            .await
            .map_err(|error| format!("读取配置失败：{}", error))?;

    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| format!("解析配置失败：{}", error))
        })
        .transpose()
}

async fn set_json_value<T>(
    pool: &SqlitePool,
    table: &str,
    key: &str,
    value: &T,
) -> Result<(), String>
where
    T: Serialize,
{
    let value =
        serde_json::to_string(value).map_err(|error| format!("序列化配置失败：{}", error))?;
    sqlx::query(&format!(
        r#"
        INSERT INTO {table} (key, value, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = excluded.updated_at
        "#,
    ))
    .bind(key)
    .bind(value)
    .bind(current_timestamp_ms() as i64)
    .execute(pool)
    .await
    .map_err(|error| format!("保存配置失败：{}", error))?;

    Ok(())
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
