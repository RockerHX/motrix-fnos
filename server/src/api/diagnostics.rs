use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::runtime::{process_status, update_aria2_log_mode, Aria2LogModeUpdateError};
use axum::body::Body;
use axum::extract::State;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route(
            "/diagnostics/aria2-log-mode",
            get(get_aria2_log_mode).put(put_aria2_log_mode),
        )
        .route("/diagnostics/diagnostic-bundle", get(get_diagnostic_bundle))
}

#[derive(Debug, Deserialize)]
struct UpdateAria2LogModeRequest {
    detailed: bool,
}

async fn get_aria2_log_mode(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<crate::aria2::Aria2LogModeStatus>, ApiError> {
    log_mode_status(&state).map(Json)
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

fn classify_log_mode_error(error: Aria2LogModeUpdateError) -> ApiError {
    match error {
        Aria2LogModeUpdateError::Conflict(message) => {
            ApiError::conflict("aria2_log_mode_conflict", message)
        }
        Aria2LogModeUpdateError::Failed(message) => {
            ApiError::service_unavailable("aria2_log_mode_update_failed", message)
        }
    }
}
