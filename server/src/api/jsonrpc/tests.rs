use super::add_uri::{authorized_save_dir, parse_add_uri_command, resolve_authorized_save_dir};
use super::auth::validate_add_uri_token;
use super::methods::{
    execute_method, execute_method_with_access, handle_jsonrpc_payload,
    handle_jsonrpc_payload_with_access,
};
use super::JsonRpcAccess;
use crate::app::HttpAppState;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::debug_logs::DebugLogLevel;
use crate::runtime::{auto_stop_aria2, stop_aria2, stop_process, ManagedAria2Process};
use crate::settings::service::save_json_rpc_token;
use crate::test_support::TestTracingCapture;
use axum::body::{to_bytes, Body};
use axum::http::header::CONTENT_LENGTH;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Notify};
use tokio::time::timeout;
use tower::ServiceExt;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn validate_add_uri_token_rejects_empty_configured_token() {
    let error = validate_add_uri_token(
        "",
        &json!(["token:anything", ["https://example.com/file.zip"]]),
    )
    .expect_err("empty configured token should fail");

    assert_eq!(error.code, -32002);
    assert_eq!(error.message, "JSON-RPC token not configured");
}

#[test]
fn validate_add_uri_token_rejects_missing_or_wrong_token() {
    let missing = validate_add_uri_token("secret", &json!([["https://example.com/file.zip"]]))
        .expect_err("missing token should fail");
    assert_eq!(missing.code, -32001);

    let wrong = validate_add_uri_token(
        "secret",
        &json!(["token:wrong", ["https://example.com/file.zip"]]),
    )
    .expect_err("wrong token should fail");
    assert_eq!(wrong.code, -32001);
    assert_eq!(wrong.message, "JSON-RPC token invalid");
}

#[test]
fn validate_add_uri_token_accepts_matching_token() {
    validate_add_uri_token(
        "secret",
        &json!(["token:secret", ["https://example.com/file.zip"]]),
    )
    .expect("matching token should pass");
}

#[tokio::test]
async fn public_and_lan_tokens_are_rejected_across_entry_scopes() {
    let state = test_state().await;
    state.remember_json_rpc_token("public-secret");
    *state.lan_json_rpc_config.write().await = crate::settings::service::LanJsonRpcConfig {
        enabled: true,
        token: "lan-secret".to_string(),
    };

    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Proxy,
        "aria2.getGlobalOption",
        &json!(["token:public-secret"]),
    )
    .await
    .is_ok());
    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Proxy,
        "aria2.getGlobalOption",
        &json!(["token:lan-secret"]),
    )
    .await
    .is_err());
    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Lan,
        "aria2.getGlobalOption",
        &json!(["token:lan-secret"]),
    )
    .await
    .is_ok());
    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Lan,
        "aria2.getGlobalOption",
        &json!(["token:public-secret"]),
    )
    .await
    .is_err());

    let anonymous_version =
        execute_method_with_access(&state, JsonRpcAccess::Lan, "aria2.getVersion", &json!([]))
            .await
            .expect("getVersion should remain anonymous");
    assert!(anonymous_version.get("version").is_some());

    let multicall = handle_jsonrpc_payload_with_access(
        &state,
        JsonRpcAccess::Lan,
        json!({
            "jsonrpc": "2.0",
            "id": "lan-multicall",
            "method": "system.multicall",
            "params": [[
                {
                    "methodName": "aria2.getGlobalOption",
                    "params": ["token:lan-secret"]
                },
                {
                    "methodName": "aria2.getGlobalOption",
                    "params": ["token:public-secret"]
                }
            ]]
        }),
    )
    .await;
    assert_eq!(
        multicall["result"][0][0]["dir"],
        state.json_rpc_default_download_dir()
    );
    assert_eq!(multicall["result"][1]["faultCode"], -32001);
}

