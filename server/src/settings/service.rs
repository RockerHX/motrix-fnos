use crate::database::settings::{get_app_config_value, set_app_config_value};
use crate::storage::validate_default_download_dir;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

const APP_CONFIG_KEY: &str = "download";
const LAN_JSONRPC_CONFIG_KEY: &str = "jsonrpc_lan";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const ENGLISH_LANGUAGE: &str = "en-US";
pub const DEFAULT_MAX_CONCURRENT_DOWNLOADS: u32 = 5;
pub const MAX_CONCURRENT_DOWNLOADS_LIMIT: u32 = 128;
pub const DEFAULT_MAX_CONNECTION_PER_SERVER: u32 = 1;
pub const MAX_CONNECTION_PER_SERVER_LIMIT: u32 = 64;
pub const DEFAULT_SPLIT: u32 = 5;
pub const MAX_SPLIT_LIMIT: u32 = 64;
pub const MIN_SPLIT_SIZE_OPTIONS: [&str; 4] = ["1M", "5M", "10M", "20M"];
pub const DEFAULT_MIN_SPLIT_SIZE: &str = "20M";
pub const DEFAULT_CONNECT_TIMEOUT: u32 = 60;
pub const MAX_CONNECT_TIMEOUT_LIMIT: u32 = 300;
pub const DEFAULT_MAX_TRIES: u32 = 5;
pub const MAX_TRIES_LIMIT: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    #[serde(default = "default_max_connection_per_server")]
    pub max_connection_per_server: u32,
    #[serde(default = "default_split")]
    pub split: u32,
    #[serde(default = "default_min_split_size")]
    pub min_split_size: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u32,
    #[serde(default = "default_max_tries")]
    pub max_tries: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_download_dir: String::new(),
            max_concurrent_downloads: DEFAULT_MAX_CONCURRENT_DOWNLOADS,
            max_connection_per_server: default_max_connection_per_server(),
            split: default_split(),
            min_split_size: default_min_split_size(),
            connect_timeout: default_connect_timeout(),
            max_tries: default_max_tries(),
            download_limit: 0,
            upload_limit: 0,
            language: default_language(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanJsonRpcConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredAppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    #[serde(default = "default_max_connection_per_server")]
    pub max_connection_per_server: u32,
    #[serde(default = "default_split")]
    pub split: u32,
    #[serde(default = "default_min_split_size")]
    pub min_split_size: String,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout: u32,
    #[serde(default = "default_max_tries")]
    pub max_tries: u32,
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

pub async fn load_lan_json_rpc_config(pool: &SqlitePool) -> Result<LanJsonRpcConfig, String> {
    let config = get_app_config_value(pool, LAN_JSONRPC_CONFIG_KEY)
        .await?
        .unwrap_or_default();
    Ok(normalize_lan_json_rpc_config(config))
}

pub async fn save_lan_json_rpc_config(
    pool: &SqlitePool,
    config: &LanJsonRpcConfig,
) -> Result<LanJsonRpcConfig, String> {
    let config = normalize_lan_json_rpc_config(config.clone());
    set_app_config_value(pool, LAN_JSONRPC_CONFIG_KEY, &config).await?;
    Ok(config)
}

fn normalize_lan_json_rpc_config(mut config: LanJsonRpcConfig) -> LanJsonRpcConfig {
    config.token = config.token.trim().to_string();
    config
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
        max_concurrent_downloads: config
            .max_concurrent_downloads
            .clamp(1, MAX_CONCURRENT_DOWNLOADS_LIMIT),
        max_connection_per_server: config
            .max_connection_per_server
            .clamp(1, MAX_CONNECTION_PER_SERVER_LIMIT),
        split: config.split.clamp(1, MAX_SPLIT_LIMIT),
        min_split_size: normalize_min_split_size(&config.min_split_size),
        connect_timeout: config.connect_timeout.clamp(1, MAX_CONNECT_TIMEOUT_LIMIT),
        max_tries: config.max_tries.clamp(1, MAX_TRIES_LIMIT),
        download_limit: config.download_limit,
        upload_limit: config.upload_limit,
        language: normalize_language(&config.language),
    })
}

fn default_app_config(default_download_dir: &str) -> Result<AppConfig, String> {
    Ok(AppConfig {
        default_download_dir: default_download_dir.trim().to_string(),
        ..AppConfig::default()
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
            max_connection_per_server: self.max_connection_per_server,
            split: self.split,
            min_split_size: self.min_split_size.clone(),
            connect_timeout: self.connect_timeout,
            max_tries: self.max_tries,
            download_limit: self.download_limit,
            upload_limit: self.upload_limit,
            language: self.language.clone(),
        }
    }

    fn from_public(config: AppConfig, json_rpc_token: String) -> Self {
        Self {
            default_download_dir: config.default_download_dir,
            max_concurrent_downloads: config.max_concurrent_downloads,
            max_connection_per_server: config.max_connection_per_server,
            split: config.split,
            min_split_size: config.min_split_size,
            connect_timeout: config.connect_timeout,
            max_tries: config.max_tries,
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

fn default_max_connection_per_server() -> u32 {
    DEFAULT_MAX_CONNECTION_PER_SERVER
}

fn default_split() -> u32 {
    DEFAULT_SPLIT
}

fn default_min_split_size() -> String {
    DEFAULT_MIN_SPLIT_SIZE.to_string()
}

fn default_connect_timeout() -> u32 {
    DEFAULT_CONNECT_TIMEOUT
}

fn default_max_tries() -> u32 {
    DEFAULT_MAX_TRIES
}

pub fn normalize_min_split_size(value: &str) -> String {
    let value = value.trim();
    if MIN_SPLIT_SIZE_OPTIONS.iter().any(|option| *option == value) {
        value.to_string()
    } else {
        default_min_split_size()
    }
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
