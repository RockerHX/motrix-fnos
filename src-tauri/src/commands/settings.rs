use crate::app::AppState;
use crate::database::settings::{
    get_app_config_value, get_ui_preference_value, set_app_config_value, set_ui_preference_value,
};
use crate::tasks::default_download_dir_string;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use tauri::State;

const APP_CONFIG_KEY: &str = "download";
const UI_PREFERENCES_KEY: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferences {
    pub task_table_column_widths: BTreeMap<String, u32>,
}

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = load_app_config_from_pool(&state.database.pool).await?;
    state.debug_logs.info("settings", "读取应用配置");
    Ok(config)
}

#[tauri::command]
pub async fn save_app_config(
    state: State<'_, AppState>,
    payload: AppConfig,
) -> Result<AppConfig, String> {
    let config = normalize_app_config(payload)?;
    set_app_config_value(&state.database.pool, APP_CONFIG_KEY, &config).await?;
    state.debug_logs.info("settings", "应用配置已保存");
    Ok(config)
}

#[tauri::command]
pub async fn get_ui_preferences(state: State<'_, AppState>) -> Result<UiPreferences, String> {
    let preferences = load_ui_preferences_from_pool(&state.database.pool).await?;
    state.debug_logs.info("settings", "读取 UI 偏好");
    Ok(preferences)
}

#[tauri::command]
pub async fn save_ui_preferences(
    state: State<'_, AppState>,
    payload: UiPreferences,
) -> Result<UiPreferences, String> {
    set_ui_preference_value(&state.database.pool, UI_PREFERENCES_KEY, &payload).await?;
    state.debug_logs.info("settings", "UI 偏好已保存");
    Ok(payload)
}

pub async fn load_app_config_from_pool(pool: &SqlitePool) -> Result<AppConfig, String> {
    match get_app_config_value(pool, APP_CONFIG_KEY).await? {
        Some(config) => normalize_app_config(config),
        None => default_app_config(),
    }
}

pub async fn load_ui_preferences_from_pool(pool: &SqlitePool) -> Result<UiPreferences, String> {
    Ok(get_ui_preference_value(pool, UI_PREFERENCES_KEY)
        .await?
        .unwrap_or_default())
}

fn default_app_config() -> Result<AppConfig, String> {
    Ok(AppConfig {
        default_download_dir: default_download_dir_string()?,
        max_concurrent_downloads: 5,
        download_limit: 0,
        upload_limit: 0,
    })
}

fn normalize_app_config(config: AppConfig) -> Result<AppConfig, String> {
    let default_download_dir = if config.default_download_dir.trim().is_empty() {
        default_download_dir_string()?
    } else {
        config.default_download_dir.trim().to_string()
    };

    Ok(AppConfig {
        default_download_dir,
        max_concurrent_downloads: config.max_concurrent_downloads.max(1),
        download_limit: config.download_limit,
        upload_limit: config.upload_limit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::connect_database;

    #[test]
    fn app_config_uses_defaults_and_round_trips_saved_values() {
        tauri::async_runtime::block_on(async {
            let path = std::env::temp_dir().join(format!(
                "motrix-fnos-app-config-test-{}.sqlite",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("system time should be valid")
                    .as_millis()
            ));
            let database = connect_database(path.clone())
                .await
                .expect("database should connect");

            let default_config = load_app_config_from_pool(&database.pool)
                .await
                .expect("default config should load");
            assert!(default_config.default_download_dir.ends_with("Downloads"));

            let saved = normalize_app_config(AppConfig {
                default_download_dir: "/tmp/downloads".to_string(),
                max_concurrent_downloads: 0,
                download_limit: 1024,
                upload_limit: 2048,
            })
            .expect("config should normalize");
            set_app_config_value(&database.pool, APP_CONFIG_KEY, &saved)
                .await
                .expect("config should save");

            let loaded = load_app_config_from_pool(&database.pool)
                .await
                .expect("config should load");
            assert_eq!(loaded.default_download_dir, "/tmp/downloads");
            assert_eq!(loaded.max_concurrent_downloads, 1);
            assert_eq!(loaded.download_limit, 1024);
            assert_eq!(loaded.upload_limit, 2048);

            database.pool.close().await;
            let _ = std::fs::remove_file(path);
        });
    }
}
