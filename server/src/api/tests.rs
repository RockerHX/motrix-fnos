use super::*;
use crate::api::app::{AppInfo, AppReadiness, BackendPing};
use crate::api::error::ErrorResponse;
use crate::api::settings::{JsonRpcTokenStatus, LanJsonRpcMutationResponse, LanJsonRpcStatus};
use crate::api::storage::AccessiblePathsResponse;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::aria2::{Aria2ConfigStatus, Aria2RpcStatus};
use crate::auth::SessionKind;
use crate::debug_logs::DebugLogEntry;
use crate::runtime::Aria2ProcessStatus;
use crate::settings::service::AppConfig;
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
use crate::test_support::TestTracingCapture;
use axum::body::to_bytes;
use axum::extract::ConnectInfo;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::routing::get;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tower::ServiceExt;

#[tokio::test]
async fn tcp_router_serves_web_ui_assets_and_api_on_the_desktop_entry_port() {
    let state = test_state(None).await;
    let static_dir = temp_dir("tcp-static");
    std::fs::create_dir_all(static_dir.join("assets")).expect("assets dir should create");
    std::fs::write(static_dir.join("index.html"), b"<html>motrix-ui</html>")
        .expect("index should write");
    std::fs::write(static_dir.join("assets/app.js"), b"console.log('motrix')")
        .expect("asset should write");
    let app = management_router_with_static_dir(state, static_dir);

    for (uri, expected_body) in [
        ("/", "<html>motrix-ui</html>"),
        ("/assets/app.js", "console.log('motrix')"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(body.as_ref(), expected_body.as_bytes());
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/app/ping")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn management_requests_receive_unique_server_request_ids() {
    let state = test_state(None).await;
    let app = management_router(state);

    let request = || {
        Request::builder()
            .method("GET")
            .uri("/api/app/ping")
            .header("x-request-id", "client-supplied-id")
            .body(Body::empty())
            .expect("request should build")
    };
    let (first, second, third) = tokio::join!(
        app.clone().oneshot(request()),
        app.clone().oneshot(request()),
        app.oneshot(request()),
    );
    let responses = [
        first.expect("first response should succeed"),
        second.expect("second response should succeed"),
        third.expect("third response should succeed"),
    ];
    let ids = responses
        .iter()
        .map(|response| {
            response
                .headers()
                .get("x-request-id")
                .expect("request ID should exist")
                .to_str()
                .expect("request ID should be text")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(ids.iter().all(|id| id.starts_with("req-")));
    assert!(ids.iter().all(|id| id != "client-supplied-id"));
    assert_eq!(
        ids.iter().collect::<std::collections::HashSet<_>>().len(),
        3
    );
}

#[tokio::test]
async fn sse_requests_receive_server_request_ids() {
    let state = test_state(None).await;
    let auth_state = state
        .auth
        .service
        .state()
        .await
        .expect("auth state should load");
    let session = state
        .auth
        .sessions
        .create(SessionKind::AnonymousManagement, auth_state.auth_version)
        .expect("anonymous session should create");
    let app = management_router(state);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/events")
                .header("cookie", format!("motrix_web_session={}", session.id))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("req-")));
}

#[tokio::test]
async fn management_router_returns_404_for_unknown_paths_without_cors() {
    let state = test_state(None).await;
    let static_dir = temp_dir("management-router-static");
    std::fs::create_dir_all(&static_dir).expect("static dir should create");
    std::fs::write(static_dir.join("index.html"), b"<html>motrix-ui</html>")
        .expect("index should write");
    let app = management_router_with_static_dir(state, static_dir);

    for uri in ["/missing", "/nested/route", "/api/missing"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "uri: {uri}");
    }

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/app/ping")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get("access-control-allow-origin")
        .is_none());
}

