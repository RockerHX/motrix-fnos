use super::types::{positional_params, RpcFault};
use super::JsonRpcAccess;
use crate::app::HttpAppState;
use serde_json::Value;
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub(super) async fn ensure_add_uri_token(
    state: &Arc<HttpAppState>,
    access: JsonRpcAccess,
    params: &Value,
) -> Result<(), RpcFault> {
    let token = configured_token(state, access).await;
    validate_add_uri_token(&token, params)
}

pub(super) async fn ensure_global_option_token(
    state: &Arc<HttpAppState>,
    access: JsonRpcAccess,
    params: &Value,
) -> Result<(), RpcFault> {
    let token = configured_token(state, access).await;
    validate_add_uri_token(&token, params)
}

pub(super) async fn ensure_compat_token(
    state: &Arc<HttpAppState>,
    access: JsonRpcAccess,
    params: &Value,
) -> Result<(), RpcFault> {
    let token = configured_token(state, access).await;
    validate_add_uri_token(&token, params)
}

async fn configured_token(state: &HttpAppState, access: JsonRpcAccess) -> String {
    match access {
        JsonRpcAccess::Proxy => state.json_rpc_token(),
        JsonRpcAccess::Lan => state.lan_json_rpc_config().await.token,
    }
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
    let Some(token) = extract_token_param(params) else {
        return Err(RpcFault::token_invalid());
    };
    if bool::from(configured_token.as_bytes().ct_eq(token.as_bytes())) {
        Ok(())
    } else {
        Err(RpcFault::token_invalid())
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
