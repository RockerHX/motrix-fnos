use crate::api::error::ApiError;
use crate::app::HttpAppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const APP_NAME: &str = "Motrix";
pub const APP_MAINTAINER: &str = "rockerhx";
pub const REPOSITORY_URL: &str = "https://github.com/RockerHX/motrix-fnos";
pub const RELEASE_PAGE_URL: &str = "https://github.com/RockerHX/motrix-fnos/releases";
pub const UPDATE_MODE: &str = "manual_fpk_or_app_center";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub backend_status: String,
    pub maintainer: String,
    pub repository_url: String,
    pub release_page_url: String,
    pub target_arch: String,
    pub update_mode: String,
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

async fn get_app_info(State(state): State<Arc<HttpAppState>>) -> Result<Json<AppInfo>, ApiError> {
    state.core.debug_logs.info("app", "读取应用信息");
    Ok(Json(AppInfo {
        name: APP_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        backend_status: "ready".to_string(),
        maintainer: APP_MAINTAINER.to_string(),
        repository_url: REPOSITORY_URL.to_string(),
        release_page_url: RELEASE_PAGE_URL.to_string(),
        target_arch: std::env::consts::ARCH.to_string(),
        update_mode: UPDATE_MODE.to_string(),
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
