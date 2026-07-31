use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::aria2::{ping_rpc, Aria2ConfigStatus};
use crate::runtime::{
    process_status, resolve_aria2_binary, start_aria2, stop_aria2, Aria2ProcessStatus,
    Aria2StopError,
};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/aria2/config", get(get_aria2_config_status))
        .route("/aria2/process", get(get_aria2_process_status))
        .route("/aria2/rpc", get(get_aria2_rpc_status))
        .route("/aria2/start", post(start_aria2_process))
        .route("/aria2/stop", post(stop_aria2_process))
}

async fn get_aria2_config_status(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ConfigStatus>, ApiError> {
    let mut config = state.aria2_config();
    if let Ok(resolved) = resolve_aria2_binary(&state.runtime, &config) {
        config.aria2_path = Some(resolved.path.display().to_string());
        config.binary_source = resolved.source;
    }
    Ok(Json(Aria2ConfigStatus::from_config(&config)))
}

async fn get_aria2_process_status(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    let status = process_status(&state.aria2_process)
        .map_err(|error| ApiError::internal("aria2_process_status_failed", error))?;
    Ok(Json(status))
}

async fn get_aria2_rpc_status(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<crate::aria2::Aria2RpcStatus>, ApiError> {
    let process = process_status(&state.aria2_process)
        .map_err(|error| ApiError::internal("aria2_process_status_failed", error))?;
    let status = if !process.running {
        if state.aria2_runtime_snapshot().is_some() {
            disconnected_rpc_status("Aria2 运行态待确认")
        } else {
            disconnected_rpc_status("Aria2 未运行")
        }
    } else if state.aria2_runtime_snapshot().is_none() {
        disconnected_rpc_status("Aria2 运行态未记录")
    } else {
        ping_rpc(&state.aria2_rpc, &state.aria2_config(), None).await
    };
    if let Some(version) = status.version.as_deref() {
        state.remember_aria2_version(version);
    }
    Ok(Json(status))
}

fn disconnected_rpc_status(message: &str) -> crate::aria2::Aria2RpcStatus {
    crate::aria2::Aria2RpcStatus {
        connected: false,
        version: None,
        message: message.to_string(),
    }
}

async fn start_aria2_process(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    ensure_runtime_not_exiting(&state)?;

    let status = start_aria2(&state)
        .await
        .map_err(classify_aria2_start_error)?;
    Ok(Json(status))
}

async fn stop_aria2_process(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    ensure_runtime_not_exiting(&state)?;
    let status = stop_aria2(&state)
        .await
        .map_err(classify_aria2_stop_error)?;
    Ok(Json(status))
}

fn ensure_runtime_not_exiting(state: &HttpAppState) -> Result<(), ApiError> {
    if state.core.shutdown.is_exiting() {
        return Err(ApiError::conflict(
            "runtime_exiting",
            "服务正在退出，不能执行当前操作",
        ));
    }

    Ok(())
}

fn classify_aria2_start_error(error: String) -> ApiError {
    if error.contains("已被其他进程占用") || error.contains("未找到可用 Aria2 Next") {
        return ApiError::conflict("aria2_start_conflict", error);
    }

    ApiError::internal("aria2_start_failed", error)
}

fn classify_aria2_stop_error(error: Aria2StopError) -> ApiError {
    match error {
        Aria2StopError::Busy(message) => ApiError::conflict("aria2_busy", message),
        Aria2StopError::Failed(message) => {
            ApiError::service_unavailable("aria2_stop_failed", message)
        }
    }
}
