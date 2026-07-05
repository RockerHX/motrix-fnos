use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready};
use crate::settings::service::load_app_config_from_pool;
use crate::tasks::service::{RuntimeGuard, TaskService};
use crate::tasks::CreateDownloadTaskRequest;
use axum::body::{Body, Bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::header::{CONTENT_TYPE, SEC_WEBSOCKET_PROTOCOL};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

const JSONRPC_VERSION: &str = "2.0";

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new().route(
        "/jsonrpc",
        post(handle_http_jsonrpc)
            .get(handle_ws_jsonrpc)
            .options(handle_jsonrpc_options),
    )
}

async fn handle_http_jsonrpc(State(state): State<Arc<HttpAppState>>, body: Bytes) -> Response {
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(payload) => handle_jsonrpc_payload(&state, payload).await,
        Err(_) => rpc_error(Value::Null, -32700, "Parse error"),
    };
    jsonrpc_http_response(StatusCode::OK, payload)
}

async fn handle_jsonrpc_options() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    insert_cors_headers(response.headers_mut());
    response
}

async fn handle_ws_jsonrpc(
    State(state): State<Arc<HttpAppState>>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.protocols(["jsonrpc"])
        .on_upgrade(move |socket| handle_jsonrpc_socket(socket, state))
}

async fn handle_jsonrpc_socket(mut socket: WebSocket, state: Arc<HttpAppState>) {
    while let Some(message) = socket.recv().await {
        let Ok(message) = message else {
            break;
        };

        let payload = match message {
            Message::Text(text) => serde_json::from_str::<Value>(&text),
            Message::Binary(bytes) => serde_json::from_slice::<Value>(&bytes),
            Message::Ping(bytes) => {
                let _ = socket.send(Message::Pong(bytes)).await;
                continue;
            }
            Message::Pong(_) => continue,
            Message::Close(_) => break,
        };

        let response = match payload {
            Ok(payload) => handle_jsonrpc_payload(&state, payload).await,
            Err(_) => rpc_error(Value::Null, -32700, "Parse error"),
        };

        if socket
            .send(Message::Text(response.to_string()))
            .await
            .is_err()
        {
            break;
        }
    }
}

async fn handle_jsonrpc_payload(state: &Arc<HttpAppState>, payload: Value) -> Value {
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

async fn execute_method(
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

async fn add_uri(state: &Arc<HttpAppState>, params: &Value) -> Result<String, RpcFault> {
    ensure_add_uri_token(state, params).await?;
    let command = parse_add_uri_command(params)?;
    let save_dir = match command.save_dir {
        Some(save_dir) => save_dir,
        None => default_save_dir(state)?,
    };
    ensure_authorized_save_dir(state, &save_dir)?;

    let service = TaskService::new(
        &state.core.database.pool,
        &state.core.download_tasks,
        &state.core.next_task_id,
        &state.core.debug_logs,
        RuntimeGuard::new(&state.core.shutdown),
    );
    service
        .ensure_not_exiting()
        .map_err(RpcFault::server_error)?;

    let config = ensure_aria2_ready(state)
        .await
        .map_err(RpcFault::server_error)?;
    let task = service
        .create_download_task(
            &config,
            CreateDownloadTaskRequest {
                url: command.url,
                file_name: command.file_name,
                save_dir: Some(save_dir),
                aria2_options: command.aria2_options,
            },
        )
        .await
        .map_err(RpcFault::server_error)?;
    broadcast_tasks_snapshot(state).map_err(RpcFault::server_error)?;

    task.gid
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| RpcFault::server_error("创建下载任务成功，但响应缺少 GID"))
}

async fn get_version(state: &Arc<HttpAppState>) -> Result<Value, RpcFault> {
    let config = ensure_aria2_ready(state)
        .await
        .map_err(RpcFault::server_error)?;
    let status = crate::aria2::ping_rpc(&config, Some(&state.core.debug_logs)).await;
    if !status.connected {
        return Err(RpcFault::server_error(status.message));
    }

    Ok(json!({
        "version": status.version.unwrap_or_else(|| "unknown".to_string()),
        "enabledFeatures": [],
    }))
}

fn parse_add_uri_command(params: &Value) -> Result<AddUriCommand, RpcFault> {
    let params = positional_params(params)?;
    let params = strip_token_param(params);
    let uris = params
        .first()
        .ok_or_else(|| RpcFault::invalid_params("aria2.addUri requires URI list"))?;
    let url = first_uri(uris)?;
    let options = params.get(1).and_then(Value::as_object);

    Ok(AddUriCommand {
        url,
        save_dir: options.and_then(|options| string_option(options.get("dir"))),
        file_name: options.and_then(|options| string_option(options.get("out"))),
        aria2_options: options.map(collect_aria2_options).unwrap_or_default(),
    })
}

fn collect_aria2_options(
    options: &serde_json::Map<String, Value>,
) -> serde_json::Map<String, Value> {
    const PASSTHROUGH_OPTIONS: &[&str] = &[
        "allow-overwrite",
        "auto-file-renaming",
        "check-certificate",
        "connect-timeout",
        "continue",
        "header",
        "lowest-speed-limit",
        "max-connection-per-server",
        "max-download-limit",
        "max-file-not-found",
        "max-tries",
        "min-split-size",
        "referer",
        "retry-wait",
        "split",
        "timeout",
        "user-agent",
    ];

    options
        .iter()
        .filter(|(key, _)| PASSTHROUGH_OPTIONS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            normalize_aria2_option_value(value).map(|value| (key.clone(), value))
        })
        .collect()
}

