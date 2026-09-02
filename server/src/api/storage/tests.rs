use super::*;
use crate::api::error::ErrorResponse;
use crate::api::management_router;
use crate::app::tests::replace_fnos_api_client;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::fnos::{FnosApiClient, API_TOKEN_ENV};
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tower::ServiceExt;

static TEST_DIR_ID: AtomicU64 = AtomicU64::new(1);
static TEST_API_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct TestApiTokenGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for TestApiTokenGuard {
    fn drop(&mut self) {
        std::env::remove_var(API_TOKEN_ENV);
    }
}

fn test_api_token() -> TestApiTokenGuard {
    let lock = TEST_API_ENV_LOCK.get_or_init(|| Mutex::new(()));
    let guard = lock.lock().expect("test API environment lock should work");
    std::env::set_var(API_TOKEN_ENV, "test-token");
    TestApiTokenGuard { _lock: guard }
}

#[tokio::test]
async fn refresh_route_requires_management_bearer_token() {
    let state = test_state("auth").await;
    let app = management_router(state.clone());

    let unauthenticated = app
        .clone()
        .oneshot(refresh_request(None))
        .await
        .expect("request should complete");
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let auth_state = state
        .auth
        .service
        .setup("test management password")
        .await
        .expect("auth should initialize");
    let token = state
        .auth
        .service
        .issue_admin_token(&auth_state)
        .await
        .expect("token should issue");

    let unavailable = app
        .oneshot(refresh_request(Some(&token)))
        .await
        .expect("request should complete");
    let error = response_json::<ErrorResponse>(unavailable, StatusCode::SERVICE_UNAVAILABLE).await;
    assert_eq!(error.code, "fnos_api_token_missing");
}

#[tokio::test]
async fn get_route_keeps_the_existing_paths_only_response() {
    let state = test_state("get").await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        r#"{"paths":["/vol1/downloads"]}"#,
    )
    .expect("snapshot should write");
    let app = routes().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/accessible-paths")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");
    let paths = response_json::<AccessiblePathsResponse>(response, StatusCode::OK).await;
    assert_eq!(paths.paths, vec!["/vol1/downloads"]);
}

#[tokio::test]
async fn display_route_requires_management_session() {
    let state = test_state("display-auth").await;
    let app = management_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/storage/accessible-paths/display?language=zh-CN")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn display_route_validates_language_before_calling_fnos() {
    let state = test_state("display-language").await;
    let app = routes().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/accessible-paths/display?language=fr-FR")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");
    let error = response_json::<ErrorResponse>(response, StatusCode::BAD_REQUEST).await;

    assert_eq!(error.code, "display_language_invalid");
}