#[tokio::test]
async fn readiness_route_uses_listener_and_shutdown_state_without_database_probe() {
    let state = test_state(None).await;
    let app = management_router(state.clone());

    let unavailable = response_json::<ErrorResponse>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/app/ready")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    assert_eq!(unavailable.code, "app_not_ready");

    state.mark_listeners_ready();
    let readiness = response_json::<AppReadiness>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/app/ready")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(readiness.ready);

    state.core.database.pool.close().await;
    let readiness = response_json::<AppReadiness>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/app/ready")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(readiness.ready);

    state.request_shutdown("测试服务退出");
    let unavailable = response_json::<ErrorResponse>(
        app.oneshot(
            Request::builder()
                .uri("/api/app/ready")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::SERVICE_UNAVAILABLE,
    )
    .await;
    assert_eq!(unavailable.code, "app_not_ready");
}

#[tokio::test]
async fn management_api_enforces_one_mebibyte_body_limit() {
    let state = test_state(None).await;
    let save_dir = state.runtime.app_data_dir.display().to_string();
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&json!({ "paths": [save_dir] }))
            .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    let app = management_router(state.clone());

    for (size, expected_status) in [
        (API_BODY_LIMIT, StatusCode::OK),
        (API_BODY_LIMIT + 1, StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let payload =
            padded_settings_payload(&state.runtime.app_data_dir.display().to_string(), size);
        serde_json::from_slice::<AppConfig>(&payload).expect("settings payload should deserialize");
        let mut request =
            authorized_request(&state, "PUT", "/api/settings", Body::from(payload.clone())).await;
        request.headers_mut().insert(
            CONTENT_LENGTH,
            payload
                .len()
                .to_string()
                .parse()
                .expect("content length should parse"),
        );

        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("response should succeed");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should read");
        assert_eq!(
            status,
            expected_status,
            "body size: {size}, response: {}",
            String::from_utf8_lossy(&body)
        );
    }
}

#[tokio::test]
async fn torrent_upload_keeps_file_and_total_body_limits_separate() {
    let state = test_state(None).await;
    let app = management_router(state.clone());
    let oversized_file = vec![b'x'; 10 * 1024 * 1024 + 1];
    let (content_type, body) = torrent_multipart_body(&oversized_file);
    let response = app
        .clone()
        .oneshot(authorized_multipart_request(&state, content_type, body).await)
        .await
        .expect("response should succeed");
    let error = response_json::<ErrorResponse>(response, StatusCode::BAD_REQUEST).await;
    assert_eq!(error.code, "torrent_too_large");

    let total_body = vec![b'x'; TORRENT_UPLOAD_BODY_LIMIT + 1];
    let response = app
        .clone()
        .oneshot(
            authorized_multipart_request(
                &state,
                "multipart/form-data; boundary=motrix-fnos-test-boundary".to_string(),
                total_body,
            )
            .await,
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);

    let response = app
        .oneshot(
            authorized_multipart_request(
                &state,
                "multipart/form-data".to_string(),
                b"invalid multipart body".to_vec(),
            )
            .await,
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn limited_http_routes_timeout_slow_requests() {
    let app = with_http_resource_limits(
        Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                StatusCode::OK
            }),
        ),
        HttpResourceLimits {
            body_limit: API_BODY_LIMIT,
            concurrency_limit: 1,
            timeout: Duration::from_millis(1),
        },
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/slow")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}

#[tokio::test]
async fn jsonrpc_router_only_serves_the_exact_jsonrpc_path() {
    let state = test_state(None).await;
    let app = jsonrpc_router(state);

    for uri in [
        "/",
        "/api/settings",
        "/api/tasks",
        "/api/events",
        "/index.html",
        "/assets/app.js",
        "/jsonrpc/",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "uri: {uri}");
    }

    let response = app
        .clone()
        .oneshot(json_request(
            "POST",
            "/jsonrpc",
            &json!({
                "jsonrpc": "2.0",
                "id": "version",
                "method": "aria2.getVersion",
                "params": []
            }),
        ))
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/jsonrpc")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("*")
    );
}

