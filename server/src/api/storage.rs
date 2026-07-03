use crate::api::error::ApiError;
use crate::app::HttpAppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DATA_ACCESSIBLE_PATHS_ENV: &str = "TRIM_DATA_ACCESSIBLE_PATHS";

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/storage/accessible-paths", get(get_accessible_paths))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccessiblePathsResponse {
    pub paths: Vec<String>,
}

async fn get_accessible_paths(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<AccessiblePathsResponse>, ApiError> {
    let paths = load_accessible_paths(&state)?;
    Ok(Json(AccessiblePathsResponse { paths }))
}

pub(crate) fn load_accessible_paths(state: &HttpAppState) -> Result<Vec<String>, ApiError> {
    if state.runtime.accessible_paths_path.is_file() {
        let content =
            std::fs::read_to_string(&state.runtime.accessible_paths_path).map_err(|error| {
                ApiError::internal("accessible_paths_read_failed", error.to_string())
            })?;
        let response =
            serde_json::from_str::<AccessiblePathsResponse>(&content).map_err(|error| {
                ApiError::internal("accessible_paths_parse_failed", error.to_string())
            })?;
        return Ok(normalize_paths(response.paths));
    }

    Ok(normalize_paths(
        std::env::var(DATA_ACCESSIBLE_PATHS_ENV)
            .ok()
            .map(|value| value.split(':').map(str::to_string).collect())
            .unwrap_or_default(),
    ))
}

fn normalize_paths(paths: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for path in paths {
        let path = path.trim();
        if !path.is_empty() && !normalized.iter().any(|item| item == path) {
            normalized.push(path.to_string());
        }
    }
    normalized
}
