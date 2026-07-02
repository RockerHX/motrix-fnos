use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::aria2::{apply_global_options, global_options_from_values, ping_rpc};
use crate::settings::service::{
    load_app_config_from_pool, load_ui_preferences_from_pool, save_app_config, save_ui_preferences,
    AppConfig, UiPreferences,
};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
        .route(
            "/ui-preferences",
            get(get_ui_preferences).put(update_ui_preferences),
        )
}

async fn get_settings(State(state): State<Arc<HttpAppState>>) -> Result<Json<AppConfig>, ApiError> {
    let config = load_app_config_from_pool(&state.core.database.pool)
        .await
        .map_err(|error| ApiError::internal("settings_load_failed", error))?;
    state.core.debug_logs.info("settings", "读取应用配置");
    Ok(Json(config))
}

async fn update_settings(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<AppConfig>,
) -> Result<Json<AppConfig>, ApiError> {
    let config = save_app_config(&state.core.database.pool, payload)
        .await
        .map_err(|error| ApiError::internal("settings_save_failed", error))?;
    state.core.debug_logs.info("settings", "应用配置已保存");
    apply_runtime_download_config(&state, &config).await;
    Ok(Json(config))
}

async fn get_ui_preferences(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<UiPreferences>, ApiError> {
    let preferences = load_ui_preferences_from_pool(&state.core.database.pool)
        .await
        .map_err(|error| ApiError::internal("ui_preferences_load_failed", error))?;
    state.core.debug_logs.info("settings", "读取 UI 偏好");
    Ok(Json(preferences))
}

async fn update_ui_preferences(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UiPreferences>,
) -> Result<Json<UiPreferences>, ApiError> {
    let preferences = save_ui_preferences(&state.core.database.pool, payload)
        .await
        .map_err(|error| ApiError::internal("ui_preferences_save_failed", error))?;
    state.core.debug_logs.info("settings", "UI 偏好已保存");
    Ok(Json(preferences))
}

async fn apply_runtime_download_config(state: &HttpAppState, config: &AppConfig) {
    let aria2_config = state.aria2_config();
    let status = ping_rpc(&aria2_config, None).await;
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
    if let Err(error) =
        apply_global_options(&aria2_config, &options, Some(&state.core.debug_logs)).await
    {
        state
            .core
            .debug_logs
            .warn("settings", format!("即时应用下载配置失败：{}", error));
    }
}