#[test]
fn lan_jsonrpc_peer_filter_only_allows_rfc1918_ipv4() {
    for allowed in ["10.0.0.1", "172.16.0.1", "172.31.255.254", "192.168.1.12"] {
        assert!(is_rfc1918_peer(allowed.parse().expect("IP should parse")));
    }
    for denied in [
        "127.0.0.1",
        "169.254.1.1",
        "172.15.255.255",
        "172.32.0.1",
        "192.0.2.1",
        "8.8.8.8",
        "::1",
        "fd00::1",
    ] {
        assert!(!is_rfc1918_peer(denied.parse().expect("IP should parse")));
    }
}

#[tokio::test]
async fn lan_jsonrpc_router_checks_switch_and_true_tcp_peer_before_protocol_handling() {
    let state = test_state(None).await;
    let private_peer: SocketAddr = "192.168.1.12:45678".parse().expect("peer should parse");
    let request = |method: &str, body: Body| {
        let mut request = Request::builder()
            .method(method)
            .uri("/jsonrpc")
            .body(body)
            .expect("request should build");
        request.extensions_mut().insert(ConnectInfo(private_peer));
        request
    };

    let disabled = lan_jsonrpc_router(state.clone())
        .oneshot(request("POST", Body::from("not-json")))
        .await
        .expect("disabled response should succeed");
    assert_eq!(disabled.status(), StatusCode::NOT_FOUND);

    *state.lan_json_rpc_config.write().await = crate::settings::service::LanJsonRpcConfig {
        enabled: true,
        token: "lan-secret".to_string(),
    };
    let mut public_request = json_request(
        "POST",
        "/jsonrpc",
        &json!({
            "jsonrpc": "2.0",
            "id": "spoofed-peer",
            "method": "aria2.getVersion",
            "params": []
        }),
    );
    public_request.extensions_mut().insert(ConnectInfo(
        "203.0.113.10:45678".parse::<SocketAddr>().unwrap(),
    ));
    public_request
        .headers_mut()
        .insert("x-forwarded-for", "192.168.1.12".parse().unwrap());
    let denied = lan_jsonrpc_router(state.clone())
        .oneshot(public_request)
        .await
        .expect("denied response should succeed");
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let allowed = lan_jsonrpc_router(state.clone())
        .oneshot(request(
            "POST",
            Body::from(
                json!({
                    "jsonrpc": "2.0",
                    "id": "lan-version",
                    "method": "aria2.getVersion",
                    "params": []
                })
                .to_string(),
            ),
        ))
        .await
        .expect("allowed response should succeed");
    assert_eq!(allowed.status(), StatusCode::OK);

    let options = lan_jsonrpc_router(state.clone())
        .oneshot(request("OPTIONS", Body::empty()))
        .await
        .expect("OPTIONS response should succeed");
    assert_eq!(options.status(), StatusCode::NO_CONTENT);

    let mut websocket = request("GET", Body::empty());
    let headers = websocket.headers_mut();
    headers.insert("connection", "upgrade".parse().unwrap());
    headers.insert("upgrade", "websocket".parse().unwrap());
    headers.insert("sec-websocket-version", "13".parse().unwrap());
    headers.insert(
        "sec-websocket-key",
        "dGhlIHNhbXBsZSBub25jZQ==".parse().unwrap(),
    );
    headers.insert("sec-websocket-protocol", "jsonrpc".parse().unwrap());
    let on_upgrade = hyper::upgrade::on(&mut websocket);
    websocket.extensions_mut().insert(on_upgrade);
    let upgraded = lan_jsonrpc_router(state)
        .oneshot(websocket)
        .await
        .expect("WebSocket response should succeed");
    assert_eq!(upgraded.status(), StatusCode::SWITCHING_PROTOCOLS);
    assert_eq!(
        upgraded.headers().get("sec-websocket-protocol"),
        Some(&"jsonrpc".parse().unwrap())
    );
}