#[tokio::test]
async fn token_validation_uses_memory_after_database_is_closed() {
    let state = test_state().await;
    state.remember_json_rpc_token("public-secret");
    *state.lan_json_rpc_config.write().await = crate::settings::service::LanJsonRpcConfig {
        enabled: true,
        token: "lan-secret".to_string(),
    };
    state.core.database.pool.close().await;

    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Proxy,
        "aria2.getGlobalOption",
        &json!(["token:public-secret"]),
    )
    .await
    .is_ok());
    assert!(execute_method_with_access(
        &state,
        JsonRpcAccess::Lan,
        "aria2.getGlobalOption",
        &json!(["token:lan-secret"]),
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn jsonrpc_http_enforces_one_mebibyte_body_limit() {
    let state = test_state().await;
    let app = super::super::jsonrpc_router(state);

    for (size, expected_status) in [
        (super::super::API_BODY_LIMIT, StatusCode::OK),
        (
            super::super::API_BODY_LIMIT + 1,
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let body = padded_jsonrpc_payload(size);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/jsonrpc")
                    .header("content-type", "application/json")
                    .header(CONTENT_LENGTH, body.len())
                    .body(Body::from(body))
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), expected_status, "body size: {size}");
    }
}

#[tokio::test]
async fn jsonrpc_requests_receive_server_request_ids() {
    let state = test_state().await;
    let app = super::super::jsonrpc_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jsonrpc")
                .header("x-request-id", "client-supplied-id")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");

    let request_id = response
        .headers()
        .get("x-request-id")
        .expect("request ID should exist")
        .to_str()
        .expect("request ID should be text");
    assert!(request_id.starts_with("req-"));
    assert_ne!(request_id, "client-supplied-id");
}

#[tokio::test]
async fn jsonrpc_websocket_rejects_oversized_frames_and_messages() {
    let oversized_frame = vec![b'x'; super::JSONRPC_WEBSOCKET_MESSAGE_LIMIT + 1];
    assert_websocket_frames_rejected(vec![(true, 0x1, oversized_frame)]).await;

    let fragment = vec![b'x'; super::JSONRPC_WEBSOCKET_MESSAGE_LIMIT / 2 + 1];
    assert_websocket_frames_rejected(vec![(false, 0x1, fragment.clone()), (true, 0x0, fragment)])
        .await;
}

#[tokio::test]
async fn multicall_requires_token_for_each_add_uri_call() {
    let state = test_state().await;
    write_json_rpc_token(&state, "secret").await;

    let response = handle_jsonrpc_payload(
        &state,
        json!({
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
        }),
    )
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
async fn get_version_returns_stopped_state_without_starting_aria2() {
    let state = test_state().await;

    let result = execute_method(&state, "aria2.getVersion", &json!([]))
        .await
        .expect("stopped Aria2 should return a compatibility version result");

    assert_eq!(result["version"], "unknown");
    assert_eq!(result["enabledFeatures"], json!([]));
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
}

#[tokio::test]
async fn get_global_option_requires_token_and_uses_memory_snapshot() {
    let state = test_state().await;

    let unconfigured = execute_method(&state, "aria2.getGlobalOption", &json!(["token:secret"]))
        .await
        .expect_err("unconfigured token should fail");
    assert_eq!(unconfigured.code, -32002);

    write_json_rpc_token(&state, "secret").await;
    let without_authorized_dir =
        execute_method(&state, "aria2.getGlobalOption", &json!(["token:secret"]))
            .await
            .expect("missing authorization should return an empty compatibility directory");
    assert_eq!(without_authorized_dir, json!({ "dir": "" }));

    state.remember_json_rpc_default_download_dir("/vol1/1000/tmp");
    state.core.database.pool.close().await;

    let invalid = execute_method(&state, "aria2.getGlobalOption", &json!(["token:wrong"]))
        .await
        .expect_err("wrong token should fail");
    assert_eq!(invalid.code, -32001);

    let result = execute_method(&state, "aria2.getGlobalOption", &json!(["token:secret"]))
        .await
        .expect("cached global options should be returned without database access");
    assert_eq!(result, json!({ "dir": "/vol1/1000/tmp" }));
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());

    let multicall = handle_jsonrpc_payload(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": "global-option-multicall",
            "method": "system.multicall",
            "params": [[{
                "methodName": "aria2.getGlobalOption",
                "params": ["token:secret"]
            }]]
        }),
    )
    .await;
    assert_eq!(multicall["result"][0][0]["dir"], "/vol1/1000/tmp");
}

#[tokio::test]
async fn get_version_returns_retryable_busy_state_during_aria2_stop() {
    let state = test_state().await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Stopping)
        .expect("lifecycle should enter stopping");

    let error = execute_method(&state, "aria2.getVersion", &json!([]))
        .await
        .expect_err("stopping Aria2 should return a retryable protocol error");

    assert_eq!(error.code, -32004);
    assert_eq!(error.message, "Aria2 正在停止，请稍后重试");
}

