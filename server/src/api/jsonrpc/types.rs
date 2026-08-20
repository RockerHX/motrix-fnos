use serde::Deserialize;
use serde_json::{json, Value};

const JSONRPC_VERSION: &str = "2.0";

#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcRequest {
    pub(super) id: Option<Value>,
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MulticallItem {
    pub(super) method_name: String,
    pub(super) params: Option<Value>,
}

#[derive(Debug)]
pub(super) struct RpcFault {
    pub(super) code: i64,
    pub(super) message: String,
}

impl RpcFault {
    pub(super) fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    pub(super) fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
        }
    }

    pub(super) fn server_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
        }
    }

    pub(super) fn aria2_busy(message: impl Into<String>) -> Self {
        Self {
            code: -32004,
            message: message.into(),
        }
    }

    pub(super) fn gid_not_found(gid: &str) -> Self {
        Self {
            code: -32003,
            message: format!("Download task not found for GID {gid}"),
        }
    }

    pub(super) fn token_invalid() -> Self {
        Self {
            code: -32001,
            message: "JSON-RPC token invalid".to_string(),
        }
    }

    pub(super) fn token_not_configured() -> Self {
        Self {
            code: -32002,
            message: "JSON-RPC token not configured".to_string(),
        }
    }
}

pub(super) fn rpc_success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result,
    })
}

pub(super) fn rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

pub(super) fn positional_params(params: &Value) -> Result<&[Value], RpcFault> {
    match params {
        Value::Null => Ok(&[]),
        Value::Array(params) => Ok(params),
        _ => Err(RpcFault::invalid_params("params must be an array")),
    }
}

pub(super) fn strip_token_param(params: &[Value]) -> &[Value] {
    if params
        .first()
        .and_then(Value::as_str)
        .map(|value| value.starts_with("token:"))
        .unwrap_or(false)
    {
        &params[1..]
    } else {
        params
    }
}
