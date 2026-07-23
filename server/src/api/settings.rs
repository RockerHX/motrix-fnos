use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::aria2::{apply_global_options, global_options_from_values, ping_rpc};
use crate::debug_logs::{emit_file_log, DebugLogLevel};
use crate::settings::service::{
    load_app_config_from_pool, load_json_rpc_token, save_app_config, save_json_rpc_token, AppConfig,
};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route(
            "/settings/jsonrpc-token",
            get(get_json_rpc_token).put(update_json_rpc_token),
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

async fn get_settings(State(state): State<Arc<HttpAppState>>) -> Result<Json<AppConfig>, ApiError> {
    let default_download_dir = default_download_dir(&state)?;
    let config = load_app_config_from_pool(&state.core.database.pool, &default_download_dir)
        .await
        .map_err(|error| ApiError::internal("settings_load_failed", error))?;
    emit_file_log(DebugLogLevel::Info, "settings", "读取应用配置");
    Ok(Json(config))
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
    state.core.debug_logs.info("settings", "应用配置已保存");
    apply_runtime_download_config(&state, &config).await;
    Ok(Json(config))
}

async fn get_json_rpc_token(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<JsonRpcTokenStatus>, ApiError> {
    let token = load_json_rpc_token(&state.core.database.pool)
        .await
        .map_err(|error| ApiError::internal("jsonrpc_token_load_failed", error))?;
    Ok(Json(json_rpc_token_status(&token)))
}

async fn update_json_rpc_token(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UpdateJsonRpcTokenRequest>,
) -> Result<Json<JsonRpcTokenStatus>, ApiError> {
    let token = save_json_rpc_token(&state.core.database.pool, &payload.token)
        .await
        .map_err(|error| ApiError::internal("jsonrpc_token_save_failed", error))?;
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

async fn apply_runtime_download_config(state: &HttpAppState, config: &AppConfig) {
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