fn normalize_aria2_option_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Object(_) => None,
        Value::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(Value::String(value.to_string()))
            }
        }
        Value::Array(items) => {
            let normalized = items
                .iter()
                .filter_map(|item| normalize_aria2_option_value(item))
                .collect::<Vec<_>>();
            if normalized.is_empty() {
                None
            } else {
                Some(Value::Array(normalized))
            }
        }
        Value::Bool(_) | Value::Number(_) => Some(value.clone()),
    }
}

fn positional_params(params: &Value) -> Result<&[Value], RpcFault> {
    match params {
        Value::Null => Ok(&[]),
        Value::Array(params) => Ok(params),
        _ => Err(RpcFault::invalid_params("params must be an array")),
    }
}

fn strip_token_param(params: &[Value]) -> &[Value] {
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

async fn ensure_add_uri_token(state: &Arc<HttpAppState>, params: &Value) -> Result<(), RpcFault> {
    let default_download_dir = state.runtime.app_data_dir.display().to_string();
    let config = load_app_config_from_pool(&state.core.database.pool, &default_download_dir)
        .await
        .map_err(RpcFault::server_error)?;

    validate_add_uri_token(&config.json_rpc_token, params)
}

fn validate_add_uri_token(configured_token: &str, params: &Value) -> Result<(), RpcFault> {
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

fn first_uri(value: &Value) -> Result<String, RpcFault> {
    let uri = match value {
        Value::Array(uris) => uris.first().and_then(Value::as_str),
        Value::String(uri) => Some(uri.as_str()),
        _ => None,
    };
    uri.map(str::trim)
        .filter(|uri| !uri.is_empty())
        .map(str::to_string)
        .ok_or_else(|| RpcFault::invalid_params("aria2.addUri requires a non-empty URI"))
}

fn string_option(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn default_save_dir(state: &HttpAppState) -> Result<String, RpcFault> {
    crate::storage::load_default_download_dir(
        &state.runtime.accessible_paths_path,
        &state.runtime.app_data_dir,
    )
    .map_err(RpcFault::server_error)
}

fn ensure_authorized_save_dir(state: &HttpAppState, save_dir: &str) -> Result<(), RpcFault> {
    let accessible_paths =
        crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
            .map_err(RpcFault::server_error)?;
    if accessible_paths.is_empty() {
        return Err(RpcFault::invalid_params(
            "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权",
        ));
    }
    if !accessible_paths.iter().any(|path| path == save_dir) {
        return Err(RpcFault::invalid_params("保存目录不在飞牛已授权目录列表中"));
    }
    Ok(())
}

fn rpc_success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "result": result,
    })
}

