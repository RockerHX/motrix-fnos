use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::aria2::{apply_global_options, global_options_from_values, ping_rpc};
use crate::settings::proxy::{
    delete_download_proxy, load_download_proxy_status, update_download_proxy,
    DownloadProxyMutationResponse, DownloadProxyServiceContext, DownloadProxyServiceError,
    DownloadProxyStatus,
};
use crate::settings::service::{
    load_app_config_from_pool, save_app_config, save_json_rpc_token, save_lan_json_rpc_config,
    AppConfig, LanJsonRpcConfig,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route(
            "/settings/jsonrpc-token",
            get(get_json_rpc_token).put(update_json_rpc_token),
        )
        .route(
            "/settings/lan-jsonrpc",
            get(get_lan_json_rpc).put(update_lan_json_rpc),
        )
        .route(
            "/settings/lan-jsonrpc/token",
            post(rotate_lan_json_rpc_token),
        )
        .route(
            "/settings/proxy",
            get(get_download_proxy)
                .put(put_download_proxy)
                .delete(clear_download_proxy),
        )
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct JsonRpcTokenStatus {
    pub configured: bool,
    pub masked_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateJsonRpcTokenRequest {
    token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanJsonRpcStatus {
    pub enabled: bool,
    pub configured: bool,
    pub masked_token: Option<String>,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
struct UpdateLanJsonRpcRequest {
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadProxyRequest {
    proxy_url: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanJsonRpcMutationResponse {
    pub status: LanJsonRpcStatus,
    pub issued_token: Option<String>,
}

async fn get_settings(State(state): State<Arc<HttpAppState>>) -> Result<Json<AppConfig>, ApiError> {
    let default_download_dir = default_download_dir(&state)?;
    let config = load_app_config_from_pool(&state.core.database.pool, &default_download_dir)
        .await
        .map_err(|error| ApiError::internal("settings_load_failed", error))?;
    Ok(Json(config))
}

async fn get_download_proxy(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<DownloadProxyStatus>, ApiError> {
    load_download_proxy_status(&state.core.database.pool)
        .await
        .map(Json)
        .map_err(classify_download_proxy_error)
}

async fn put_download_proxy(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UpdateDownloadProxyRequest>,
) -> Result<Json<DownloadProxyMutationResponse>, ApiError> {
    let aria2_config = state.aria2_runtime_snapshot().map(|_| state.aria2_config());
    update_download_proxy(
        DownloadProxyServiceContext {
            pool: &state.core.database.pool,
            tasks: &state.core.download_tasks,
            aria2_lifecycle: &state.aria2_lifecycle,
            aria2_rpc: &state.aria2_rpc,
            aria2_config,
            debug_logs: &state.core.debug_logs,
            update_lock: &state.download_proxy_update_lock,
        },
        &payload.proxy_url,
    )
    .await
    .map(Json)
    .map_err(classify_download_proxy_error)
}

async fn clear_download_proxy(
    State(state): State<Arc<HttpAppState>>,
) -> Result<StatusCode, ApiError> {
    delete_download_proxy(
        &state.core.database.pool,
        &state.core.download_tasks,
        &state.download_proxy_update_lock,
        &state.core.debug_logs,
    )
    .await
    .map(|()| StatusCode::NO_CONTENT)
    .map_err(classify_download_proxy_error)
}

async fn update_settings(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<AppConfig>,
) -> Result<Json<AppConfig>, ApiError> {
    let accessible_paths = accessible_paths(&state)?;
    let default_download_dir =
        crate::storage::default_download_dir(&accessible_paths, &state.runtime.app_data_dir)
            .display()
            .to_string();
    let config = save_app_config(
        &state.core.database.pool,
        payload,
        &default_download_dir,
        &accessible_paths,
        &state.runtime.app_data_dir,
    )
    .await
    .map_err(classify_settings_save_error)?;
    state.refresh_json_rpc_default_download_dir(&config.default_download_dir, &accessible_paths);
    state.core.debug_logs.info("settings", "应用配置已保存");
    apply_runtime_download_config(&state, &config).await;
    Ok(Json(config))
}

async fn get_json_rpc_token(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<JsonRpcTokenStatus>, ApiError> {
    Ok(Json(json_rpc_token_status(&state.json_rpc_token())))
}

async fn get_lan_json_rpc(State(state): State<Arc<HttpAppState>>) -> Json<LanJsonRpcStatus> {
    let config = state.lan_json_rpc_config().await;
    Json(lan_json_rpc_status(&config))
}

async fn update_lan_json_rpc(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UpdateLanJsonRpcRequest>,
) -> Result<Json<LanJsonRpcMutationResponse>, ApiError> {
    let mut current = state.lan_json_rpc_config.write().await;
    let mut next = current.clone();
    let issued_token = if payload.enabled && next.token.is_empty() {
        let token = generate_lan_json_rpc_token()?;
        next.token = token.clone();
        Some(token)
    } else {
        None
    };
    next.enabled = payload.enabled;
    let persisted = save_lan_json_rpc_config(&state.core.database.pool, &next)
        .await
        .map_err(|error| ApiError::internal("lan_jsonrpc_save_failed", error))?;
    *current = persisted.clone();
    state.core.debug_logs.info(
        "settings.jsonrpc_lan",
        if persisted.enabled {
            "局域网 JSON-RPC 入口已启用"
        } else {
            "局域网 JSON-RPC 入口已关闭"
        },
    );
    Ok(Json(LanJsonRpcMutationResponse {
        status: lan_json_rpc_status(&persisted),
        issued_token,
    }))
}

async fn rotate_lan_json_rpc_token(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<LanJsonRpcMutationResponse>, ApiError> {
    let mut current = state.lan_json_rpc_config.write().await;
    let token = generate_lan_json_rpc_token()?;
    let next = LanJsonRpcConfig {
        enabled: current.enabled,
        token: token.clone(),
    };
    let persisted = save_lan_json_rpc_config(&state.core.database.pool, &next)
        .await
        .map_err(|error| ApiError::internal("lan_jsonrpc_token_save_failed", error))?;
    *current = persisted.clone();
    state
        .core
        .debug_logs
        .info("settings.jsonrpc_lan", "局域网 JSON-RPC Token 已轮换");
    Ok(Json(LanJsonRpcMutationResponse {
        status: lan_json_rpc_status(&persisted),
        issued_token: Some(token),
    }))
}

async fn update_json_rpc_token(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UpdateJsonRpcTokenRequest>,
) -> Result<Json<JsonRpcTokenStatus>, ApiError> {
    let token = save_json_rpc_token(&state.core.database.pool, &payload.token)
        .await
        .map_err(|error| ApiError::internal("jsonrpc_token_save_failed", error))?;
    state.remember_json_rpc_token(&token);
    state
        .core
        .debug_logs
        .info("settings.jsonrpc", "JSON-RPC Token 已更新");
    Ok(Json(json_rpc_token_status(&token)))
}

fn json_rpc_token_status(token: &str) -> JsonRpcTokenStatus {
    if token.is_empty() {
        return JsonRpcTokenStatus {
            configured: false,
            masked_token: None,
        };
    }
    let chars = token.chars().collect::<Vec<_>>();
    let suffix = if chars.len() >= 8 {
        chars[chars.len() - 4..].iter().collect::<String>()
    } else {
        String::new()
    };
    JsonRpcTokenStatus {
        configured: true,
        masked_token: Some(format!("••••••••{suffix}")),
    }
}

fn lan_json_rpc_status(config: &LanJsonRpcConfig) -> LanJsonRpcStatus {
    let token = json_rpc_token_status(&config.token);
    LanJsonRpcStatus {
        enabled: config.enabled,
        configured: token.configured,
        masked_token: token.masked_token,
        port: 17082,
    }
}

fn generate_lan_json_rpc_token() -> Result<String, ApiError> {
    let mut bytes = [0_u8; 32];
    OsRng.try_fill_bytes(&mut bytes).map_err(|error| {
        ApiError::internal("lan_jsonrpc_token_generate_failed", error.to_string())
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests;

fn default_download_dir(state: &HttpAppState) -> Result<String, ApiError> {
    crate::storage::load_default_download_dir(
        &state.runtime.accessible_paths_path,
        &state.runtime.app_data_dir,
    )
    .map_err(|error| ApiError::internal("default_download_dir_failed", error))
}

fn accessible_paths(state: &HttpAppState) -> Result<Vec<String>, ApiError> {
    crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
        .map_err(|error| ApiError::internal("accessible_paths_load_failed", error))
}

fn classify_settings_save_error(error: String) -> ApiError {
    if error.contains("默认下载目录") {
        return ApiError::bad_request("settings_save_failed", error);
    }
    ApiError::internal("settings_save_failed", error)
}

fn classify_download_proxy_error(error: DownloadProxyServiceError) -> ApiError {
    match error {
        DownloadProxyServiceError::InvalidUrl(message) => {
            ApiError::bad_request("proxy_invalid_url", message)
        }
        DownloadProxyServiceError::InUse => {
            ApiError::conflict("proxy_in_use", "仍有任务使用应用代理配置，不能清除")
        }
        DownloadProxyServiceError::Load(message) => {
            ApiError::internal("proxy_load_failed", message)
        }
        DownloadProxyServiceError::Save(message) => {
            ApiError::internal("proxy_save_failed", message)
        }
        DownloadProxyServiceError::State(message) => {
            ApiError::internal("proxy_save_failed", message)
        }
    }
}

async fn apply_runtime_download_config(state: &HttpAppState, config: &AppConfig) {
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
            return;
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
        return;
    }

    let options = global_options_from_values(
        config.max_concurrent_downloads,
        config.download_limit,
        config.upload_limit,
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
    }
}