#[tokio::test(flavor = "current_thread")]
async fn get_version_returns_real_version_for_confirmed_running_aria2() {
    let state = test_state().await;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let port = listener
        .local_addr()
        .expect("mock addr should exist")
        .port();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/jsonrpc",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "jsonrpc": "2.0",
                    "id": "motrix-fnos-version-check",
                    "result": { "version": "2.4.9" }
                }))
            }),
        );
        axum::serve(listener, app)
            .await
            .expect("mock server should serve");
    });

    let child = spawn_long_running_child();
    let pid = child.id();
    state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .replace(ManagedAria2Process::new(
            child,
            crate::config::aria2::Aria2BinarySource::Sidecar,
        ));

    let config = crate::aria2::runtime_config(&state.base_aria2_config, port, "secret".to_string());
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            crate::config::aria2::Aria2BinarySource::Sidecar,
            Vec::new(),
        ))
        .expect("runtime should persist");

    let capture = TestTracingCapture::default();
    let tracing_guard = tracing::subscriber::set_default(capture.subscriber());
    for _ in 0..2 {
        let result = execute_method(&state, "aria2.getVersion", &json!([]))
            .await
            .expect("running Aria2 should return its version");
        assert_eq!(result["version"], "2.4.9");
        assert_eq!(result["enabledFeatures"], json!([]));
    }
    assert_eq!(capture.contents(), "");
    drop(tracing_guard);

    stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test Aria2 process should stop");
    state.clear_aria2_runtime();

    let cached_result = execute_method(&state, "aria2.getVersion", &json!([]))
        .await
        .expect("stopped Aria2 should return the last observed version");
    assert_eq!(cached_result["version"], "2.4.9");
    assert_eq!(cached_result["enabledFeatures"], json!([]));

    server.abort();
    let _ = server.await;
}

