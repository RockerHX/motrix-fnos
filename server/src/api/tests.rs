use super::*;
use crate::api::app::{AppInfo, BackendPing};
use crate::api::error::ErrorResponse;
use crate::api::storage::AccessiblePathsResponse;
use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
use crate::aria2::{Aria2ConfigStatus, Aria2RpcStatus};
use crate::debug_logs::DebugLogEntry;
use crate::runtime::Aria2ProcessStatus;
use crate::settings::service::AppConfig;
use axum::body::to_bytes;
use axum::http::StatusCode;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_requires_authenticated_admin_and_tcp_router_exposes_only_jsonrpc() {
    let state = test_state(None).await;
    let gateway = gateway_router(state.clone());

    let unauthorized = gateway
        .clone()
        .oneshot(
            Request::builder()
                .uri("/app/motrix/api/app/ping")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let forbidden = gateway
        .clone()
        .oneshot(
            Request::builder()
                .uri("/app/motrix/api/app/ping")
                .header("x-trim-userid", "1000")
                .header("x-trim-isadmin", "false")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    let allowed = gateway
        .oneshot(
            Request::builder()
                .uri("/app/motrix/api/app/ping")
                .header("x-trim-userid", "1000")
                .header("x-trim-isadmin", "true")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(allowed.status(), StatusCode::OK);

    let public_api = jsonrpc_router(state)
        .oneshot(
            Request::builder()
                .uri("/api/app/ping")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(public_api.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn app_routes_return_expected_payloads() {
    let state = test_state(None).await;
    let app = router(state);

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
        app.oneshot(
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
    let app = router(state);

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

#[tokio::test]
async fn aria2_mutation_routes_reject_when_runtime_is_exiting() {
    let state = test_state(None).await;
    state.core.shutdown.mark_exiting();
    let app = router(state);

    for uri in ["/api/aria2/start", "/api/aria2/stop"] {
        let error = response_json::<ErrorResponse>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .body(Body::empty())
                        .expect("request should build"),
                )
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
    let app = router(state.clone());

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
    assert_eq!(default_settings.json_rpc_token, "");

    let updated_settings = response_json::<AppConfig>(
        app.clone()
            .oneshot(json_request(
                "PUT",
                "/api/settings",
                &AppConfig {
                    default_download_dir: "/tmp/custom".to_string(),
                    max_concurrent_downloads: 0,
                    download_limit: 1024,
                    upload_limit: 2048,
                    language: "en-US".to_string(),
                    json_rpc_token: "test-token".to_string(),
                },
            ))
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
    assert_eq!(updated_settings.json_rpc_token, "test-token");

    let stored_settings = response_json::<AppConfig>(
        app.oneshot(
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
    let app = router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "PUT",
            "/api/settings",
            &AppConfig {
                default_download_dir: "/tmp/custom".to_string(),
                max_concurrent_downloads: 5,
                download_limit: 0,
                upload_limit: 0,
                language: "zh-CN".to_string(),
                json_rpc_token: String::new(),
            },
        ))
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
    let app = router(state);

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
    let app = router(state);

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
    let app = router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &serde_json::json!({
                "url": "https://example.com/file.iso",
                "saveDir": "/vol1/not-authorized"
            }),
        ))
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
    let app = router(state.clone());

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
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/debug-logs")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(state.core.debug_logs.list().is_empty());
}

#[tokio::test]
async fn invalid_json_payload_uses_unified_error_response() {
    let state = test_state(None).await;
    let app = router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/settings")
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(error.code, "invalid_json");
    assert!(error.message.contains("请求体 JSON 无效"));
}

async fn test_state(aria2_path: Option<String>) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("api-state");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        gateway_socket_path: None,
        aria2_path: aria2_path.map(PathBuf::from),
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
