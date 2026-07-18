use super::add_uri::parse_add_uri_command;
use super::auth::validate_add_uri_token;
use super::methods::{execute_method, handle_jsonrpc_payload};
use crate::app::HttpAppState;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::settings::service::save_json_rpc_token;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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