#[tokio::test(flavor = "current_thread")]
async fn get_version_logs_confirmed_rpc_failure_as_warning() {
    let state = test_state().await;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let port = listener
        .local_addr()
        .expect("mock addr should exist")
        .port();
    let server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/jsonrpc",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "jsonrpc": "2.0",
                    "id": "motrix-fnos-version-check",
                    "error": { "message": "rpc unavailable" }
                }))
            }),
        );
        axum::serve(listener, app)
            .await
            .expect("mock server should serve");
    });

    let child = spawn_long_running_child();
    let pid = child.id();
    state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .replace(ManagedAria2Process::new(
            child,
            crate::config::aria2::Aria2BinarySource::Sidecar,
        ));
    let config = crate::aria2::runtime_config(&state.base_aria2_config, port, "secret".to_string());
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            crate::config::aria2::Aria2BinarySource::Sidecar,
            Vec::new(),
        ))
        .expect("runtime should persist");

    state.core.debug_logs.clear();
    let error = execute_method(&state, "aria2.getVersion", &json!([]))
        .await
        .expect_err("RPC failure should return a protocol error");
    assert_eq!(error.code, -32000);
    assert!(
        error.message.contains("rpc unavailable"),
        "{}",
        error.message
    );
    let logs = state.core.debug_logs.list();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].level, DebugLogLevel::Warn);
    assert_eq!(logs[0].module, "aria2.rpc");
    assert!(logs[0].message.contains("aria2.getVersion 调用失败"));
    assert!(logs[0].message.contains("rpc unavailable"));

    stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test Aria2 process should stop");
    state.clear_aria2_runtime();
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn protocol_regression_keeps_add_uri_contract_across_all_rpc_entries() {
    let rpc_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock RPC listener should bind");
    let rpc_port = rpc_listener
        .local_addr()
        .expect("mock RPC address should exist")
        .port();
    let rpc_server = tokio::spawn(async move {
        axum::serve(
            rpc_listener,
            axum::Router::new().route("/jsonrpc", axum::routing::post(protocol_regression_rpc)),
        )
        .await
        .expect("mock RPC server should serve");
    });

    let state = test_state().await;
    let save_dir = state.runtime.app_data_dir.join("protocol-downloads");
    let absolute_save_dir = save_dir.display().to_string();
    let relative_save_dir = absolute_save_dir
        .strip_prefix('/')
        .expect("test save directory should be absolute")
        .to_string();
    std::fs::create_dir_all(&save_dir).expect("protocol save directory should create");
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&json!({
            "paths": [absolute_save_dir]
        }))
        .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    write_json_rpc_token(&state, "secret").await;

    let child = spawn_long_running_child();
    let pid = child.id();
    state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .replace(ManagedAria2Process::new(
            child,
            crate::config::aria2::Aria2BinarySource::ExternalPath,
        ));
    let config =
        crate::aria2::runtime_config(&state.base_aria2_config, rpc_port, "secret".to_string());
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            crate::config::aria2::Aria2BinarySource::ExternalPath,
            Vec::new(),
        ))
        .expect("runtime should persist");
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");

    let http_response = super::super::jsonrpc_router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": "http-add-uri",
                        "method": "aria2.addUri",
                        "params": [
                            "token:secret",
                            ["https://example.com/http.zip"],
                            { "dir": relative_save_dir, "out": "http.zip" }
                        ]
                    })
                    .to_string(),
                ))
                .expect("HTTP request should build"),
        )
        .await
        .expect("HTTP response should succeed");
    assert_eq!(http_response.status(), StatusCode::OK);
    let http_payload: Value = serde_json::from_slice(
        &to_bytes(http_response.into_body(), usize::MAX)
            .await
            .expect("HTTP body should read"),
    )
    .expect("HTTP payload should parse");
    assert_eq!(http_payload["result"], "gid-protocol");
    let tasks = state.core.download_tasks.list().expect("tasks should list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].save_dir, save_dir.display().to_string());

    let multicall_payload = handle_jsonrpc_payload(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": "multicall-add-uri",
            "method": "system.multicall",
            "params": [
                "token:secret",
                [{
                    "methodName": "aria2.addUri",
                    "params": [
                        "token:secret",
                        ["https://example.com/multicall.zip"],
                        { "dir": save_dir, "out": "multicall.zip" }
                    ]
                }]
            ]
        }),
    )
    .await;
    assert_eq!(multicall_payload["result"][0][0], "gid-protocol");

    let websocket_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("WebSocket listener should bind");
    let websocket_addr = websocket_listener
        .local_addr()
        .expect("WebSocket address should exist");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let websocket_state = state.clone();
    let websocket_server = tokio::spawn(async move {
        axum::serve(
            websocket_listener,
            super::super::jsonrpc_router(websocket_state),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("WebSocket server should stop cleanly");
    });

    let mut socket = connect_websocket(websocket_addr).await;
    write_masked_websocket_frame(
        &mut socket,
        true,
        0x1,
        json!({
            "jsonrpc": "2.0",
            "id": "websocket-add-uri",
            "method": "aria2.addUri",
            "params": [
                "token:secret",
                ["https://example.com/websocket.zip"],
                { "dir": save_dir, "out": "websocket.zip" }
            ]
        })
        .to_string()
        .as_bytes(),
    )
    .await;
    let websocket_payload: Value =
        serde_json::from_str(&read_websocket_text_frame(&mut socket).await)
            .expect("WebSocket payload should parse");
    assert_eq!(websocket_payload["result"], "gid-protocol");
    drop(socket);

    shutdown_tx
        .send(())
        .expect("WebSocket server should accept shutdown");
    websocket_server
        .await
        .expect("WebSocket server task should join");
    stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test Aria2 process should stop");
    state.clear_aria2_runtime();
    state.core.database.pool.close().await;
    rpc_server.abort();
    let _ = rpc_server.await;
    let _ = std::fs::remove_dir_all(&state.runtime.app_data_dir);
}

