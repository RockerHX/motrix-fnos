use crate::app::HttpAppState;
use crate::aria2::{apply_global_options, global_options_from_values, ping_rpc};
use crate::database::settings::{get_app_config_value, set_app_config_value};
use crate::storage::validate_default_download_dir;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, SqlitePool};
use std::path::Path;

const APP_CONFIG_KEY: &str = "download";
const LAN_JSONRPC_CONFIG_KEY: &str = "jsonrpc_lan";
const DEFAULT_LANGUAGE: &str = "zh-CN";
const ENGLISH_LANGUAGE: &str = "en-US";
pub const DEFAULT_MAX_CONCURRENT_DOWNLOADS: u32 = 5;
pub const MAX_CONCURRENT_DOWNLOADS_LIMIT: u32 = 128;
pub const DEFAULT_HTTP_MAX_CONNECTIONS: u32 = 1;
pub const HTTP_MAX_CONNECTIONS_LIMIT: u32 = 64;
pub const DEFAULT_CONNECT_TIMEOUT: u32 = 60;
pub const MAX_CONNECT_TIMEOUT_LIMIT: u32 = 300;
pub const DEFAULT_MAX_TRIES: u32 = 5;
pub const MAX_TRIES_LIMIT: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub default_download_dir: String,
    pub max_concurrent_downloads: u32,
    #[serde(default = "default_http_max_connections")]
    pub http_max_connections: u32,
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
            http_max_connections: default_http_max_connections(),
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
    #[serde(default)]
    pub allow_shared_address_space: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StoredAppConfig {
    #[serde(flatten)]
    pub config: AppConfig,
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
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| format!("启动下载配置更新事务失败：{error}"))?;
    let json_rpc_token = load_stored_app_config(&mut *transaction, default_download_dir)
        .await?
        .json_rpc_token;
    set_app_config_value(
        &mut *transaction,
        APP_CONFIG_KEY,
        &StoredAppConfig::from_public(config.clone(), json_rpc_token),
    )
    .await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交下载配置更新失败：{error}"))?;
    Ok(config)
}

pub async fn load_json_rpc_token(pool: &SqlitePool) -> Result<String, String> {
    load_stored_app_config(pool, "")
        .await
        .map(|config| config.json_rpc_token)
}

pub async fn save_json_rpc_token(pool: &SqlitePool, token: &str) -> Result<String, String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| format!("启动 RPC Token 更新事务失败：{error}"))?;
    let mut config = load_stored_app_config(&mut *transaction, "").await?;
    config.json_rpc_token = token.trim().to_string();
    set_app_config_value(&mut *transaction, APP_CONFIG_KEY, &config).await?;
    transaction
        .commit()
        .await
        .map_err(|error| format!("提交 RPC Token 更新失败：{error}"))?;
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
    if !(1..=HTTP_MAX_CONNECTIONS_LIMIT).contains(&config.http_max_connections) {
        return Err("settings_connections_invalid：HTTP 连接上限必须在 1 到 64 之间".to_string());
    }
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
        http_max_connections: config.http_max_connections,
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

async fn load_stored_app_config<'e>(
    executor: impl Executor<'e, Database = Sqlite>,
    default_download_dir: &str,
) -> Result<StoredAppConfig, String> {
    match get_app_config_value::<serde_json::Value>(executor, APP_CONFIG_KEY).await? {
        Some(mut value) => {
            migrate_connection_settings(&mut value)?;
            let config =
                serde_json::from_value(value).map_err(|_| "已保存的下载配置无效".to_string())?;
            normalize_stored_app_config(config, default_download_dir)
        }
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
        self.config.clone()
    }

    fn from_public(config: AppConfig, json_rpc_token: String) -> Self {
        Self {
            config,
            json_rpc_token,
        }
    }
}

fn default_language() -> String {
    DEFAULT_LANGUAGE.to_string()
}

fn default_http_max_connections() -> u32 {
    DEFAULT_HTTP_MAX_CONNECTIONS
}

fn default_connect_timeout() -> u32 {
    DEFAULT_CONNECT_TIMEOUT
}

fn default_max_tries() -> u32 {
    DEFAULT_MAX_TRIES
}

pub fn parse_app_config_request(mut value: serde_json::Value) -> Result<AppConfig, String> {
    let object = value
        .as_object()
        .ok_or("settings_save_failed：下载配置必须是对象")?;
    if object.contains_key("minSplitSize") {
        return Err("settings_option_unsupported：minSplitSize 已失效，请移除该字段".to_string());
    }
    if object.contains_key("httpMaxConnections")
        && (object.contains_key("split") || object.contains_key("maxConnectionPerServer"))
    {
        return Err("settings_option_conflict：不能同时指定新旧 HTTP 连接参数".to_string());
    }
    if let Some(limit) = object.get("httpMaxConnections") {
        if !limit
            .as_u64()
            .is_some_and(|limit| (1..=64).contains(&limit))
        {
            return Err(
                "settings_connections_invalid：HTTP 连接上限必须是 1 到 64 的整数".to_string(),
            );
        }
    }
    migrate_connection_settings(&mut value)?;
    serde_json::from_value(value).map_err(|_| "settings_save_failed：下载配置字段无效".to_string())
}

