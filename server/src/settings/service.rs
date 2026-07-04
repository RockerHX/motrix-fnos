use crate::database::settings::{
    get_app_config_value, get_ui_preference_value, set_app_config_value, set_ui_preference_value,
};
use crate::storage::validate_default_download_dir;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::BTreeMap;
use std::path::Path;

const APP_CONFIG_KEY: &str = "download";
const UI_PREFERENCES_KEY: &str = "main";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const ENGLISH_LANGUAGE: &str = "en-US";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
    #[serde(default)]
    pub auto_start_enabled: bool,
    #[serde(default)]
    pub notifications_enabled: bool,
    #[serde(default = "default_language")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferences {
    pub task_table_column_widths: BTreeMap<String, u32>,
}

pub async fn load_app_config_from_pool(
    pool: &SqlitePool,
    default_download_dir: &str,
) -> Result<AppConfig, String> {
    match get_app_config_value(pool, APP_CONFIG_KEY).await? {
        Some(config) => normalize_app_config(config, default_download_dir),
        None => default_app_config(default_download_dir),
    }
}

pub async fn save_app_config(
    pool: &SqlitePool,
    payload: AppConfig,
    default_download_dir: &str,
    accessible_paths: &[String],
    app_data_dir: &Path,
) -> Result<AppConfig, String> {
    let config = normalize_app_config(payload, default_download_dir)?;
    validate_default_download_dir(&config.default_download_dir, accessible_paths, app_data_dir)?;
    set_app_config_value(pool, APP_CONFIG_KEY, &config).await?;
    Ok(config)
}

pub async fn load_ui_preferences_from_pool(pool: &SqlitePool) -> Result<UiPreferences, String> {
    Ok(get_ui_preference_value(pool, UI_PREFERENCES_KEY)
        .await?
        .unwrap_or_default())
}

pub async fn save_ui_preferences(
    pool: &SqlitePool,
    payload: UiPreferences,
) -> Result<UiPreferences, String> {
    set_ui_preference_value(pool, UI_PREFERENCES_KEY, &payload).await?;
    Ok(payload)
}

pub fn normalize_app_config(
    config: AppConfig,
    default_download_dir: &str,
) -> Result<AppConfig, String> {
    let default_download_dir = if config.default_download_dir.trim().is_empty() {
        default_download_dir.trim().to_string()
    } else {
        config.default_download_dir.trim().to_string()
    };

    Ok(AppConfig {
        default_download_dir,
        max_concurrent_downloads: config.max_concurrent_downloads.clamp(1, 64),
        download_limit: config.download_limit,
        upload_limit: config.upload_limit,
        auto_start_enabled: config.auto_start_enabled,
        notifications_enabled: config.notifications_enabled,
        language: normalize_language(&config.language),
    })
}

fn default_app_config(default_download_dir: &str) -> Result<AppConfig, String> {
    Ok(AppConfig {
        default_download_dir: default_download_dir.trim().to_string(),
        max_concurrent_downloads: 5,
        download_limit: 0,
        upload_limit: 0,
        auto_start_enabled: false,
        notifications_enabled: false,
        language: default_language(),
    })
}

fn default_language() -> String {
    DEFAULT_LANGUAGE.to_string()
}

fn normalize_language(language: &str) -> String {
    match language.trim() {
        DEFAULT_LANGUAGE => DEFAULT_LANGUAGE.to_string(),
        ENGLISH_LANGUAGE => ENGLISH_LANGUAGE.to_string(),
        _ => default_language(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::connect_database;

    #[test]
    fn app_config_uses_defaults_and_round_trips_saved_values() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
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

                let default_config = load_app_config_from_pool(&database.pool, "/app/data")
                    .await
                    .expect("default config should load");
                assert_eq!(default_config.default_download_dir, "/app/data");

                let saved = normalize_app_config(
                    AppConfig {
                        default_download_dir: "/tmp/downloads".to_string(),
                        max_concurrent_downloads: 0,
                        download_limit: 1024,
                        upload_limit: 2048,
                        auto_start_enabled: true,
                        notifications_enabled: true,
                        language: "en-US".to_string(),
                    },
                    "/app/data",
                )
                .expect("config should normalize");
                save_app_config(
                    &database.pool,
                    saved,
                    "/app/data",
                    &["/tmp/downloads".to_string()],
                    std::path::Path::new("/app/data"),
                )
                .await
                .expect("config should save");

                let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                    .await
                    .expect("config should load");
                assert_eq!(loaded.default_download_dir, "/tmp/downloads");
                assert_eq!(loaded.max_concurrent_downloads, 1);
                assert_eq!(loaded.download_limit, 1024);
                assert_eq!(loaded.upload_limit, 2048);
                assert!(loaded.auto_start_enabled);
                assert!(loaded.notifications_enabled);
                assert_eq!(loaded.language, "en-US");

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }

    #[test]
    fn app_config_rejects_unauthorized_default_download_dir() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let path = std::env::temp_dir().join(format!(
                    "motrix-fnos-unauthorized-app-config-test-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system time should be valid")
                        .as_nanos()
                ));
                let database = connect_database(path.clone())
                    .await
                    .expect("database should connect");

                let error = save_app_config(
                    &database.pool,
                    AppConfig {
                        default_download_dir: "/tmp/downloads".to_string(),
                        max_concurrent_downloads: 5,
                        download_limit: 0,
                        upload_limit: 0,
                        auto_start_enabled: false,
                        notifications_enabled: false,
                        language: "zh-CN".to_string(),
                    },
                    "/app/data",
                    &["/app/data".to_string()],
                    std::path::Path::new("/app/data"),
                )
                .await
                .expect_err("unauthorized directory should fail");

                assert_eq!(error, "默认下载目录不在已授权目录列表中");

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }

    #[test]
    fn app_config_accepts_legacy_saved_values() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime should create")
            .block_on(async {
                let path = std::env::temp_dir().join(format!(
                    "motrix-fnos-legacy-app-config-test-{}.sqlite",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("system time should be valid")
                        .as_millis()
                ));
                let database = connect_database(path.clone())
                    .await
                    .expect("database should connect");

                sqlx::query(
                    r#"
                    INSERT INTO app_config (key, value, updated_at)
                    VALUES ('download', '{"defaultDownloadDir":"/tmp/downloads","maxConcurrentDownloads":128,"downloadLimit":0,"uploadLimit":0}', 1)
                    "#,
                )
                .execute(&database.pool)
                .await
                .expect("legacy config should insert");

                let loaded = load_app_config_from_pool(&database.pool, "/app/data")
                    .await
                    .expect("legacy config should load");

                assert_eq!(loaded.default_download_dir, "/tmp/downloads");
                assert_eq!(loaded.max_concurrent_downloads, 64);
                assert!(!loaded.auto_start_enabled);
                assert!(!loaded.notifications_enabled);
                assert_eq!(loaded.language, "zh-CN");

                database.pool.close().await;
                let _ = std::fs::remove_file(path);
            });
    }

    #[test]
    fn app_config_falls_back_to_default_language_for_invalid_values() {
        let config = normalize_app_config(
            AppConfig {
                default_download_dir: "/tmp/downloads".to_string(),
                max_concurrent_downloads: 5,
                download_limit: 0,
                upload_limit: 0,
                auto_start_enabled: false,
                notifications_enabled: false,
                language: "fr-FR".to_string(),
            },
            "/app/data",
        )
        .expect("config should normalize");

        assert_eq!(config.language, "zh-CN");
    }
}
