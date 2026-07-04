use crate::api::error::ApiError;
use crate::app::HttpAppState;
pub use crate::storage::AccessiblePathsResponse;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/storage/accessible-paths", get(get_accessible_paths))
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
