use super::types::{positional_params, RpcFault};
use crate::app::HttpAppState;
use crate::settings::service::load_app_config_from_pool;
use serde_json::Value;
use std::sync::Arc;

pub(super) async fn ensure_add_uri_token(
    state: &Arc<HttpAppState>,
    params: &Value,
) -> Result<(), RpcFault> {
    let default_download_dir = state.runtime.app_data_dir.display().to_string();
    let config = load_app_config_from_pool(&state.core.database.pool, &default_download_dir)
        .await
        .map_err(RpcFault::server_error)?;

    validate_add_uri_token(&config.json_rpc_token, params)
}

pub(super) fn validate_add_uri_token(
    configured_token: &str,
    params: &Value,
) -> Result<(), RpcFault> {
    let configured_token = configured_token.trim();
    if configured_token.is_empty() {
        return Err(RpcFault::token_not_configured());
    }

    let params = positional_params(params)?;
    match extract_token_param(params) {
        Some(token) if token == configured_token => Ok(()),
        _ => Err(RpcFault::token_invalid()),
    }
}

fn extract_token_param(params: &[Value]) -> Option<&str> {
    params
        .first()
        .and_then(Value::as_str)
        .and_then(|value| value.strip_prefix("token:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