#[tokio::test]
async fn quiescing_auto_stop_yields_to_external_add_uri_workflow() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock RPC listener should bind");
    let rpc_port = listener
        .local_addr()
        .expect("mock RPC address should exist")
        .port();
    let rpc_state = Arc::new(LifecycleRaceRpcState {
        add_uri_started: Notify::new(),
        release_add_uri: Notify::new(),
        save_session_started: Notify::new(),
        release_save_session: Notify::new(),
    });
    let rpc_server = tokio::spawn({
        let rpc_state = Arc::clone(&rpc_state);
        async move {
            axum::serve(
                listener,
                axum::Router::new()
                    .route("/jsonrpc", axum::routing::post(lifecycle_race_rpc))
                    .with_state(rpc_state),
            )
            .await
            .expect("mock RPC server should serve");
        }
    });

    let state = test_state().await;
    let save_dir = state.runtime.app_data_dir.join("race-downloads");
    std::fs::create_dir_all(&save_dir).expect("race save directory should create");
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&json!({
            "paths": [save_dir.display().to_string()]
        }))
        .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    write_json_rpc_token(&state, "secret").await;

    let child = spawn_long_running_child();
    let pid = child.id();
    state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .replace(ManagedAria2Process::new(
            child,
            crate::config::aria2::Aria2BinarySource::ExternalPath,
        ));
    let config =
        crate::aria2::runtime_config(&state.base_aria2_config, rpc_port, "secret".to_string());
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            crate::config::aria2::Aria2BinarySource::ExternalPath,
            Vec::new(),
        ))
        .expect("runtime should persist");
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");

    let auto_stop_state = state.clone();
    let auto_stop = tokio::spawn(async move { auto_stop_aria2(&auto_stop_state).await });
    timeout(
        Duration::from_secs(2),
        rpc_state.save_session_started.notified(),
    )
    .await
    .expect("auto stop should reach saveSession while quiescing");

    let add_uri_state = state.clone();
    let add_uri = tokio::spawn(async move {
        execute_method(
            &add_uri_state,
            "aria2.addUri",
            &json!([
                "token:secret",
                ["https://example.com/race.zip"],
                {
                    "dir": save_dir.display().to_string(),
                    "out": "race.zip"
                }
            ]),
        )
        .await
    });
    timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = state
                .aria2_lifecycle
                .snapshot()
                .expect("lifecycle snapshot should load");
            if snapshot.active_leases > 0 && snapshot.queued_requests > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("external addUri should register activity while auto stop is quiescing");

    rpc_state.release_save_session.notify_one();
    let auto_stop_result = auto_stop.await.expect("auto stop should not panic");
    let auto_stop_error = auto_stop_result.expect_err("new activity should cancel auto stop");
    assert!(auto_stop_error.contains("在途生命周期操作"));
    assert_eq!(
        state
            .aria2_lifecycle
            .snapshot()
            .expect("lifecycle snapshot should load")
            .phase,
        crate::runtime::Aria2LifecyclePhase::Ready
    );

    timeout(Duration::from_secs(2), rpc_state.add_uri_started.notified())
        .await
        .expect("external addUri should reach RPC server");
    rpc_state.release_add_uri.notify_one();
    let add_uri_result = add_uri.await.expect("external addUri should not panic");

    let task_snapshot = state.core.download_tasks.list().expect("tasks should list");
    let manual_stop_with_task = stop_aria2(&state).await;
    let process_still_owned = state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some();
    stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test Aria2 process should stop");
    state.clear_aria2_runtime();
    state.core.database.pool.close().await;
    rpc_server.abort();
    let _ = rpc_server.await;
    let _ = std::fs::remove_dir_all(&state.runtime.app_data_dir);

    assert_eq!(
        add_uri_result.expect("external addUri should succeed"),
        "gid-race"
    );
    assert_eq!(task_snapshot.len(), 1);
    assert_eq!(task_snapshot[0].gid.as_deref(), Some("gid-race"));
    assert!(process_still_owned);
    let task_stop_error =
        manual_stop_with_task.expect_err("manual stop should reject an active task");
    assert!(task_stop_error.to_string().contains("活动或在途操作"));
}