#[tokio::test]
async fn app_routes_return_expected_payloads() {
    let state = test_state(None).await;
    let app = management_router(state);

    let info = response_json::<AppInfo>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/app/info")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(info.name, "Motrix");
    assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
    assert_eq!(info.backend_status, "ready");
    assert_eq!(info.maintainer, "rockerhx");
    assert_eq!(
        info.repository_url,
        "https://github.com/RockerHX/motrix-fnos"
    );
    assert_eq!(
        info.release_page_url,
        "https://github.com/RockerHX/motrix-fnos/releases"
    );
    assert_eq!(info.target_arch, std::env::consts::ARCH);
    assert_eq!(info.update_mode, "manual_fpk_or_app_center");

    let ping = response_json::<BackendPing>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/app/ping")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(ping.ok);
    assert_eq!(ping.message, "Rust 后端通信正常");
}

#[tokio::test]
async fn aria2_routes_return_status_payloads() {
    let explicit_path = temp_dir("aria2-config").join("aria2-next");
    std::fs::create_dir_all(explicit_path.parent().expect("parent should exist"))
        .expect("dir should create");
    std::fs::write(&explicit_path, b"").expect("binary should exist");

    let state = test_state(Some(explicit_path.display().to_string())).await;
    let app = management_router(state);

    let config = response_json::<Aria2ConfigStatus>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/aria2/config")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(config.configured);
    assert!(config.path_exists);
    assert_eq!(
        config.path.as_deref(),
        Some(explicit_path.to_string_lossy().as_ref())
    );

    let process = response_json::<Aria2ProcessStatus>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/aria2/process")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(!process.running);
    assert_eq!(process.message, "Aria2 进程未启动");

    let rpc = response_json::<Aria2RpcStatus>(
        app.oneshot(
            Request::builder()
                .uri("/api/aria2/rpc")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(!rpc.connected);
    assert!(rpc.version.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn readonly_routes_do_not_write_file_logs_but_mutations_and_errors_still_do() {
    let state = test_state(None).await;
    let app = management_router(state.clone());
    let capture = TestTracingCapture::default();
    let tracing_guard = tracing::subscriber::set_default(capture.subscriber());

    for _ in 0..2 {
        for uri in [
            "/api/app/info",
            "/api/app/ping",
            "/api/settings",
            "/api/settings/jsonrpc-token",
            "/api/aria2/config",
            "/api/aria2/process",
            "/api/aria2/rpc",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request should build"),
                )
                .await
                .expect("response should succeed");
            assert_eq!(response.status(), StatusCode::OK, "uri: {uri}");
        }
    }
    assert_eq!(capture.contents(), "");

    capture.clear();
    let response = app
        .clone()
        .oneshot(
            authorized_json_request(
                &state,
                "PUT",
                "/api/settings/jsonrpc-token",
                &json!({ "token": "logging-test-token" }),
            )
            .await,
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(capture.contents().contains("JSON-RPC Token 已更新"));

    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&AccessiblePathsResponse {
            paths: vec![state.runtime.app_data_dir.display().to_string()],
        })
        .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    let error_request = authorized_json_request(
        &state,
        "POST",
        "/api/tasks",
        &json!({
            "url": "https://example.com/file.iso",
            "saveDir": "/tmp/not-authorized"
        }),
    )
    .await;
    capture.clear();
    let response = app
        .oneshot(error_request)
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let logs = capture.contents();
    assert!(logs.contains("WARN"));
    assert!(logs.contains("未授权"));

    drop(tracing_guard);
}

#[tokio::test]
async fn aria2_rpc_status_does_not_probe_stopped_or_unconfirmed_runtime() {
    let requests = std::sync::Arc::new(AtomicU64::new(0));
    let requests_for_handler = requests.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let port = listener
        .local_addr()
        .expect("mock addr should exist")
        .port();
    let handle = tokio::spawn(async move {
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
        axum::serve(listener, app)
            .await
            .expect("mock server should serve");
    });

    let mut state = test_state(None).await;
    std::sync::Arc::get_mut(&mut state)
        .expect("state should be uniquely owned")
        .base_aria2_config
        .rpc_port = port;
    let app = management_router(state.clone());

    let stopped = response_json::<Aria2RpcStatus>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/aria2/rpc")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(!stopped.connected);
    assert_eq!(stopped.message, "Aria2 未运行");
    assert_eq!(requests.load(Ordering::SeqCst), 0);

    let config = crate::aria2::runtime_config(&state.base_aria2_config, port, "secret".to_string());
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            999_999,
            &config,
            crate::config::aria2::Aria2BinarySource::Sidecar,
            Vec::new(),
        ))
        .expect("runtime should persist");

    let unconfirmed = response_json::<Aria2RpcStatus>(
        app.oneshot(
            Request::builder()
                .uri("/api/aria2/rpc")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(!unconfirmed.connected);
    assert_eq!(unconfirmed.message, "Aria2 运行态待确认");
    assert_eq!(requests.load(Ordering::SeqCst), 0);
    assert!(state.aria2_runtime_snapshot().is_some());

    handle.abort();
}

#[tokio::test]
async fn aria2_stop_returns_busy_conflict_for_active_task() {
    let state = test_state(None).await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(active_task_for_stop()))
        .expect("tasks should be writable");
    let app = management_router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(authorized_request(&state, "POST", "/api/aria2/stop", Body::empty()).await)
            .await
            .expect("response should succeed"),
        StatusCode::CONFLICT,
    )
    .await;

    assert_eq!(error.code, "aria2_busy");
    assert!(error.message.contains("活动或在途操作"));
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
}

#[tokio::test]
async fn aria2_stop_allows_missing_metadata_record_without_engine_activity() {
    let state = test_state(None).await;
    let mut task = active_task_for_stop();
    task.source_type = DownloadTaskSourceType::Torrent;
    task.gid = None;
    task.status = DownloadTaskStatus::Error;
    task.metadata_torrent_path = None;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("tasks should be writable");
    let app = management_router(state.clone());

    let status = response_json::<Aria2ProcessStatus>(
        app.oneshot(authorized_request(&state, "POST", "/api/aria2/stop", Body::empty()).await)
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert!(!status.running);
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
}

#[tokio::test]
async fn aria2_mutation_routes_reject_when_runtime_is_exiting() {
    let state = test_state(None).await;
    state.core.shutdown.mark_exiting();
    let app = management_router(state.clone());

    for uri in ["/api/aria2/start", "/api/aria2/stop"] {
        let error = response_json::<ErrorResponse>(
            app.clone()
                .oneshot(authorized_request(&state, "POST", uri, Body::empty()).await)
                .await
                .expect("response should succeed"),
            StatusCode::CONFLICT,
        )
        .await;
        assert_eq!(error.code, "runtime_exiting");
        assert_eq!(error.message, "服务正在退出，不能执行当前操作");
    }
}

#[tokio::test]
async fn settings_routes_round_trip_payloads_and_log_rpc_warning() {
    let state = test_state(None).await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&AccessiblePathsResponse {
            paths: vec![
                state.runtime.app_data_dir.display().to_string(),
                "/tmp/custom".to_string(),
            ],
        })
        .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    let app = management_router(state.clone());

    let default_settings = response_json::<AppConfig>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        default_settings.default_download_dir,
        state.runtime.app_data_dir.display().to_string()
    );

    let token_status = response_json::<JsonRpcTokenStatus>(
        app.clone()
            .oneshot(
                authorized_json_request(
                    &state,
                    "PUT",
                    "/api/settings/jsonrpc-token",
                    &json!({ "token": "test-token-a1b2" }),
                )
                .await,
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(token_status.configured);
    assert_eq!(token_status.masked_token.as_deref(), Some("••••••••a1b2"));
    assert_eq!(state.json_rpc_token(), "test-token-a1b2");

    let initial_lan_status = response_json::<LanJsonRpcStatus>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/settings/lan-jsonrpc")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(
        initial_lan_status,
        LanJsonRpcStatus {
            enabled: false,
            configured: false,
            masked_token: None,
            port: 17082,
        }
    );
    let enabled_lan = response_json::<LanJsonRpcMutationResponse>(
        app.clone()
            .oneshot(
                authorized_json_request(
                    &state,
                    "PUT",
                    "/api/settings/lan-jsonrpc",
                    &json!({ "enabled": true }),
                )
                .await,
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    let first_lan_token = enabled_lan
        .issued_token
        .expect("first enable should return a one-time token");
    assert!(enabled_lan.status.enabled);
    assert!(enabled_lan.status.configured);
    assert_eq!(state.lan_json_rpc_config().await.token, first_lan_token);
    let rotated_lan = response_json::<LanJsonRpcMutationResponse>(
        app.clone()
            .oneshot(
                authorized_json_request(
                    &state,
                    "POST",
                    "/api/settings/lan-jsonrpc/token",
                    &json!({}),
                )
                .await,
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_ne!(
        rotated_lan.issued_token.as_deref(),
        Some(first_lan_token.as_str())
    );

    let updated_settings = response_json::<AppConfig>(
        app.clone()
            .oneshot(
                authorized_json_request(
                    &state,
                    "PUT",
                    "/api/settings",
                    &json!({
                        "defaultDownloadDir": "/tmp/custom",
                        "maxConcurrentDownloads": 0,
                        "downloadLimit": 1024,
                        "uploadLimit": 2048,
                        "language": "en-US",
                        "jsonRpcToken": "must-not-overwrite"
                    }),
                )
                .await,
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(updated_settings.default_download_dir, "/tmp/custom");
    assert_eq!(updated_settings.max_concurrent_downloads, 1);
    assert_eq!(updated_settings.download_limit, 1024);
    assert_eq!(updated_settings.upload_limit, 2048);
    assert_eq!(updated_settings.language, "en-US");
    assert_eq!(state.json_rpc_default_download_dir(), "/tmp/custom");

    let stored_settings = response_json::<AppConfig>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/settings")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(stored_settings, updated_settings);
    let stored_token = response_json::<JsonRpcTokenStatus>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/settings/jsonrpc-token")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(stored_token.masked_token.as_deref(), Some("••••••••a1b2"));
    assert!(state.core.debug_logs.list().iter().any(|entry| {
        entry.module == "settings" && entry.message.contains("下载配置将在下次启动后生效")
    }));
}

#[tokio::test]
async fn settings_route_rejects_unauthorized_default_download_dir() {
    let state = test_state(None).await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&AccessiblePathsResponse {
            paths: vec![state.runtime.app_data_dir.display().to_string()],
        })
        .expect("accessible paths should serialize"),
    )
    .expect("accessible paths should write");
    let app = management_router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(
            authorized_json_request(
                &state,
                "PUT",
                "/api/settings",
                &AppConfig {
                    default_download_dir: "/tmp/custom".to_string(),
                    max_concurrent_downloads: 5,
                    download_limit: 0,
                    upload_limit: 0,
                    language: "zh-CN".to_string(),
                },
            )
            .await,
        )
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "settings_save_failed");
    assert_eq!(error.message, "默认下载目录不在已授权目录列表中");
}

#[tokio::test]
async fn ui_preferences_routes_are_not_exposed() {
    let state = test_state(None).await;
    let app = management_router(state);

    for request in [
        Request::builder()
            .uri("/api/ui-preferences")
            .body(Body::empty())
            .expect("request should build"),
        Request::builder()
            .method("PUT")
            .uri("/api/ui-preferences")
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("request should build"),
    ] {
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn storage_route_returns_accessible_paths_from_runtime_file() {
    let state = test_state(None).await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        r#"{"paths":["/vol1/downloads"," /vol1/media ","","/vol1/downloads"]}"#,
    )
    .expect("accessible paths file should write");
    let app = management_router(state);

    let response = response_json::<AccessiblePathsResponse>(
        app.oneshot(
            Request::builder()
                .uri("/api/storage/accessible-paths")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(
        response.paths,
        vec!["/vol1/downloads".to_string(), "/vol1/media".to_string()]
    );
}

#[tokio::test]
async fn task_route_logs_unauthorized_save_dir_failure() {
    let state = test_state(None).await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        r#"{"paths":["/vol1/downloads"]}"#,
    )
    .expect("accessible paths file should write");
    let app = management_router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(
            authorized_json_request(
                &state,
                "POST",
                "/api/tasks",
                &serde_json::json!({
                    "url": "https://example.com/file.iso",
                    "saveDir": "/vol1/not-authorized"
                }),
            )
            .await,
        )
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "save_dir_not_authorized");
    assert!(state.core.debug_logs.list().iter().any(|entry| {
        entry.module == "storage.auth" && entry.message.contains("未授权目录")
    }));
}

#[tokio::test]
async fn debug_log_routes_list_and_clear_entries() {
    let state = test_state(None).await;
    state.core.debug_logs.info("test", "first");
    state.core.debug_logs.warn("test", "second");
    let app = management_router(state.clone());

    let logs = response_json::<Vec<DebugLogEntry>>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/debug-logs")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(logs.iter().any(|entry| entry.message == "first"));
    assert!(logs.iter().any(|entry| entry.message == "second"));

    let response = app
        .oneshot(authorized_request(&state, "DELETE", "/api/debug-logs", Body::empty()).await)
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(state.core.debug_logs.list().is_empty());
}

#[tokio::test]
async fn invalid_json_payload_uses_unified_error_response() {
    let state = test_state(None).await;
    let app = management_router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(authorized_request(&state, "PUT", "/api/settings", Body::from("{")).await)
            .await
            .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(error.code, "invalid_json");
    assert!(error.message.contains("请求体 JSON 无效"));
}

#[tokio::test]
async fn management_router_requires_setup_session_csrf_and_event_context() {
    let state = raw_test_state(None).await;
    let app = management_router(state.clone());
    for uri in [
        "/api/app/ping",
        "/api/settings",
        "/api/tasks",
        "/api/events",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("response should succeed");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "uri: {uri}");
    }

    let configured = state
        .auth
        .service
        .setup("test management password")
        .await
        .expect("auth should initialize");
    let admin = state
        .auth
        .sessions
        .create(crate::auth::SessionKind::Admin, configured.auth_version)
        .expect("admin session should create");
    let admin_cookie = format!("motrix_web_session={}", admin.id);
    let authenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/app/ping")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(authenticated.status(), StatusCode::OK);

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/aria2/stop")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let disabled = state
        .auth
        .service
        .set_protection(false, "test management password")
        .await
        .expect("protection should disable");
    let anonymous_read = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/app/ping")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(anonymous_read.status(), StatusCode::OK);
    let anonymous_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/debug-logs")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(anonymous_write.status(), StatusCode::UNAUTHORIZED);
    let anonymous_event = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(anonymous_event.status(), StatusCode::UNAUTHORIZED);

    let anonymous = state
        .auth
        .sessions
        .create(
            crate::auth::SessionKind::AnonymousManagement,
            disabled.auth_version,
        )
        .expect("anonymous session should create");
    let protected_write = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/debug-logs")
                .header("cookie", format!("motrix_web_session={}", anonymous.id))
                .header("x-csrf-token", anonymous.csrf_token)
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(protected_write.status(), StatusCode::NO_CONTENT);
}

async fn test_state(aria2_path: Option<String>) -> Arc<HttpAppState> {
    let state = raw_test_state(aria2_path).await;
    state
        .auth
        .service
        .setup("test management password")
        .await
        .expect("test auth should initialize");
    state
        .auth
        .service
        .set_protection(false, "test management password")
        .await
        .expect("test auth protection should disable");
    state
}

async fn raw_test_state(aria2_path: Option<String>) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("api-state");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: aria2_path.map(PathBuf::from),
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };

    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

async fn response_json<T: DeserializeOwned>(
    response: axum::response::Response,
    expected_status: StatusCode,
) -> T {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    assert_eq!(
        status,
        expected_status,
        "response body: {}",
        String::from_utf8_lossy(&body)
    );
    serde_json::from_slice(&body).expect("response json should deserialize")
}

fn json_request<T: serde::Serialize>(method: &str, uri: &str, payload: &T) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(payload).expect("payload should serialize"),
        ))
        .expect("request should build")
}

