use super::*;
use crate::aria2::Aria2RpcClient;
use axum::extract::{Json, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[test]
fn get_option_request_targets_one_gid() {
    let request = build_get_option_request_with_id(&test_config(6800), "gid-1", "test-get-option");

    assert_eq!(request["method"], "aria2.getOption");
    assert_eq!(request["params"][0], "gid-1");
}

#[tokio::test]
async fn gid_control_rejects_empty_result() {
    let mock = MockRpcServer::spawn(vec![MockResponse::Json(json!({ "result": "" }))]).await;

    let error = send_gid_control_request(
        &Aria2RpcClient::new(),
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

    let rpc_error = pause_task(
        &Aria2RpcClient::new(),
        &test_config(rpc_mock.port),
        "gid-1",
        None,
    )
    .await
    .expect_err("rpc error should fail");
    let parse_error = pause_task(
        &Aria2RpcClient::new(),
        &test_config(parse_mock.port),
        "gid-1",
        None,
    )
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

    let result = remove_task(
        &Aria2RpcClient::new(),
        &test_config(mock.port),
        "gid-1",
        None,
    )
    .await
    .expect("fallback should succeed");

    assert_eq!(result, "gid-1");
    assert_eq!(
        *mock.methods.lock().expect("methods should lock"),
        vec!["aria2.remove", "aria2.removeDownloadResult"]
    );
    mock.abort();
}

#[tokio::test]
async fn get_task_options_returns_structured_options() {
    let mock = MockRpcServer::spawn(vec![MockResponse::Json(json!({
        "result": { "all-proxy": "http://127.0.0.1:7890/" }
    }))])
    .await;

    let options = get_task_options(
        &Aria2RpcClient::new(),
        &test_config(mock.port),
        "gid-1",
        Some("get-option-test"),
        None,
    )
    .await
    .expect("task options should load");

    assert_eq!(options["all-proxy"], "http://127.0.0.1:7890/");
    assert_eq!(
        *mock.methods.lock().expect("methods should lock"),
        vec!["aria2.getOption"]
    );
    mock.abort();
}

#[tokio::test]
async fn task_option_error_does_not_expose_proxy_credentials() {
    let mock = MockRpcServer::spawn(vec![MockResponse::Json(json!({
        "error": {
            "code": 1,
            "message": "rejected http://proxy-user:proxy-password@proxy.example.com:7890/"
        }
    }))])
    .await;
    let logs = DebugLogStore::default();

    let error = change_task_options(
        &Aria2RpcClient::new(),
        &test_config(mock.port),
        "gid-1",
        serde_json::Map::from_iter([(
            "all-proxy".to_string(),
            json!("http://proxy-user:proxy-password@proxy.example.com:7890/"),
        )]),
        Some(&logs),
    )
    .await
    .expect_err("remote option failure should be sanitized");

    assert!(!error.contains("proxy-user"));
    assert!(!error.contains("proxy-password"));
    assert!(logs.list().iter().all(|entry| {
        !entry.message.contains("proxy-user") && !entry.message.contains("proxy-password")
    }));
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

async fn mock_rpc(State(state): State<Arc<MockRpcState>>, Json(payload): Json<Value>) -> Response {
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
