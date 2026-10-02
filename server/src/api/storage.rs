use crate::api::build_task_service;
use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::fnos::FnosApiError;
use crate::fnos::PathLanguage;
use crate::storage::AccessiblePathsRefreshError;
pub use crate::storage::AccessiblePathsResponse;
pub use crate::storage::DisplayAccessiblePathsResponse;
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

#[derive(Debug, serde::Deserialize, Default)]
struct DisplayPathsQuery {
    language: Option<String>,
}

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/storage/accessible-paths", get(get_accessible_paths))
        .route(
            "/storage/accessible-paths/display",
            get(get_display_accessible_paths),
        )
        .route(
            "/storage/accessible-paths/refresh",
            post(refresh_accessible_paths),
        )
}

async fn get_display_accessible_paths(
    State(state): State<Arc<HttpAppState>>,
    Query(query): Query<DisplayPathsQuery>,
) -> Result<Json<DisplayAccessiblePathsResponse>, ApiError> {
    let language = parse_display_language(query.language.as_deref())?;
    let paths = load_accessible_paths(&state)?;
    let paths = state.display_paths(&paths, language).await;
    Ok(Json(DisplayAccessiblePathsResponse { paths }))
}

pub(crate) fn parse_display_language(language: Option<&str>) -> Result<PathLanguage, ApiError> {
    match language.unwrap_or("zh-CN") {
        "zh-CN" => Ok(PathLanguage::ZhCn),
        "en-US" => Ok(PathLanguage::EnUs),
        _ => Err(ApiError::bad_request(
            "display_language_invalid",
            "路径展示语言只支持 zh-CN 或 en-US",
        )),
    }
}

async fn refresh_accessible_paths(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AccessiblePathsResponse>, ApiError> {
    let paths = state
        .refresh_accessible_paths_from_fnos()
        .await
        .map_err(classify_refresh_error)?;
    let process = crate::runtime::process_status(&state.aria2_process).map_err(|error| {
        ApiError::service_unavailable(
            "aria2_status_unavailable",
            format!("无法确认 Aria2 进程状态，撤权任务可能仍在运行：{}", error),
        )
    })?;
    if process.running {
        let runtime = state.aria2_runtime_snapshot().ok_or_else(|| {
            ApiError::service_unavailable(
                "aria2_status_unavailable",
                "Aria2 进程正在运行但缺少运行态记录，无法确认撤权任务已暂停",
            )
        })?;
        if process.pid != Some(runtime.pid) {
            return Err(ApiError::service_unavailable(
                "aria2_status_unavailable",
                format!(
                    "无法确认 Aria2 进程归属，授权已更新但不能确认撤权任务已暂停（运行 PID {:?}，记录 PID {}）",
                    process.pid, runtime.pid
                ),
            ));
        }

        let _activity = state
            .aria2_lifecycle
            .acquire_activity()
            .map_err(|error| ApiError::service_unavailable("aria2_busy", error))?;
        let result = build_task_service(&state)
            .pause_unauthorized_tasks(&state.aria2_config())
            .await;
        if result.as_ref().is_err() || matches!(result.as_ref(), Ok(true)) {
            crate::runtime::broadcast_tasks_snapshot(&state)
                .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
        }
        result.map_err(|error| ApiError::internal("accessible_paths_pause_failed", error))?;
    }
    Ok(Json(AccessiblePathsResponse { paths }))
}

async fn get_accessible_paths(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AccessiblePathsResponse>, ApiError> {
    let paths = load_accessible_paths(&state)?;
    Ok(Json(AccessiblePathsResponse { paths }))
}

pub(crate) fn load_accessible_paths(state: &HttpAppState) -> Result<Vec<String>, ApiError> {
    crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
        .map_err(|error| ApiError::internal("accessible_paths_load_failed", error))
}

fn classify_refresh_error(error: AccessiblePathsRefreshError) -> ApiError {
    match error {
        AccessiblePathsRefreshError::Fnos(FnosApiError::TokenMissing) => {
            ApiError::service_unavailable("fnos_api_token_missing", error.to_string())
        }
        AccessiblePathsRefreshError::Fnos(FnosApiError::SocketUnavailable) => {
            ApiError::service_unavailable("fnos_api_socket_unavailable", error.to_string())
        }
        AccessiblePathsRefreshError::Fnos(FnosApiError::Timeout) => {
            ApiError::service_unavailable("fnos_api_timeout", error.to_string())
        }
        AccessiblePathsRefreshError::Fnos(FnosApiError::Transport) => {
            ApiError::service_unavailable("fnos_api_transport_error", error.to_string())
        }
        AccessiblePathsRefreshError::Fnos(FnosApiError::Rejected { .. }) => {
            ApiError::bad_gateway("fnos_api_rejected", error.to_string())
        }
        AccessiblePathsRefreshError::Fnos(
            FnosApiError::TokenInvalid
            | FnosApiError::ResponseTooLarge
            | FnosApiError::InvalidResponse,
        )
        | AccessiblePathsRefreshError::InvalidPaths => {
            ApiError::bad_gateway("fnos_api_invalid_response", error.to_string())
        }
        AccessiblePathsRefreshError::Persist => {
            ApiError::internal("accessible_paths_persist_failed", error.to_string())
        }
    }
}

#[cfg(test)]
mod tests;
