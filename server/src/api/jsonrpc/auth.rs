use super::types::{positional_params, RpcFault};
use crate::app::HttpAppState;
use crate::settings::service::load_json_rpc_token;
use serde_json::Value;
use std::sync::Arc;

pub(super) async fn ensure_add_uri_token(
    state: &Arc<HttpAppState>,
    params: &Value,
) -> Result<(), RpcFault> {
    let token = load_json_rpc_token(&state.core.database.pool)
        .await
        .map_err(RpcFault::server_error)?;

    validate_add_uri_token(&token, params)
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
