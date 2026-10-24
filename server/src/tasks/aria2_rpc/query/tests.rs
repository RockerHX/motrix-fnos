use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};

fn test_config() -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: 6800,
        rpc_secret: String::new(),
        session_path: None,
        state_dir: None,
        log_path: None,
    }
}

#[test]
fn engine_activity_detects_upload_and_integrity_work_without_download_speed() {
    let uploading: Aria2ActiveTaskActivity =
        serde_json::from_value(serde_json::json!({"bittorrent": {}, "uploadSpeed": "12"})).unwrap();
    assert!(uploading.is_bt_uploading());
    let verifying: Aria2ActiveTaskActivity =
        serde_json::from_value(serde_json::json!({"verifiedLength": "0"})).unwrap();
    assert!(verifying.is_verifying());
    let pending: Aria2ActiveTaskActivity =
        serde_json::from_value(serde_json::json!({"verifyIntegrityPending": true})).unwrap();
    assert!(pending.is_verifying());
    let idle: Aria2ActiveTaskActivity =
        serde_json::from_value(serde_json::json!({"uploadSpeed": "0"})).unwrap();
    assert!(!idle.is_verifying());
    assert!(!idle.is_bt_uploading());
    let fields = status_fields();
    assert!(fields
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "uploadSpeed"));
    assert!(fields
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field == "verifyIntegrityPending"));
}

#[test]
fn native_status_fields_do_not_request_legacy_followed_gid() {
    let mut config = test_config();
    config.state_dir = Some("/private/state".to_string());
    let fields = build_tell_status_request(&config, "gid-1")["params"][1]
        .as_array()
        .expect("fields should be array")
        .clone();
    assert!(!fields.iter().any(|field| field == "followedBy"));
    assert!(fields.iter().any(|field| field == "bittorrent"));
}
