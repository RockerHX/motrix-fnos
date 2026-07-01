use crate::api::error::ApiError;
use crate::app::HttpAppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub backend_status: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendPing {
    pub ok: bool,
    pub message: String,
}

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/app/info", get(get_app_info))
        .route("/app/ping", get(ping_backend))
}

async fn get_app_info(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AppInfo>, ApiError> {
    state.core.debug_logs.info("app", "读取应用信息");
    Ok(Json(AppInfo {
        name: "Motrix FNOS".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend_status: "ready".to_string(),
    }))
}

async fn ping_backend(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<BackendPing>, ApiError> {
    state.core.debug_logs.info("app", "Rust 后端通信检查成功");
    Ok(Json(BackendPing {
        ok: true,
        message: "Rust 后端通信正常".to_string(),
    }))
}