#[tokio::test]
async fn display_route_converts_only_paths_from_the_current_snapshot() {
    let _token = test_api_token();
    let state = test_state("display-success").await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        r#"{"paths":["/vol1/downloads","/vol2/media"]}"#,
    )
    .expect("snapshot should write");
    let socket = socket_path("display-success");
    let server = serve_gateway_response(
        &socket,
        "200 OK",
        r#"{"code":0,"msg":"","data":{"status":0,"result":[{"path":"/vol2/media","semanticPath":"Storage 2/media"},{"path":"/vol1/downloads","semanticPath":"Storage 1/downloads"}]}}"#,
    )
    .await;
    replace_fnos_api_client(
        &state,
        FnosApiClient::with_limits(socket.clone(), Duration::from_secs(1), 4096),
    );
    let app = routes().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/accessible-paths/display?language=en-US")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");
    let paths = response_json::<DisplayAccessiblePathsResponse>(response, StatusCode::OK).await;
    let request = String::from_utf8(server.await.expect("gateway should finish"))
        .expect("request should be utf8");
    let _ = std::fs::remove_file(socket);

    assert_eq!(paths.paths[0].path, "/vol1/downloads");
    assert_eq!(paths.paths[0].display_path, "Storage 1/downloads");
    assert_eq!(paths.paths[1].path, "/vol2/media");
    assert_eq!(paths.paths[1].display_path, "Storage 2/media");
    assert!(request.contains(r#""path":["/vol1/downloads","/vol2/media"]"#));
    assert!(request.contains(r#""language":"en-US""#));
}

#[tokio::test]
async fn display_route_falls_back_to_real_paths_when_fnos_rejects_the_request() {
    let _token = test_api_token();
    let state = test_state("display-fallback").await;
    std::fs::write(
        &state.runtime.accessible_paths_path,
        r#"{"paths":["/vol1/downloads"]}"#,
    )
    .expect("snapshot should write");
    let socket = socket_path("display-fallback");
    let server = serve_gateway_response(
        &socket,
        "401 Unauthorized",
        r#"{"code":1000001,"msg":"denied","data":null}"#,
    )
    .await;
    replace_fnos_api_client(
        &state,
        FnosApiClient::with_limits(socket.clone(), Duration::from_secs(1), 4096),
    );
    let app = routes().with_state(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/storage/accessible-paths/display")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");
    let paths = response_json::<DisplayAccessiblePathsResponse>(response, StatusCode::OK).await;
    server.await.expect("gateway should finish");
    let _ = std::fs::remove_file(socket);

    assert_eq!(paths.paths[0].path, "/vol1/downloads");
    assert_eq!(paths.paths[0].display_path, "/vol1/downloads");
}

#[tokio::test]
async fn refresh_route_persists_official_paths_and_updates_jsonrpc_default() {
    let _token = test_api_token();
    let state = test_state("success").await;
    let socket = socket_path("success");
    let server = serve_gateway_responses(&socket, 1, None).await;
    replace_fnos_api_client(
        &state,
        FnosApiClient::with_limits(socket.clone(), Duration::from_secs(1), 4096),
    );
    let app = routes().with_state(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/storage/accessible-paths/refresh")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("request should complete");
    let paths = response_json::<AccessiblePathsResponse>(response, StatusCode::OK).await;
    server.await.expect("gateway should finish");

    assert_eq!(paths.paths, vec!["/vol1/downloads"]);
    assert_eq!(
        crate::storage::load_accessible_paths(&state.runtime.accessible_paths_path)
            .expect("snapshot should load"),
        paths.paths
    );
    assert_eq!(state.json_rpc_default_download_dir(), "/vol1/downloads");
    let _ = std::fs::remove_file(socket);
}

#[tokio::test]
async fn concurrent_refreshes_are_serialized_before_calling_the_gateway() {
    let _token = test_api_token();
    let state = test_state("concurrent").await;
    let socket = socket_path("concurrent");
    let (first_request_tx, first_request_rx) = oneshot::channel();
    let server = serve_gateway_responses(&socket, 2, Some(first_request_tx)).await;
    replace_fnos_api_client(
        &state,
        FnosApiClient::with_limits(socket.clone(), Duration::from_secs(2), 4096),
    );

    let first_state = state.clone();
    let first = tokio::spawn(async move { first_state.refresh_accessible_paths_from_fnos().await });
    first_request_rx
        .await
        .expect("first gateway request should arrive");
    let second_state = state.clone();
    let second =
        tokio::spawn(async move { second_state.refresh_accessible_paths_from_fnos().await });

    assert_eq!(
        first.await.expect("first refresh should join"),
        Ok(vec!["/vol1/downloads".to_string()])
    );
    assert_eq!(
        second.await.expect("second refresh should join"),
        Ok(vec!["/vol1/downloads".to_string()])
    );
    let second_arrived_early = server.await.expect("gateway should finish");
    assert!(
        !second_arrived_early,
        "refresh lock must serialize gateway calls"
    );
    let _ = std::fs::remove_file(socket);
}

#[tokio::test]
async fn refresh_errors_never_map_to_browser_unauthorized() {
    let cases = [
        (
            AccessiblePathsRefreshError::Fnos(FnosApiError::Rejected {
                http_status: Some(401),
                business_code: Some(1_000_001),
            }),
            StatusCode::BAD_GATEWAY,
            "fnos_api_rejected",
        ),
        (
            AccessiblePathsRefreshError::Fnos(FnosApiError::Timeout),
            StatusCode::SERVICE_UNAVAILABLE,
            "fnos_api_timeout",
        ),
        (
            AccessiblePathsRefreshError::InvalidPaths,
            StatusCode::BAD_GATEWAY,
            "fnos_api_invalid_response",
        ),
        (
            AccessiblePathsRefreshError::Persist,
            StatusCode::INTERNAL_SERVER_ERROR,
            "accessible_paths_persist_failed",
        ),
    ];

    for (error, expected_status, expected_code) in cases {
        let response = classify_refresh_error(error).into_response();
        assert_eq!(response.status(), expected_status);
        let body = response_json::<ErrorResponse>(response, expected_status).await;
        assert_eq!(body.code, expected_code);
    }
}

fn refresh_request(token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/storage/accessible-paths/refresh");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("request should build")
}

async fn response_json<T: DeserializeOwned>(
    response: axum::response::Response,
    expected_status: StatusCode,
) -> T {
    assert_eq!(response.status(), expected_status);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should read");
    serde_json::from_slice(&body).expect("body should be json")
}

async fn test_state(label: &str) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir(label);
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("address should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("address should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };
    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-storage-api-{label}-{}-{}",
        std::process::id(),
        TEST_DIR_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory should exist");
    path
}

fn socket_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-storage-api-{label}-{}-{}.sock",
        std::process::id(),
        TEST_DIR_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn serve_gateway_responses(
    socket: &PathBuf,
    request_count: usize,
    first_request_tx: Option<oneshot::Sender<()>>,
) -> tokio::task::JoinHandle<bool> {
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).expect("gateway socket should bind");
    tokio::spawn(async move {
        let mut first_request_tx = first_request_tx;
        let response_body = r#"{"code":0,"msg":"","data":{"paths":["/vol1/downloads"]}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        let mut pending_second = None;
        let mut second_arrived_early = false;
        for index in 0..request_count {
            let mut stream = if index == 1 {
                pending_second.take().unwrap_or_else(|| {
                    panic!("second gateway stream should be accepted asynchronously")
                })
            } else {
                let (stream, _) = listener.accept().await.expect("request should connect");
                stream
            };
            read_http_request(&mut stream).await;
            if index == 0 {
                if let Some(sender) = first_request_tx.take() {
                    let _ = sender.send(());
                    match tokio::time::timeout(Duration::from_millis(50), listener.accept()).await {
                        Ok(Ok((stream, _))) => {
                            second_arrived_early = true;
                            pending_second = Some(stream);
                        }
                        Ok(Err(error)) => panic!("accepting second request failed: {error}"),
                        Err(_) => {}
                    }
                }
            }
            stream
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
            if index == 0 && request_count > 1 && pending_second.is_none() {
                let (stream, _) = listener
                    .accept()
                    .await
                    .expect("second request should connect");
                pending_second = Some(stream);
            }
        }
        second_arrived_early
    })
}

async fn serve_gateway_response(
    socket: &PathBuf,
    status: &str,
    response_body: &str,
) -> tokio::task::JoinHandle<Vec<u8>> {
    let _ = std::fs::remove_file(socket);
    let listener = UnixListener::bind(socket).expect("gateway socket should bind");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
        response_body.len()
    );
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request should connect");
        let request = read_http_request_bytes(&mut stream).await;
        stream
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        request
    })
}

async fn read_http_request(stream: &mut UnixStream) {
    let _ = read_http_request_bytes(stream).await;
}

async fn read_http_request_bytes(stream: &mut UnixStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).await.expect("request should read");
        assert_ne!(read, 0, "request ended before body completed");
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .expect("content length should exist");
        if request.len() >= header_end + 4 + content_length {
            return request;
        }
    }
}
