use super::add_uri::add_uri;
use super::types::{
    positional_params, rpc_error, rpc_success, strip_token_param, JsonRpcRequest, MulticallItem,
    RpcFault,
};
use crate::app::HttpAppState;
use crate::debug_logs::{emit_file_log, DebugLogLevel};
use crate::runtime::process_status;
use serde_json::{json, Value};
use std::sync::Arc;

pub(super) async fn handle_jsonrpc_payload(state: &Arc<HttpAppState>, payload: Value) -> Value {
    match payload {
        Value::Array(items) if items.is_empty() => {
            rpc_error(Value::Null, -32600, "Invalid Request")
        }
        Value::Array(items) => {
            let mut responses = Vec::with_capacity(items.len());
            for item in items {
                responses.push(handle_jsonrpc_request(state, item).await);
            }
            Value::Array(responses)
        }
        Value::Object(_) => handle_jsonrpc_request(state, payload).await,
        _ => rpc_error(Value::Null, -32600, "Invalid Request"),
    }
}

async fn handle_jsonrpc_request(state: &Arc<HttpAppState>, payload: Value) -> Value {
    let request = match serde_json::from_value::<JsonRpcRequest>(payload) {
        Ok(request) => request,
        Err(_) => return rpc_error(Value::Null, -32600, "Invalid Request"),
    };
    let id = request.id.clone().unwrap_or(Value::Null);

    let result = if request.method == "system.multicall" {
        execute_multicall(state, &request.params).await
    } else {
        execute_method(state, &request.method, &request.params).await
    };

    match result {
        Ok(result) => rpc_success(id, result),
        Err(error) => rpc_error(id, error.code, error.message),
    }
}

async fn execute_multicall(state: &Arc<HttpAppState>, params: &Value) -> Result<Value, RpcFault> {
    let params = positional_params(params)?;
    let params = strip_token_param(params);
    let calls = params
        .first()
        .and_then(Value::as_array)
        .ok_or_else(|| RpcFault::invalid_params("system.multicall requires a call list"))?;

    let mut results = Vec::with_capacity(calls.len());
    for call in calls {
        let call = serde_json::from_value::<MulticallItem>(call.clone())
            .map_err(|_| RpcFault::invalid_params("Invalid multicall item"))?;
        let params = call.params.unwrap_or(Value::Array(Vec::new()));
        match execute_method(state, &call.method_name, &params).await {
            Ok(result) => results.push(json!([result])),
            Err(error) => results.push(json!({
                "faultCode": error.code,
                "faultString": error.message,
            })),
        }
    }

    Ok(Value::Array(results))
}

pub(super) async fn execute_method(
    state: &Arc<HttpAppState>,
    method: &str,
    params: &Value,
) -> Result<Value, RpcFault> {
    match method {
        "aria2.addUri" => add_uri(state, params).await.map(Value::String),
        "aria2.getVersion" => get_version(state).await,
        _ => Err(RpcFault::method_not_found(format!(
            "Method not found: {method}"
        ))),
    }
}

async fn get_version(state: &Arc<HttpAppState>) -> Result<Value, RpcFault> {
    if state
        .aria2_lifecycle
        .snapshot()
        .map_err(RpcFault::server_error)?
        .phase
        == crate::runtime::Aria2LifecyclePhase::Stopping
    {
        return Err(RpcFault::aria2_busy(
            "Aria2 正在停止，请稍后重试".to_string(),
        ));
    }
    let process = process_status(&state.aria2_process).map_err(RpcFault::server_error)?;
    let Some(runtime) = state.aria2_runtime_snapshot() else {
        return Ok(version_result(state.last_aria2_version()));
    };
    if !process.running || process.pid != Some(runtime.pid) {
        return Ok(version_result(state.last_aria2_version()));
    }

    let config = state.aria2_config();
    let status = crate::aria2::ping_rpc(&state.aria2_rpc, &config, None).await;
    if !status.connected {
        emit_file_log(
            DebugLogLevel::Warn,
            "aria2.rpc",
            &format!("aria2.getVersion 调用失败：{}", status.message),
        );
        return Err(RpcFault::server_error(status.message));
    }

    if let Some(version) = status.version.as_deref() {
        state.remember_aria2_version(version);
    }

    Ok(version_result(status.version))
}

fn version_result(version: Option<String>) -> Value {
    json!({
        "version": version.unwrap_or_else(|| "unknown".to_string()),
        "enabledFeatures": [],
    })
}
