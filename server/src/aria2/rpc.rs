use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<Aria2VersionResult>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct EmptyJsonRpcResponse {
    pub(super) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2VersionResult {
    version: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcError {
    pub(super) message: String,
}

pub async fn ping_rpc(config: &Aria2Config, debug_logs: Option<&DebugLogStore>) -> Aria2RpcStatus {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(format!("token:{}", config.rpc_secret));
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-version-check",
        "method": "aria2.getVersion",
        "params": params,
    });

    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.warn("aria2.rpc", format!("Aria2 RPC 暂不可用：{}", error));
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: format!("Aria2 RPC 连接失败：{}", error),
            };
        }
    };

    let rpc_response = match response.json::<JsonRpcResponse>().await {
        Ok(body) => body,
        Err(error) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.error("aria2.rpc", format!("Aria2 RPC 响应解析失败：{}", error));
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: format!("Aria2 RPC 响应解析失败：{}", error),
            };
        }
    };

    if let Some(error) = rpc_response.error {
        if let Some(debug_logs) = debug_logs {
            debug_logs.error(
                "aria2.rpc",
                format!("Aria2 RPC 返回错误：{}", error.message),
            );
        }
        return Aria2RpcStatus {
            connected: false,
            version: None,
            message: format!("Aria2 RPC 返回错误：{}", error.message),
        };
    }

    match rpc_response.result {
        Some(result) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.info(
                    "aria2.rpc",
                    format!("Aria2 RPC ready，版本 {}", result.version),
                );
            }
            Aria2RpcStatus {
                connected: true,
                version: Some(result.version.clone()),
                message: format!("Aria2 RPC 连接正常，版本 {}", result.version),
            }
        }
        None => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.error("aria2.rpc", "Aria2 RPC 响应缺少版本信息");
            }
            Aria2RpcStatus {
                connected: false,
                version: None,
                message: "Aria2 RPC 响应缺少版本信息".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Json;
    use axum::http::header::CONTENT_TYPE;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use axum::Router;
    use serde_json::{json, Value};
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn ping_rpc_accepts_version_and_sends_configured_token() {
        let captured = Arc::new(Mutex::new(None));
        let captured_for_handler = captured.clone();
        let (port, handle) = spawn_router(Router::new().route(
            "/jsonrpc",
            post(move |Json(payload): Json<Value>| {
                let captured = captured_for_handler.clone();
                async move {
                    *captured.lock().expect("captured payload should lock") = Some(payload);
                    Json(json!({ "result": { "version": "2.4.9" } }))
                }
            }),
        ))
        .await;
        let mut config = test_config(port);
        config.rpc_secret = "secret".to_string();

        let status = ping_rpc(&config, None).await;

        assert!(status.connected);
        assert_eq!(status.version.as_deref(), Some("2.4.9"));
        assert_eq!(
            captured
                .lock()
                .expect("captured payload should lock")
                .as_ref()
                .expect("payload should be captured")["params"][0],
            "token:secret"
        );
        handle.abort();
    }

    #[tokio::test]
    async fn ping_rpc_reports_rpc_error() {
        let (config, handle) = config_with_json(json!({
            "error": { "message": "unauthorized" }
        }))
        .await;

        let status = ping_rpc(&config, None).await;

        assert!(!status.connected);
        assert!(status.message.contains("unauthorized"));
        handle.abort();
    }

    #[tokio::test]
    async fn ping_rpc_reports_invalid_json() {
        let (port, handle) = spawn_router(Router::new().route(
            "/jsonrpc",
            post(|| async { ([(CONTENT_TYPE, "application/json")], "{").into_response() }),
        ))
        .await;

        let status = ping_rpc(&test_config(port), None).await;

        assert!(!status.connected);
        assert!(status.message.contains("响应解析失败"));
        handle.abort();
    }

    #[tokio::test]
    async fn ping_rpc_reports_missing_result() {
        let (config, handle) = config_with_json(json!({ "result": null })).await;

        let status = ping_rpc(&config, None).await;

        assert!(!status.connected);
        assert_eq!(status.message, "Aria2 RPC 响应缺少版本信息");
        handle.abort();
    }

    #[tokio::test]
    async fn ping_rpc_reports_connection_failure() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("unused port should bind");
        let port = listener.local_addr().expect("addr should exist").port();
        drop(listener);

        let status = ping_rpc(&test_config(port), None).await;

        assert!(!status.connected);
        assert!(status.message.contains("连接失败"));
    }

    async fn config_with_json(payload: Value) -> (Aria2Config, tokio::task::JoinHandle<()>) {
        let (port, handle) = spawn_router(Router::new().route(
            "/jsonrpc",
            post(move || {
                let payload = payload.clone();
                async move { Json(payload) }
            }),
        ))
        .await;
        (test_config(port), handle)
    }

    async fn spawn_router(router: Router) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock listener should bind");
        let port = listener
            .local_addr()
            .expect("mock addr should exist")
            .port();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("mock rpc server should serve");
        });
        (port, handle)
    }

    fn test_config(port: u16) -> Aria2Config {
        Aria2Config {
            aria2_path: None,
            binary_source: crate::config::aria2::Aria2BinarySource::Sidecar,
            sidecar_name: "aria2-next".to_string(),
            target_triple: "test-target".to_string(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: port,
            rpc_secret: String::new(),
            session_path: None,
            log_path: None,
        }
    }
}
