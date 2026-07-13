use super::transport::{rpc_params, GidResponse};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{log_error, log_info};

pub async fn pause_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        config,
        gid,
        "aria2.pause",
        "motrix-fnos-pause",
        "暂停任务",
        debug_logs,
    )
    .await
}

pub async fn unpause_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        config,
        gid,
        "aria2.unpause",
        "motrix-fnos-unpause",
        "恢复任务",
        debug_logs,
    )
    .await
}

pub async fn change_task_options(
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let request_body = super::build_change_option_request(config, gid, options);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "更新任务选项失败：无法连接 Aria2 RPC".to_string();
            log_error(debug_logs, "aria2.changeOption", &error);
            return Err(error);
        }
    };

    let rpc_response = match response.json::<GidResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("更新任务选项失败，响应解析失败：{}", error);
            log_error(debug_logs, "aria2.changeOption", &error);
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("更新任务选项失败：{}", error.message);
        log_error(debug_logs, "aria2.changeOption", &error);
        return Err(error);
    }

    Ok(rpc_response.result.unwrap_or_else(|| gid.to_string()))
}

pub async fn remove_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    match send_gid_control_request(
        config,
        gid,
        "aria2.remove",
        "motrix-fnos-remove",
        "删除任务",
        debug_logs,
    )
    .await
    {
        Ok(result_gid) => Ok(result_gid),
        Err(error) => {
            log_info(
                debug_logs,
                "aria2.removeDownloadResult",
                format!(
                    "aria2.remove 未完成，尝试清理已停止任务结果，GID {}：{}",
                    gid, error
                ),
            );
            send_gid_control_request(
                config,
                gid,
                "aria2.removeDownloadResult",
                "motrix-fnos-remove-result",
                "删除任务结果",
                debug_logs,
            )
            .await
        }
    }
}

pub(crate) async fn send_gid_control_request(
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
    action_label: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let module = method;
    log_info(
        debug_logs,
        module,
        format!("开始{}，GID {}", action_label, gid),
    );
    let request_body = super::build_gid_control_request(config, gid, method, request_id);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = format!("{}失败：无法连接 Aria2 RPC", action_label);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    let rpc_response = match response.json::<GidResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("{}失败，响应解析失败：{}", action_label, error);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("{}失败：{}", action_label, error.message);
        log_error(debug_logs, module, format!("GID {} {}", gid, error));
        return Err(error);
    }

    let result_gid = rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| format!("{}失败：响应缺少 GID", action_label))?;
    log_info(
        debug_logs,
        module,
        format!("{}成功，GID {}", action_label, result_gid),
    );
    Ok(result_gid)
}

pub(crate) fn build_gid_control_request(
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params,
    })
}

pub(crate) fn build_change_option_request(
    config: &Aria2Config,
    gid: &str,
    options: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut params = rpc_params(config);
    params.push(serde_json::json!(gid));
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-change-option",
        "method": "aria2.changeOption",
        "params": params,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Json, State};
    use axum::http::header::CONTENT_TYPE;
    use axum::response::{IntoResponse, Response};
    use axum::routing::post;
    use axum::Router;
    use serde_json::{json, Value};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn gid_control_rejects_empty_result() {
        let mock = MockRpcServer::spawn(vec![MockResponse::Json(json!({ "result": "" }))]).await;

        let error = send_gid_control_request(
            &test_config(mock.port),
            "gid-1",
            "aria2.pause",
            "pause-test",
            "暂停任务",
            None,
        )
        .await
        .expect_err("empty gid should fail");

        assert!(error.contains("响应缺少 GID"));
        mock.abort();
    }

    #[tokio::test]
    async fn gid_control_reports_rpc_and_parse_errors() {
        let rpc_mock = MockRpcServer::spawn(vec![MockResponse::Json(json!({
            "error": { "message": "cannot pause" }
        }))])
        .await;
        let parse_mock = MockRpcServer::spawn(vec![MockResponse::Raw("{")]).await;

        let rpc_error = pause_task(&test_config(rpc_mock.port), "gid-1", None)
            .await
            .expect_err("rpc error should fail");
        let parse_error = pause_task(&test_config(parse_mock.port), "gid-1", None)
            .await
            .expect_err("invalid json should fail");

        assert!(rpc_error.contains("cannot pause"));
        assert!(parse_error.contains("响应解析失败"));
        rpc_mock.abort();
        parse_mock.abort();
    }

    #[tokio::test]
    async fn remove_task_falls_back_to_remove_download_result() {
        let mock = MockRpcServer::spawn(vec![
            MockResponse::Json(json!({ "error": { "message": "task stopped" } })),
            MockResponse::Json(json!({ "result": "gid-1" })),
        ])
        .await;

        let result = remove_task(&test_config(mock.port), "gid-1", None)
            .await
            .expect("fallback should succeed");

        assert_eq!(result, "gid-1");
        assert_eq!(
            *mock.methods.lock().expect("methods should lock"),
            vec!["aria2.remove", "aria2.removeDownloadResult"]
        );
        mock.abort();
    }

    enum MockResponse {
        Json(Value),
        Raw(&'static str),
    }

    struct MockRpcState {
        responses: Mutex<VecDeque<MockResponse>>,
        methods: Arc<Mutex<Vec<String>>>,
    }

    struct MockRpcServer {
        port: u16,
        methods: Arc<Mutex<Vec<String>>>,
        handle: tokio::task::JoinHandle<()>,
    }

    impl MockRpcServer {
        async fn spawn(responses: Vec<MockResponse>) -> Self {
            let methods = Arc::new(Mutex::new(Vec::new()));
            let state = Arc::new(MockRpcState {
                responses: Mutex::new(responses.into()),
                methods: methods.clone(),
            });
            let router = Router::new()
                .route("/jsonrpc", post(mock_rpc))
                .with_state(state);
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
            Self {
                port,
                methods,
                handle,
            }
        }

        fn abort(self) {
            self.handle.abort();
        }
    }

    async fn mock_rpc(
        State(state): State<Arc<MockRpcState>>,
        Json(payload): Json<Value>,
    ) -> Response {
        state
            .methods
            .lock()
            .expect("methods should lock")
            .push(payload["method"].as_str().unwrap_or_default().to_string());
        match state
            .responses
            .lock()
            .expect("responses should lock")
            .pop_front()
            .expect("mock response should exist")
        {
            MockResponse::Json(payload) => Json(payload).into_response(),
            MockResponse::Raw(body) => ([(CONTENT_TYPE, "application/json")], body).into_response(),
        }
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
