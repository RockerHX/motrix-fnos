use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::fnos::FnosApiError;
use crate::storage::AccessiblePathsRefreshError;
pub use crate::storage::AccessiblePathsResponse;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/storage/accessible-paths", get(get_accessible_paths))
        .route(
            "/storage/accessible-paths/refresh",
            post(refresh_accessible_paths),
        )
}

async fn refresh_accessible_paths(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AccessiblePathsResponse>, ApiError> {
    let paths = state
        .refresh_accessible_paths_from_fnos()
        .await
        .map_err(classify_refresh_error)?;
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
