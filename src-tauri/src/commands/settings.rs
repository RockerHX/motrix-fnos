use crate::app::AppState;
use crate::aria2::{apply_global_options, global_options_from_values, ping_rpc};
use tauri::State;

pub use motrix_fnos_server::settings::service::{
    load_app_config_from_pool, load_ui_preferences_from_pool, AppConfig, UiPreferences,
};
use motrix_fnos_server::settings::service::{
    save_app_config as persist_app_config, save_ui_preferences as persist_ui_preferences,
};

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = load_app_config_from_pool(&state.core.database.pool).await?;
    state.core.debug_logs.info("settings", "读取应用配置");
    Ok(config)
}

#[tauri::command]
pub async fn save_app_config(
    state: State<'_, AppState>,
    payload: AppConfig,
) -> Result<AppConfig, String> {
    let config = persist_app_config(&state.core.database.pool, payload).await?;
    state.core.debug_logs.info("settings", "应用配置已保存");
    apply_runtime_download_config(&state, &config).await;
    Ok(config)
}

#[tauri::command]
pub async fn get_ui_preferences(state: State<'_, AppState>) -> Result<UiPreferences, String> {
    let preferences = load_ui_preferences_from_pool(&state.core.database.pool).await?;
    state.core.debug_logs.info("settings", "读取 UI 偏好");
    Ok(preferences)
}

#[tauri::command]
pub async fn save_ui_preferences(
    state: State<'_, AppState>,
    payload: UiPreferences,
) -> Result<UiPreferences, String> {
    let preferences = persist_ui_preferences(&state.core.database.pool, payload).await?;
    state.core.debug_logs.info("settings", "UI 偏好已保存");
    Ok(preferences)
}

async fn apply_runtime_download_config(state: &State<'_, AppState>, config: &AppConfig) {
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
