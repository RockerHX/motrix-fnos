use super::*;

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