fn migrate_connection_settings(value: &mut serde_json::Value) -> Result<(), String> {
    let object = value.as_object_mut().ok_or("下载配置必须是对象")?;
    if let Some(limit) = object.get("httpMaxConnections") {
        if !limit
            .as_u64()
            .is_some_and(|limit| (1..=u64::from(HTTP_MAX_CONNECTIONS_LIMIT)).contains(&limit))
        {
            object.insert(
                "httpMaxConnections".to_string(),
                DEFAULT_HTTP_MAX_CONNECTIONS.into(),
            );
        }
    } else {
        let legacy_limit = |key, default| {
            object
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .filter(|value| *value > 0)
                .unwrap_or(default)
                .clamp(1, u64::from(HTTP_MAX_CONNECTIONS_LIMIT))
        };
        let limit = legacy_limit("split", 5).min(legacy_limit("maxConnectionPerServer", 1));
        object.insert("httpMaxConnections".to_string(), limit.into());
    }
    for key in ["split", "maxConnectionPerServer", "minSplitSize"] {
        object.remove(key);
    }
    Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RuntimeApplyStatus {
    Applied,
    Deferred,
    Failed,
}

pub(crate) async fn update_app_config(
    state: &HttpAppState,
    payload: AppConfig,
) -> Result<(AppConfig, RuntimeApplyStatus), AppConfigUpdateError> {
    let accessible_paths =
        crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
            .map_err(AppConfigUpdateError::AccessiblePaths)?;
    let default_download_dir =
        crate::storage::default_download_dir(&accessible_paths, &state.runtime.app_data_dir)
            .display()
            .to_string();
    let _update = state.settings_update_lock.lock().await;
    let config = save_app_config(
        &state.core.database.pool,
        payload,
        &default_download_dir,
        &accessible_paths,
        &state.runtime.app_data_dir,
    )
    .await
    .map_err(AppConfigUpdateError::Save)?;
    state.set_app_config(config.clone()).await;
    state.refresh_json_rpc_default_download_dir(&config.default_download_dir, &accessible_paths);
    state.core.debug_logs.info("settings", "应用配置已保存");
    let runtime_apply = apply_runtime_download_config(state, &config).await;
    Ok((config, runtime_apply))
}

#[derive(Debug)]
pub(crate) enum AppConfigUpdateError {
    AccessiblePaths(String),
    Save(String),
}

pub(crate) async fn update_public_rpc_token(
    state: &HttpAppState,
    token: &str,
) -> Result<String, String> {
    let _update = state.settings_update_lock.lock().await;
    let token = save_json_rpc_token(&state.core.database.pool, token).await?;
    state.remember_json_rpc_token(&token);
    state
        .core
        .debug_logs
        .info("settings.jsonrpc", "JSON-RPC Token 已更新");
    Ok(token)
}

pub(crate) enum LanJsonRpcUpdate {
    Settings {
        enabled: bool,
        allow_shared_address_space: Option<bool>,
    },
    RotateToken,
}

#[derive(Debug)]
pub(crate) enum LanJsonRpcUpdateError {
    Generate(String),
    Save(String),
}

pub(crate) async fn update_lan_json_rpc_config(
    state: &HttpAppState,
    update: LanJsonRpcUpdate,
) -> Result<(LanJsonRpcConfig, Option<String>), LanJsonRpcUpdateError> {
    let mut current = state.lan_json_rpc_config.write().await;
    let mut next = current.clone();
    let (issue_token, message) = match update {
        LanJsonRpcUpdate::Settings {
            enabled,
            allow_shared_address_space,
        } => {
            next.enabled = enabled;
            if let Some(allow) = allow_shared_address_space {
                next.allow_shared_address_space = allow;
            }
            (
                enabled && next.token.is_empty(),
                if enabled {
                    "局域网 JSON-RPC 入口已启用"
                } else {
                    "局域网 JSON-RPC 入口已关闭"
                },
            )
        }
        LanJsonRpcUpdate::RotateToken => (true, "局域网 JSON-RPC Token 已轮换"),
    };
    let issued_token = if issue_token {
        let mut bytes = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|error| LanJsonRpcUpdateError::Generate(error.to_string()))?;
        let token = URL_SAFE_NO_PAD.encode(bytes);
        next.token = token.clone();
        Some(token)
    } else {
        None
    };
    let persisted = save_lan_json_rpc_config(&state.core.database.pool, &next)
        .await
        .map_err(LanJsonRpcUpdateError::Save)?;
    *current = persisted.clone();
    state.notify_lan_json_rpc_config_changed();
    state.core.debug_logs.info("settings.jsonrpc_lan", message);
    Ok((persisted, issued_token))
}

async fn apply_runtime_download_config(
    state: &HttpAppState,
    config: &AppConfig,
) -> RuntimeApplyStatus {
    let _activity = match state.aria2_lifecycle.acquire_activity() {
        Ok(activity) => activity,
        Err(error) => {
            state.core.debug_logs.warn(
                "settings",
                format!(
                    "Aria2 生命周期正在转换，下载配置将在下次启动后生效：{}",
                    error
                ),
            );
            return RuntimeApplyStatus::Deferred;
        }
    };
    let aria2_config = state.aria2_config();
    let status = ping_rpc(&state.aria2_rpc, &aria2_config, None).await;
    if !status.connected {
        state.core.debug_logs.warn(
            "settings",
            format!(
                "Aria2 RPC 未就绪，下载配置将在下次启动后生效：{}",
                status.message
            ),
        );
        return RuntimeApplyStatus::Deferred;
    }

    let options = global_options_from_values(
        config.max_concurrent_downloads,
        config.download_limit,
        config.upload_limit,
        config.http_max_connections,
        config.connect_timeout,
        config.max_tries,
    );
    if let Err(error) = apply_global_options(
        &state.aria2_rpc,
        &aria2_config,
        &options,
        Some(&state.core.debug_logs),
    )
    .await
    {
        state
            .core
            .debug_logs
            .warn("settings", format!("即时应用下载配置失败：{}", error));
        RuntimeApplyStatus::Failed
    } else {
        RuntimeApplyStatus::Applied
    }
}