#[tokio::test]
async fn stopped_get_version_keeps_http_websocket_and_multicall_contract_consistent() {
    let requests = Arc::new(AtomicU64::new(0));
    let requests_for_handler = requests.clone();
    let rpc_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock RPC listener should bind");
    let rpc_port = rpc_listener
        .local_addr()
        .expect("mock RPC addr should exist")
        .port();
    let rpc_server = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/jsonrpc",
            axum::routing::post(move || {
                let requests = requests_for_handler.clone();
                async move {
                    requests.fetch_add(1, Ordering::SeqCst);
                    axum::Json(json!({ "result": { "version": "2.4.9" } }))
                }
            }),
        );
        axum::serve(rpc_listener, app)
            .await
            .expect("mock RPC server should serve");
    });

    let mut state = test_state().await;
    std::sync::Arc::get_mut(&mut state)
        .expect("state should be uniquely owned")
        .base_aria2_config
        .rpc_port = rpc_port;

    let http_response = super::super::jsonrpc_router(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": "http-version",
                        "method": "aria2.getVersion",
                        "params": []
                    })
                    .to_string(),
                ))
                .expect("HTTP request should build"),
        )
        .await
        .expect("HTTP response should succeed");
    let http_payload: Value = serde_json::from_slice(
        &to_bytes(http_response.into_body(), usize::MAX)
            .await
            .expect("HTTP body should read"),
    )
    .expect("HTTP payload should parse");
    assert_eq!(http_payload["result"]["version"], "unknown");
    assert_eq!(http_payload["result"]["enabledFeatures"], json!([]));

    let multicall_payload = handle_jsonrpc_payload(
        &state,
        json!({
            "jsonrpc": "2.0",
            "id": "multicall-version",
            "method": "system.multicall",
            "params": [[{
                "methodName": "aria2.getVersion",
                "params": []
            }]]
        }),
    )
    .await;
    assert_eq!(multicall_payload["result"][0][0]["version"], "unknown");
    assert_eq!(
        multicall_payload["result"][0][0]["enabledFeatures"],
        json!([])
    );

    let websocket_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("WebSocket listener should bind");
    let websocket_addr = websocket_listener
        .local_addr()
        .expect("WebSocket addr should exist");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let websocket_state = state.clone();
    let websocket_server = tokio::spawn(async move {
        axum::serve(
            websocket_listener,
            super::super::jsonrpc_router(websocket_state),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("WebSocket server should stop cleanly");
    });

    let mut socket = connect_websocket(websocket_addr).await;
    write_masked_websocket_frame(
        &mut socket,
        true,
        0x1,
        json!({
            "jsonrpc": "2.0",
            "id": "websocket-version",
            "method": "aria2.getVersion",
            "params": []
        })
        .to_string()
        .as_bytes(),
    )
    .await;
    let websocket_payload: Value =
        serde_json::from_str(&read_websocket_text_frame(&mut socket).await)
            .expect("WebSocket payload should parse");
    assert_eq!(websocket_payload["result"]["version"], "unknown");
    assert_eq!(websocket_payload["result"]["enabledFeatures"], json!([]));
    drop(socket);

    shutdown_tx
        .send(())
        .expect("WebSocket server should accept shutdown");
    websocket_server
        .await
        .expect("WebSocket server task should join");
    assert_eq!(requests.load(Ordering::SeqCst), 0);
    rpc_server.abort();
    let _ = rpc_server.await;
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
fn parse_add_uri_detects_magnet_source_type() {
    let command = parse_add_uri_command(&json!([
        "token:anything",
        ["magnet:?xt=urn:btih:test"],
        {
            "dir": "/vol1/1000/tmp"
        }
    ]))
    .expect("magnet addUri params should parse");

    assert_eq!(command.url, "magnet:?xt=urn:btih:test");
    assert_eq!(
        command.source_type,
        crate::tasks::DownloadTaskSourceType::Magnet
    );
}

#[test]
fn parse_add_uri_detects_torrent_source_type() {
    let command = parse_add_uri_command(&serde_json::json!([["torrent:example.torrent"]]))
        .expect("torrent URI should parse");

    assert_eq!(
        command.source_type,
        crate::tasks::DownloadTaskSourceType::Torrent
    );
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
            "max-download-limit": "524288",
            "all-proxy": "socks5://127.0.0.1:7890",
            "min-split-size": "1M",
            "user-agent": "Motrix",
            "header": ["Referer: https://example.com"],
            "unknown-option": "ignored"
        }
    ]))
    .expect("addUri params should parse");

    assert_eq!(command.aria2_options["split"], "256");
    assert_eq!(command.aria2_options["max-connection-per-server"], "256");
    assert_eq!(command.aria2_options["max-download-limit"], "524288");
    assert_eq!(
        command.aria2_options["all-proxy"],
        "socks5://127.0.0.1:7890"
    );
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

#[test]
fn resolve_authorized_save_dir_accepts_exact_and_missing_leading_slash() {
    let accessible_paths = vec!["/vol1/1000/tmp".to_string()];

    assert_eq!(
        resolve_authorized_save_dir("/vol1/1000/tmp", &accessible_paths)
            .expect("exact authorized path should pass"),
        "/vol1/1000/tmp"
    );
    assert_eq!(
        resolve_authorized_save_dir("vol1/1000/tmp", &accessible_paths)
            .expect("one missing leading slash should be restored"),
        "/vol1/1000/tmp"
    );
}

#[test]
fn resolve_authorized_save_dir_rejects_unsafe_or_inexact_relative_paths() {
    let accessible_paths = vec!["/vol1/1000/tmp".to_string()];

    for save_dir in [
        "vol1/1000",
        "vol1/1000/tmp/subdir",
        "vol1//1000/tmp",
        "vol1/./1000/tmp",
        "vol1/../1000/tmp",
        "vol1\\1000\\tmp",
        "/vol1/1000/tmp/",
    ] {
        assert_eq!(
            resolve_authorized_save_dir(save_dir, &accessible_paths),
            Err(crate::storage::TaskSaveDirError::Unauthorized),
            "save dir: {save_dir}"
        );
    }
}