async fn authorized_json_request<T: serde::Serialize>(
    state: &HttpAppState,
    method: &str,
    uri: &str,
    payload: &T,
) -> Request<Body> {
    authorized_request(
        state,
        method,
        uri,
        Body::from(serde_json::to_vec(payload).expect("payload should serialize")),
    )
    .await
}

async fn authorized_request(
    state: &HttpAppState,
    method: &str,
    uri: &str,
    body: Body,
) -> Request<Body> {
    let auth_state = state
        .auth
        .service
        .state()
        .await
        .expect("auth state should load");
    let session = state
        .auth
        .sessions
        .create(
            crate::auth::SessionKind::AnonymousManagement,
            auth_state.auth_version,
        )
        .expect("test session should create");
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("cookie", format!("motrix_web_session={}", session.id))
        .header("x-csrf-token", session.csrf_token)
        .body(body)
        .expect("request should build")
}

async fn authorized_multipart_request(
    state: &HttpAppState,
    content_type: String,
    body: Vec<u8>,
) -> Request<Body> {
    let body_length = body.len();
    let mut request =
        authorized_request(state, "POST", "/api/tasks/torrent", Body::from(body)).await;
    request.headers_mut().insert(
        CONTENT_TYPE,
        content_type.parse().expect("content type should parse"),
    );
    request.headers_mut().insert(
        CONTENT_LENGTH,
        body_length
            .to_string()
            .parse()
            .expect("content length should parse"),
    );
    request
}

