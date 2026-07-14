use crate::database::settings::{get_app_config_value, set_app_config_value};
use crate::storage::validate_default_download_dir;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

const APP_CONFIG_KEY: &str = "download";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const ENGLISH_LANGUAGE: &str = "en-US";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
    #[serde(default = "default_language")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredAppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub json_rpc_token: String,
}

pub async fn load_app_config_from_pool(
    pool: &SqlitePool,
    default_download_dir: &str,
) -> Result<AppConfig, String> {
    load_stored_app_config(pool, default_download_dir)
        .await
        .map(|config| config.public())
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
    let json_rpc_token = load_stored_app_config(pool, default_download_dir)
        .await?
        .json_rpc_token;
    set_app_config_value(
        pool,
        APP_CONFIG_KEY,
        &StoredAppConfig::from_public(config.clone(), json_rpc_token),
    )
    .await?;
    Ok(config)
}

pub async fn load_json_rpc_token(pool: &SqlitePool) -> Result<String, String> {
    load_stored_app_config(pool, "")
        .await
        .map(|config| config.json_rpc_token)
}

pub async fn save_json_rpc_token(pool: &SqlitePool, token: &str) -> Result<String, String> {
    let mut config = load_stored_app_config(pool, "").await?;
    config.json_rpc_token = token.trim().to_string();
    set_app_config_value(pool, APP_CONFIG_KEY, &config).await?;
    Ok(config.json_rpc_token)
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
        language: normalize_language(&config.language),
    })
}

fn default_app_config(default_download_dir: &str) -> Result<AppConfig, String> {
    Ok(AppConfig {
        default_download_dir: default_download_dir.trim().to_string(),
        max_concurrent_downloads: 5,
        download_limit: 0,
        upload_limit: 0,
        language: default_language(),
    })
}

async fn load_stored_app_config(
    pool: &SqlitePool,
    default_download_dir: &str,
) -> Result<StoredAppConfig, String> {
    match get_app_config_value(pool, APP_CONFIG_KEY).await? {
        Some(config) => normalize_stored_app_config(config, default_download_dir),
        None => Ok(StoredAppConfig::from_public(
            default_app_config(default_download_dir)?,
            String::new(),
        )),
    }
}

fn normalize_stored_app_config(
    config: StoredAppConfig,
    default_download_dir: &str,
) -> Result<StoredAppConfig, String> {
    let json_rpc_token = config.json_rpc_token.trim().to_string();
    let public = normalize_app_config(config.public(), default_download_dir)?;
    Ok(StoredAppConfig::from_public(public, json_rpc_token))
}

impl StoredAppConfig {
    fn public(&self) -> AppConfig {
        AppConfig {
            default_download_dir: self.default_download_dir.clone(),
            max_concurrent_downloads: self.max_concurrent_downloads,
            download_limit: self.download_limit,
            upload_limit: self.upload_limit,
            language: self.language.clone(),
        }
    }

    fn from_public(config: AppConfig, json_rpc_token: String) -> Self {
        Self {
            default_download_dir: config.default_download_dir,
            max_concurrent_downloads: config.max_concurrent_downloads,
            download_limit: config.download_limit,
            upload_limit: config.upload_limit,
            language: config.language,
            json_rpc_token,
        }
    }
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
mod tests;