#[test]
fn resolve_authorized_save_dir_requires_one_unique_authorized_match() {
    assert_eq!(
        resolve_authorized_save_dir("vol1/1000/tmp", &[]),
        Err(crate::storage::TaskSaveDirError::NoAccessiblePaths)
    );
    assert_eq!(
        resolve_authorized_save_dir(
            "vol1/1000/tmp",
            &["/vol1/1000/tmp".to_string(), "/vol1/1000/tmp".to_string()]
        ),
        Err(crate::storage::TaskSaveDirError::Unauthorized)
    );
}

#[tokio::test]
async fn authorized_save_dir_replaces_stale_cached_default_after_authorization_change() {
    let state = test_state().await;
    let stale_default = state.runtime.app_data_dir.display().to_string();
    let current_default = "/vol1/1000/tmp";
    state.remember_json_rpc_default_download_dir(&stale_default);
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&json!({ "paths": [current_default] }))
            .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");

    assert_eq!(
        authorized_save_dir(&state, &stale_default)
            .expect("stale advertised default should fall back to current authorization"),
        current_default
    );
    assert_eq!(state.json_rpc_default_download_dir(), current_default);

    let unauthorized = authorized_save_dir(&state, "/vol1/not-authorized")
        .expect_err("an unrelated unauthorized directory must still be rejected");
    assert_eq!(unauthorized.code, -32602);
    assert_eq!(unauthorized.message, "保存目录不在飞牛已授权目录列表中");
}

async fn test_state() -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("jsonrpc-api");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };

    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

struct LifecycleRaceRpcState {
    add_uri_started: Notify,
    release_add_uri: Notify,
    save_session_started: Notify,
    release_save_session: Notify,
}

async fn lifecycle_race_rpc(
    axum::extract::State(state): axum::extract::State<Arc<LifecycleRaceRpcState>>,
    axum::Json(payload): axum::Json<Value>,
) -> axum::Json<Value> {
    match payload.get("method").and_then(Value::as_str) {
        Some("aria2.getVersion") => axum::Json(json!({
            "result": { "version": "2.4.9" }
        })),
        Some("aria2.addUri") => {
            state.add_uri_started.notify_one();
            state.release_add_uri.notified().await;
            axum::Json(json!({ "result": "gid-race" }))
        }
        Some("aria2.saveSession") => {
            state.save_session_started.notify_one();
            state.release_save_session.notified().await;
            axum::Json(json!({ "result": "OK" }))
        }
        _ => axum::Json(json!({ "result": "ok" })),
    }
}

async fn protocol_regression_rpc(axum::Json(payload): axum::Json<Value>) -> axum::Json<Value> {
    match payload.get("method").and_then(Value::as_str) {
        Some("aria2.addUri") => axum::Json(json!({ "result": "gid-protocol" })),
        Some("aria2.getVersion") => axum::Json(json!({
            "result": { "version": "2.4.9" }
        })),
        Some(method) => axum::Json(json!({
            "error": { "message": format!("unexpected method: {method}") }
        })),
        None => axum::Json(json!({ "error": { "message": "missing method" } })),
    }
}

async fn write_json_rpc_token(state: &Arc<HttpAppState>, token: &str) {
    save_json_rpc_token(&state.core.database.pool, token)
        .await
        .expect("JSON-RPC token should save");
    state.remember_json_rpc_token(token);
}

fn padded_jsonrpc_payload(size: usize) -> Vec<u8> {
    let mut payload = json!({
        "jsonrpc": "2.0",
        "id": "version",
        "method": "aria2.getVersion",
        "params": [],
        "padding": ""
    });
    let base_size = serde_json::to_vec(&payload)
        .expect("JSON-RPC payload should serialize")
        .len();
    payload["padding"] = json!("x".repeat(size - base_size));
    let payload = serde_json::to_vec(&payload).expect("JSON-RPC payload should serialize");
    assert_eq!(payload.len(), size);
    payload
}

