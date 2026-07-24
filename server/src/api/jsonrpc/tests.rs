use super::add_uri::parse_add_uri_command;
use super::auth::validate_add_uri_token;
use super::methods::{execute_method, handle_jsonrpc_payload};
use crate::app::HttpAppState;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::settings::service::save_json_rpc_token;
use axum::body::Body;
use axum::http::header::CONTENT_LENGTH;
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
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

async fn test_state() -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("jsonrpc-api");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };

    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

async fn write_json_rpc_token(state: &Arc<HttpAppState>, token: &str) {
    save_json_rpc_token(&state.core.database.pool, token)
        .await
        .expect("JSON-RPC token should save");
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
