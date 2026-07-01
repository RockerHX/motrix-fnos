use crate::app::AppState;
use crate::request_application_exit;
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub backend_status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendPing {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    state.debug_logs.info("app", "读取应用信息");
    AppInfo {
        name: "Motrix FNOS".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend_status: "ready".to_string(),
    }
}

#[tauri::command]
pub fn ping_backend(state: State<'_, AppState>) -> BackendPing {
    state.debug_logs.info("app", "Rust 后端通信检查成功");
    BackendPing {
        ok: true,
        message: "Rust 后端通信正常".to_string(),
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    request_application_exit(&app, "前端请求退出应用");
}