fn rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

fn jsonrpc_http_response(status: StatusCode, payload: Value) -> Response {
    let mut response = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .body(Body::from(payload.to_string()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    insert_cors_headers(response.headers_mut());
    response
}

fn insert_cors_headers(headers: &mut axum::http::HeaderMap) {
    headers.insert(
        HeaderName::from_static("access-control-allow-origin"),
        HeaderValue::from_static("*"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-methods"),
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-headers"),
        HeaderValue::from_static("content-type, authorization"),
    );
    headers.insert(
        HeaderName::from_static("access-control-expose-headers"),
        HeaderValue::from_static(SEC_WEBSOCKET_PROTOCOL.as_str()),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MulticallItem {
    method_name: String,
    params: Option<Value>,
}

#[derive(Debug)]
struct AddUriCommand {
    url: String,
    save_dir: Option<String>,
    file_name: Option<String>,
    aria2_options: serde_json::Map<String, Value>,
}

#[derive(Debug)]
struct RpcFault {
    code: i64,
    message: String,
}

impl RpcFault {
    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    fn method_not_found(message: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: message.into(),
        }
    }

    fn server_error(message: impl Into<String>) -> Self {
        Self {
            code: -32000,
            message: message.into(),
        }
    }

    fn token_invalid() -> Self {
        Self {
            code: -32001,
            message: "JSON-RPC token invalid".to_string(),
        }
    }

    fn token_not_configured() -> Self {
        Self {
            code: -32002,
            message: "JSON-RPC token not configured".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
    use crate::database::settings::set_app_config_value;
    use std::path::PathBuf;

    #[test]
    fn validate_add_uri_token_rejects_empty_configured_token() {
        let error = validate_add_uri_token("", &json!([
            "token:anything",
            ["https://example.com/file.zip"]
        ]))
        .expect_err("empty configured token should fail");

        assert_eq!(error.code, -32002);
        assert_eq!(error.message, "JSON-RPC token not configured");
    }

    #[test]
    fn validate_add_uri_token_rejects_missing_or_wrong_token() {
        let missing = validate_add_uri_token("secret", &json!([["https://example.com/file.zip"]]))
            .expect_err("missing token should fail");
        assert_eq!(missing.code, -32001);

        let wrong = validate_add_uri_token("secret", &json!([
            "token:wrong",
            ["https://example.com/file.zip"]
        ]))
        .expect_err("wrong token should fail");
        assert_eq!(wrong.code, -32001);
        assert_eq!(wrong.message, "JSON-RPC token invalid");
    }

    #[test]
    fn validate_add_uri_token_accepts_matching_token() {
        validate_add_uri_token("secret", &json!([
            "token:secret",
            ["https://example.com/file.zip"]
        ]))
        .expect("matching token should pass");
    }

    #[tokio::test]
    async fn multicall_requires_token_for_each_add_uri_call() {
        let state = test_state().await;
        write_json_rpc_token(&state, "secret").await;

        let response = handle_jsonrpc_payload(&state, json!({
            "jsonrpc": "2.0",
            "id": "multi",
            "method": "system.multicall",
            "params": [
                "token:secret",
                [
                    {
                        "methodName": "aria2.addUri",
                        "params": [["https://example.com/missing-token.zip"]]
                    },
                    {
                        "methodName": "aria2.addUri",
                        "params": [
                            "token:secret",
                            ["https://example.com/with-token.zip"],
                            { "dir": "/vol1/not-authorized" }
                        ]
                    }
                ]
            ]
        }))
        .await;

        let results = response["result"]
            .as_array()
            .expect("multicall result should be an array");

        assert_eq!(results[0]["faultCode"], -32001);
        assert_eq!(results[0]["faultString"], "JSON-RPC token invalid");
        assert_eq!(results[1]["faultCode"], -32602);
        assert_eq!(
            results[1]["faultString"],
            "未检测到已授权目录，请先在飞牛应用设置中添加读写文件夹授权"
        );
    }

    #[tokio::test]
    async fn multicall_get_version_does_not_require_json_rpc_token() {
        let state = test_state().await;

        match execute_method(&state, "aria2.getVersion", &json!([])).await {
            Ok(result) => {
                assert!(result.get("version").and_then(Value::as_str).is_some());
                assert!(result.get("enabledFeatures").is_some());
            }
            Err(error) => {
                assert_ne!(error.code, -32001);
                assert_ne!(error.code, -32002);
            }
        }
    }

    #[test]
    fn parse_add_uri_accepts_token_uri_list_and_options() {
        let command = parse_add_uri_command(&json!([
            "token:anything",
            ["https://example.com/file.zip"],
            {
                "dir": "/vol1/1000/tmp",
                "out": "file.zip"
            }
        ]))
        .expect("addUri params should parse");

        assert_eq!(command.url, "https://example.com/file.zip");
        assert_eq!(command.save_dir.as_deref(), Some("/vol1/1000/tmp"));
        assert_eq!(command.file_name.as_deref(), Some("file.zip"));
        assert!(command.aria2_options.is_empty());
    }

    #[test]
    fn parse_add_uri_accepts_uri_without_token() {
        let command = parse_add_uri_command(&json!([
            ["https://example.com/file.zip"],
            {
                "dir": "/vol1/1000/tmp"
            }
        ]))
        .expect("addUri params should parse");

        assert_eq!(command.url, "https://example.com/file.zip");
        assert_eq!(command.save_dir.as_deref(), Some("/vol1/1000/tmp"));
        assert_eq!(command.file_name, None);
    }

    #[test]
    fn parse_add_uri_preserves_speed_related_options() {
        let command = parse_add_uri_command(&json!([
            "token:anything",
            ["https://example.com/file.zip"],
            {
                "dir": "/vol1/1000/tmp",
                "out": "file.zip",
                "split": "256",
                "max-connection-per-server": "256",
                "min-split-size": "1M",
                "user-agent": "Motrix",
                "header": ["Referer: https://example.com"],
                "unknown-option": "ignored"
            }
        ]))
        .expect("addUri params should parse");

        assert_eq!(command.aria2_options["split"], "256");
        assert_eq!(command.aria2_options["max-connection-per-server"], "256");
        assert_eq!(command.aria2_options["min-split-size"], "1M");
        assert_eq!(command.aria2_options["user-agent"], "Motrix");
        assert_eq!(
            command.aria2_options["header"][0],
            "Referer: https://example.com"
        );
        assert!(!command.aria2_options.contains_key("unknown-option"));
        assert!(!command.aria2_options.contains_key("dir"));
    }

    #[test]
    fn parse_add_uri_rejects_empty_uri_list() {
        let error = parse_add_uri_command(&json!([[]])).expect_err("empty URI should fail");

        assert_eq!(error.code, -32602);
    }

    async fn test_state() -> Arc<HttpAppState> {
        let app_data_dir = temp_dir("jsonrpc-api");
        let runtime = ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.sqlite"),
            accessible_paths_path: app_data_dir.join("accessible-paths.json"),
            app_data_dir: app_data_dir.clone(),
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path: None,
        };

        bootstrap_http_app_state(&runtime)
            .await
            .expect("state should bootstrap")
    }

    async fn write_json_rpc_token(state: &Arc<HttpAppState>, token: &str) {
        set_app_config_value(
            &state.core.database.pool,
            "download",
            &json!({
                "defaultDownloadDir": state.runtime.app_data_dir.display().to_string(),
                "maxConcurrentDownloads": 5,
                "downloadLimit": 0,
                "uploadLimit": 0,
                "autoStartEnabled": false,
                "notificationsEnabled": false,
                "language": "zh-CN",
                "jsonRpcToken": token
            }),
        )
        .await
        .expect("JSON-RPC token should save");
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "motrix-fnos-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ))
    }
}
