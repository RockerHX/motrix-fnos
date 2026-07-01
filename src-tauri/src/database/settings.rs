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

pub async fn get_ui_preference_value<T>(pool: &SqlitePool, key: &str) -> Result<Option<T>, String>
where
    T: DeserializeOwned,
{
    get_json_value(pool, "ui_preferences", key).await
}

pub async fn set_ui_preference_value<T>(
    pool: &SqlitePool,
    key: &str,
    value: &T,
) -> Result<(), String>
where
    T: Serialize,
{
    set_json_value(pool, "ui_preferences", key, value).await
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
        "#
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
mod tests {
    use super::*;
    use crate::database::connect_database;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct SampleConfig {
        value: String,
    }

    #[test]
    fn settings_repository_round_trips_app_config_and_ui_preferences() {
        tauri::async_runtime::block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-settings-test-{}.sqlite",
                current_timestamp_ms()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");
            let value = SampleConfig {
                value: "test".to_string(),
            };

            set_app_config_value(&database.pool, "download", &value)
                .await
                .expect("app config should save");
            set_ui_preference_value(&database.pool, "table", &value)
                .await
                .expect("ui preference should save");

            let app_config: Option<SampleConfig> = get_app_config_value(&database.pool, "download")
                .await
                .expect("app config should read");
            let ui_preference: Option<SampleConfig> =
                get_ui_preference_value(&database.pool, "table")
                    .await
                    .expect("ui preference should read");

            assert_eq!(app_config, Some(value.clone()));
            assert_eq!(ui_preference, Some(value));

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
    }
}