fn padded_settings_payload(download_dir: &str, size: usize) -> Vec<u8> {
    let mut payload = json!({
        "defaultDownloadDir": download_dir,
        "maxConcurrentDownloads": 1,
        "downloadLimit": 0,
        "uploadLimit": 0,
        "language": "zh-CN",
        "padding": ""
    });
    let base_size = serde_json::to_vec(&payload)
        .expect("settings payload should serialize")
        .len();
    payload["padding"] = json!("x".repeat(size - base_size));
    let payload = serde_json::to_vec(&payload).expect("settings payload should serialize");
    assert_eq!(payload.len(), size);
    payload
}

fn torrent_multipart_body(data: &[u8]) -> (String, Vec<u8>) {
    let boundary = "motrix-fnos-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"torrent\"; filename=\"example.torrent\"\r\nContent-Type: application/x-bittorrent\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(
        format!(
            "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{{\"saveDir\":\"/tmp\"}}\r\n--{boundary}--\r\n"
        )
        .as_bytes(),
    );
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}-{}-{}",
        label,
        std::process::id(),
        counter,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn active_task_for_stop() -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/active.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "active.zip".to_string(),
        save_dir: "/tmp/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-active".to_string()),
        status: DownloadTaskStatus::Active,
        total_length: 1,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some("/tmp/downloads/active.zip".to_string()),
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}
