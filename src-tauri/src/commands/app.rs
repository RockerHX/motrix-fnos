use serde::Serialize;

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
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: "Motrix FNOS".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend_status: "ready".to_string(),
    }
}

#[tauri::command]
pub fn ping_backend() -> BackendPing {
    BackendPing {
        ok: true,
        message: "Rust 后端通信正常".to_string(),
    }
}
