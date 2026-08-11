use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::runtime::{
    clear_aria2_logs, collect_log_usage, process_status, update_aria2_log_mode,
    Aria2LogMaintenanceOutcome, Aria2LogModeUpdateError, LogFileUsage,
};
use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route(
            "/diagnostics/aria2-log-mode",
            get(get_aria2_log_mode).put(put_aria2_log_mode),
        )
        .route("/diagnostics/logs", get(get_log_usage))
        .route("/diagnostics/aria2-logs", delete(delete_aria2_logs))
        .route("/diagnostics/diagnostic-bundle", get(get_diagnostic_bundle))
}

#[derive(Debug, Deserialize)]
struct UpdateAria2LogModeRequest {
    detailed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticsLogUsageResponse {
    pub aria2: LogFileUsage,
    pub server: LogFileUsage,
    pub lifecycle: LogFileUsage,
    pub total_bytes: u64,
    pub total_file_count: usize,
    pub aria2_log_mode: crate::aria2::Aria2LogModeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aria2LogCleanupResponse {
    pub reclaimed_bytes: u64,
    pub usage: DiagnosticsLogUsageResponse,
}

async fn get_aria2_log_mode(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<crate::aria2::Aria2LogModeStatus>, ApiError> {
    log_mode_status(&state).map(Json)
}

async fn get_log_usage(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<DiagnosticsLogUsageResponse>, ApiError> {
    log_usage_response(&state).map(Json)
}

async fn delete_aria2_logs(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Aria2LogCleanupResponse>, ApiError> {
    if state.core.shutdown.is_exiting() {
        return Err(ApiError::conflict(
            "runtime_exiting",
            "服务正在退出，不能执行当前操作",
        ));
    }

    let report = match clear_aria2_logs(&state).await.map_err(|error| {
        state.core.debug_logs.warn(
            "diagnostics.logs",
            format!("清理 Aria2 原生日志失败：{error}"),
        );
        ApiError::internal(
            "aria2_log_cleanup_failed",
            "清理 Aria2 日志失败，请稍后重试",
        )
    })? {
        Aria2LogMaintenanceOutcome::Maintained(report) => report,
        Aria2LogMaintenanceOutcome::Skipped(reason) => {
            return Err(ApiError::conflict("aria2_log_in_use", reason.to_string()))
        }
    };

    state.core.debug_logs.info(
        "diagnostics.logs",
        format!(
            "已清理 Aria2 原生日志，释放 {} 字节",
            report.reclaimed_bytes()
        ),
    );
    let usage = log_usage_response(&state)?;
    Ok(Json(Aria2LogCleanupResponse {
        reclaimed_bytes: report.reclaimed_bytes(),
        usage,
    }))
}

async fn get_diagnostic_bundle(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Response, ApiError> {
    let bundle = crate::diagnostics::build_diagnostic_bundle(&state).map_err(|error| {
        state
            .core
            .debug_logs
            .error("diagnostics.bundle", format!("生成诊断包失败：{error}"));
        ApiError::internal("diagnostic_bundle_failed", "生成诊断包失败，请稍后重试")
    })?;
    let mut response = Body::from(bundle).into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"motrix-fnos-diagnostic-bundle.zip\""),
    );
    Ok(response)
}

async fn put_aria2_log_mode(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<UpdateAria2LogModeRequest>,
) -> Result<Json<crate::aria2::Aria2LogModeStatus>, ApiError> {
    if state.core.shutdown.is_exiting() {
        return Err(ApiError::conflict(
            "runtime_exiting",
            "服务正在退出，不能执行当前操作",
        ));
    }

    update_aria2_log_mode(&state, payload.detailed)
        .await
        .map_err(classify_log_mode_error)?;
    log_mode_status(&state).map(Json)
}

fn log_mode_status(state: &HttpAppState) -> Result<crate::aria2::Aria2LogModeStatus, ApiError> {
    let process = process_status(&state.aria2_process)
        .map_err(|error| ApiError::internal("aria2_process_status_failed", error))?;
    Ok(state.aria2_log_mode.status(process.running))
}

fn log_usage_response(state: &HttpAppState) -> Result<DiagnosticsLogUsageResponse, ApiError> {
    let process = process_status(&state.aria2_process)
        .map_err(|error| ApiError::internal("aria2_process_status_failed", error))?;
    let usage = collect_log_usage(&state.runtime.app_data_dir)
        .map_err(|_| ApiError::internal("diagnostics_log_usage_failed", "读取日志占用失败"))?;
    Ok(DiagnosticsLogUsageResponse {
        aria2: usage.aria2,
        server: usage.server,
        lifecycle: usage.lifecycle,
        total_bytes: usage.total_bytes,
        total_file_count: usage.total_file_count,
        aria2_log_mode: state.aria2_log_mode.status(process.running),
    })
}

fn classify_log_mode_error(error: Aria2LogModeUpdateError) -> ApiError {
    match error {
        Aria2LogModeUpdateError::Conflict(message) => {
            ApiError::conflict("aria2_log_mode_conflict", message)
        }
        Aria2LogModeUpdateError::Failed(message)
        | Aria2LogModeUpdateError::OutcomeUnknown(message) => {
            ApiError::service_unavailable("aria2_log_mode_update_failed", message)
        }
    }
}
