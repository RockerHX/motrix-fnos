use super::*;

use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use std::sync::atomic::{AtomicU64, Ordering};

static LAN_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn token_status_masks_long_and_short_tokens_without_returning_raw_values() {
    assert_eq!(
        json_rpc_token_status(""),
        JsonRpcTokenStatus {
            configured: false,
            masked_token: None,
        }
    );
    assert_eq!(
        json_rpc_token_status("short"),
        JsonRpcTokenStatus {
            configured: true,
            masked_token: Some("••••••••".to_string()),
        }
    );
    assert_eq!(
        json_rpc_token_status("long-token-a1b2"),
        JsonRpcTokenStatus {
            configured: true,
            masked_token: Some("••••••••a1b2".to_string()),
        }
    );
}

#[test]
fn public_app_config_never_serializes_legacy_token_field() {
    let config = AppConfig {
        default_download_dir: "/downloads".to_string(),
        max_concurrent_downloads: 5,
        download_limit: 0,
        upload_limit: 0,
        language: "zh-CN".to_string(),
    };
    let value = serde_json::to_value(config).expect("config should serialize");
    assert!(value.get("jsonRpcToken").is_none());
}

#[tokio::test]
async fn lan_json_rpc_first_enable_preserves_token_and_rotation_invalidates_old_value() {
    let (state, runtime) = lan_test_state("lifecycle").await;

    let enabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest { enabled: true }),
    )
    .await
    .expect("first enable should succeed")
    .0;
    let first_token = enabled
        .issued_token
        .clone()
        .expect("first enable should issue a token");
    assert_eq!(URL_SAFE_NO_PAD.decode(&first_token).unwrap().len(), 32);
    assert!(enabled.status.enabled);
    assert!(enabled.status.configured);
    assert_eq!(enabled.status.port, 17082);
    assert!(!enabled
        .status
        .masked_token
        .as_deref()
        .unwrap_or_default()
        .contains(&first_token));

    let disabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest { enabled: false }),
    )
    .await
    .expect("disable should succeed")
    .0;
    assert!(!disabled.status.enabled);
    assert!(disabled.issued_token.is_none());

    let reenabled = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest { enabled: true }),
    )
    .await
    .expect("re-enable should succeed")
    .0;
    assert!(reenabled.issued_token.is_none());
    assert_eq!(state.lan_json_rpc_config().await.token, first_token);

    let rotated = rotate_lan_json_rpc_token(State(state.clone()))
        .await
        .expect("rotation should succeed")
        .0;
    let second_token = rotated.issued_token.expect("rotation should issue a token");
    assert_ne!(second_token, first_token);
    assert_eq!(state.lan_json_rpc_config().await.token, second_token);

    state.core.database.pool.close().await;
    let restored = bootstrap_http_app_state(&runtime)
        .await
        .expect("LAN config should restore after restart");
    let restored_config = restored.lan_json_rpc_config().await;
    assert!(restored_config.enabled);
    assert_eq!(restored_config.token, second_token);
    restored.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn lan_json_rpc_storage_failure_does_not_change_memory_state() {
    let (state, runtime) = lan_test_state("storage-failure").await;
    state.core.database.pool.close().await;

    let error = update_lan_json_rpc(
        State(state.clone()),
        ApiJson(UpdateLanJsonRpcRequest { enabled: true }),
    )
    .await
    .expect_err("closed database should reject update");

    assert_eq!(
        error.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        state.lan_json_rpc_config().await,
        LanJsonRpcConfig::default()
    );
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

async fn lan_test_state(label: &str) -> (Arc<HttpAppState>, ServerRuntimeConfig) {
    let index = LAN_TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let app_data_dir = std::env::temp_dir().join(format!(
        "motrix-fnos-lan-settings-{label}-{}-{index}",
        std::process::id()
    ));
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    (state, runtime)
}
