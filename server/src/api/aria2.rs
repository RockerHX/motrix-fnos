use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::aria2::{
    generate_rpc_secret, ping_rpc, rpc_ports_exhausted_message, runtime_config,
    select_rpc_port_with_saved_runtime, Aria2ConfigStatus, SavedAria2Runtime,
};
use crate::runtime::{
    process_status, resolve_aria2_binary, start_process, stop_process, Aria2ProcessStatus,
};
use crate::state::Aria2RuntimeInfo;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::atomic::Ordering;
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
    state.core.debug_logs.info("aria2", "读取 Aria2 配置状态");
    Ok(Json(Aria2ConfigStatus::from_config(&config)))
}

async fn get_aria2_process_status(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    let status = process_status(&state.aria2_process)
        .map_err(|error| ApiError::internal("aria2_process_status_failed", error))?;
    if !status.running && status.pid.is_some() {
        state.clear_aria2_runtime();
    }
    state
        .core
        .debug_logs
        .info("aria2", format!("读取 Aria2 进程状态：{}", status.message));
    Ok(Json(status))
}

async fn get_aria2_rpc_status(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<crate::aria2::Aria2RpcStatus>, ApiError> {
    Ok(Json(
        ping_rpc(&state.aria2_config(), Some(&state.core.debug_logs)).await,
    ))
}

async fn start_aria2_process(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    ensure_runtime_not_exiting(&state)?;

    let base = state.base_aria2_config.clone();
    let saved_runtime = state.load_saved_aria2_runtime();
    let saved_runtime = saved_runtime.as_ref().map(saved_runtime_info);
    let port = select_rpc_port_with_saved_runtime(
        &base,
        saved_runtime.as_ref(),
        &state.core.debug_logs,
    )
    .ok_or_else(|| ApiError::conflict("aria2_port_conflict", rpc_ports_exhausted_message()))?;
    let config = state
        .with_aria2_runtime_paths(runtime_config(&base, port, generate_rpc_secret()))
        .map_err(|error| ApiError::internal("aria2_runtime_prepare_failed", error))?;
    let status = start_process(
        &state.aria2_process,
        &state.runtime,
        &config,
        &state.core.debug_logs,
    )
    .map_err(classify_aria2_start_error)?;
    if let (Some(pid), Some(source)) = (status.pid, status.binary_source.clone()) {
        state
            .set_aria2_runtime(state.build_aria2_runtime_info(
                pid,
                &config,
                source,
                crate::aria2::process_args(&config),
            ))
            .map_err(|error| ApiError::internal("aria2_runtime_persist_failed", error))?;
    }
    Ok(Json(status))
}

async fn stop_aria2_process(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2ProcessStatus>, ApiError> {
    ensure_runtime_not_exiting(&state)?;
    let status = stop_process(&state.aria2_process, &state.core.debug_logs)
        .map_err(|error| ApiError::internal("aria2_stop_failed", error))?;
    state.clear_aria2_runtime();
    Ok(Json(status))
}

fn ensure_runtime_not_exiting(state: &HttpAppState) -> Result<(), ApiError> {
    if state.core.is_exiting.load(Ordering::SeqCst) {
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

fn saved_runtime_info(runtime: &Aria2RuntimeInfo) -> SavedAria2Runtime {
    SavedAria2Runtime {
        pid: runtime.pid,
        actual_port: runtime.actual_port,
        rpc_secret: runtime.rpc_secret.clone(),
        binary_source: runtime.binary_source.clone(),
        sidecar_name: runtime.sidecar_name.clone(),
        app_data_dir: runtime.app_data_dir.clone(),
        aria2_session_path: runtime.aria2_session_path.clone(),
        aria2_log_path: runtime.aria2_log_path.clone(),
    }
}
