use crate::api::error::ApiError;
use crate::app::HttpAppState;
use crate::debug_logs::DebugLogEntry;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route("/debug-logs", get(list_debug_logs).delete(clear_debug_logs))
}

async fn list_debug_logs(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Vec<DebugLogEntry>>, ApiError> {
    Ok(Json(state.core.debug_logs.list()))
}

async fn clear_debug_logs(State(state): State<Arc<HttpAppState>>) -> impl IntoResponse {
    state.core.debug_logs.clear();
    StatusCode::NO_CONTENT
}