async fn assert_websocket_frames_rejected(frames: Vec<(bool, u8, Vec<u8>)>) {
    let state = test_state().await;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let addr = listener.local_addr().expect("listener should have address");
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        axum::serve(listener, super::super::jsonrpc_router(state))
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("server should stop cleanly");
    });

    let mut socket = connect_websocket(addr).await;
    for (fin, opcode, payload) in frames {
        write_masked_websocket_frame(&mut socket, fin, opcode, &payload).await;
    }
    assert_websocket_closed(&mut socket).await;
    drop(socket);

    shutdown_tx.send(()).expect("server should accept shutdown");
    server.await.expect("server task should join");
}

async fn connect_websocket(addr: SocketAddr) -> TcpStream {
    let mut socket = TcpStream::connect(addr)
        .await
        .expect("websocket client should connect");
    socket
        .write_all(
            b"GET /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Protocol: jsonrpc\r\n\r\n",
        )
        .await
        .expect("websocket handshake should write");

    let mut response = Vec::new();
    loop {
        let mut buffer = [0_u8; 512];
        let read = timeout(Duration::from_secs(1), socket.read(&mut buffer))
            .await
            .expect("websocket handshake should respond")
            .expect("websocket handshake should read");
        assert!(read > 0, "websocket handshake closed unexpectedly");
        response.extend_from_slice(&buffer[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    assert!(
        response.starts_with(b"HTTP/1.1 101"),
        "unexpected handshake: {}",
        String::from_utf8_lossy(&response)
    );
    socket
}

async fn write_masked_websocket_frame(
    socket: &mut TcpStream,
    fin: bool,
    opcode: u8,
    payload: &[u8],
) {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(if fin { 0x80 | opcode } else { opcode });
    match payload.len() {
        0..=125 => frame.push(0x80 | payload.len() as u8),
        126..=65535 => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        _ => {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
    }
    let mask = [0x12, 0x34, 0x56, 0x78];
    frame.extend_from_slice(&mask);
    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[index % mask.len()]);
    }
    socket
        .write_all(&frame)
        .await
        .expect("websocket frame should write");
}

async fn assert_websocket_closed(socket: &mut TcpStream) {
    let mut response = [0_u8; 2];
    match timeout(Duration::from_secs(1), socket.read(&mut response)).await {
        Ok(Ok(0)) | Ok(Err(_)) => {}
        Ok(Ok(_)) => assert_eq!(response[0] & 0x0f, 0x08, "server should close websocket"),
        Err(_) => panic!("server should reject oversized websocket payload"),
    }
}

async fn read_websocket_text_frame(socket: &mut TcpStream) -> String {
    let mut header = [0_u8; 2];
    timeout(Duration::from_secs(1), socket.read_exact(&mut header))
        .await
        .expect("WebSocket response should arrive")
        .expect("WebSocket response header should read");
    assert_eq!(header[0] & 0x0f, 0x1, "response should be a text frame");
    assert_eq!(header[1] & 0x80, 0, "server response should not be masked");

    let payload_len = match header[1] & 0x7f {
        length @ 0..=125 => length as usize,
        126 => {
            let mut bytes = [0_u8; 2];
            timeout(Duration::from_secs(1), socket.read_exact(&mut bytes))
                .await
                .expect("WebSocket response length should arrive")
                .expect("WebSocket response length should read");
            u16::from_be_bytes(bytes) as usize
        }
        127 => {
            let mut bytes = [0_u8; 8];
            timeout(Duration::from_secs(1), socket.read_exact(&mut bytes))
                .await
                .expect("WebSocket response length should arrive")
                .expect("WebSocket response length should read");
            usize::try_from(u64::from_be_bytes(bytes)).expect("WebSocket payload should fit usize")
        }
        _ => unreachable!(),
    };
    let mut payload = vec![0_u8; payload_len];
    timeout(Duration::from_secs(1), socket.read_exact(&mut payload))
        .await
        .expect("WebSocket response payload should arrive")
        .expect("WebSocket response payload should read");
    String::from_utf8(payload).expect("WebSocket response should be UTF-8")
}

fn temp_dir(label: &str) -> PathBuf {
    let index = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}-{}-{}",
        label,
        std::process::id(),
        index,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}

#[cfg(unix)]
fn spawn_long_running_child() -> Child {
    Command::new("sh")
        .args(["-c", "sleep 10"])
        .spawn()
        .expect("shell should spawn")
}

#[cfg(windows)]
fn spawn_long_running_child() -> Child {
    Command::new("cmd")
        .args(["/C", "ping 127.0.0.1 -n 11 > NUL"])
        .spawn()
        .expect("cmd should spawn")
}
