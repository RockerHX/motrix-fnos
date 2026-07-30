use super::*;
use axum::extract::Json;
use axum::http::header::CONTENT_TYPE;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::Router;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn lifecycle_bound_rpc_client_rejects_requests_during_stop_without_probing() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = calls.clone();
    let (port, handle) = spawn_router(Router::new().route(
        "/jsonrpc",
        post(move || {
            let calls = calls_for_handler.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({ "result": "unexpected" }))
            }
        }),
    ))
    .await;
    let coordinator = Arc::new(crate::runtime::Aria2LifecycleCoordinator::default());
    coordinator
        .set_phase(crate::runtime::Aria2LifecyclePhase::Stopping)
        .expect("lifecycle phase should change");
    let client = Aria2RpcClient::with_lifecycle(coordinator);

    let error = client
        .request::<String>(&test_config(port), &json!({}))
        .await
        .expect_err("stopping lifecycle should reject RPC requests");

    assert!(matches!(error, Aria2RpcError::Lifecycle(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    handle.abort();
}

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

    let status = ping_rpc(&Aria2RpcClient::new(), &config, None).await;

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

    let status = ping_rpc(&Aria2RpcClient::new(), &config, None).await;

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

    let status = ping_rpc(&Aria2RpcClient::new(), &test_config(port), None).await;

    assert!(!status.connected);
    assert!(status.message.contains("响应解析失败"));
    handle.abort();
}

#[tokio::test]
async fn ping_rpc_reports_missing_result() {
    let (config, handle) = config_with_json(json!({ "result": null })).await;

    let status = ping_rpc(&Aria2RpcClient::new(), &config, None).await;

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

    let status = ping_rpc(&Aria2RpcClient::new(), &test_config(port), None).await;

    assert!(!status.connected);
    assert!(status.message.contains("连接失败"));
}

#[tokio::test]
async fn rpc_client_reuses_one_configured_instance_for_multiple_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_for_handler = calls.clone();
    let (port, handle) = spawn_router(Router::new().route(
        "/jsonrpc",
        post(move || {
            let calls = calls_for_handler.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Json(json!({ "result": "ok" }))
            }
        }),
    ))
    .await;
    let client = Aria2RpcClient::new();
    let config = test_config(port);
    let request =
        json!({ "jsonrpc": "2.0", "id": "reuse", "method": "aria2.getVersion", "params": [] });

    for _ in 0..2 {
        let result = client
            .request::<String>(&config, &request)
            .await
            .expect("request should succeed")
            .into_result()
            .expect("response should contain result");
        assert_eq!(result, "ok");
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    handle.abort();
}

#[tokio::test]
async fn rpc_client_classifies_timeout_as_unknown_outcome() {
    let (port, handle) = spawn_router(Router::new().route(
        "/jsonrpc",
        post(|| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Json(json!({ "result": "late" }))
        }),
    ))
    .await;
    let client = Aria2RpcClient::with_timeouts(Duration::from_secs(1), Duration::from_millis(10));

    let error = client
        .request::<String>(&test_config(port), &json!({}))
        .await
        .expect_err("timeout should fail");

    assert!(matches!(error, Aria2RpcError::OutcomeUnknown(_)));
    handle.abort();
}

#[tokio::test]
async fn rpc_client_classifies_disconnect_as_unknown_outcome() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let port = listener.local_addr().expect("addr should exist").port();
    let handle = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("request should connect");
    });
    let client = Aria2RpcClient::with_timeouts(Duration::from_secs(1), Duration::from_millis(100));

    let error = client
        .request::<String>(&test_config(port), &json!({}))
        .await
        .expect_err("disconnect should fail");

    assert!(matches!(error, Aria2RpcError::OutcomeUnknown(_)));
    handle.await.expect("listener task should finish");
}

#[tokio::test]
async fn rpc_client_classifies_explicit_aria2_error() {
    let (config, handle) = config_with_json(json!({
        "error": { "code": 1, "message": "rejected" }
    }))
    .await;

    let error = Aria2RpcClient::new()
        .request::<String>(&config, &json!({}))
        .await
        .expect("JSON response should parse")
        .into_result()
        .expect_err("Aria2 error should fail");

    assert!(matches!(error, Aria2RpcError::Remote(_)));
    assert!(error.to_string().contains("rejected"));
    handle.abort();
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
