use super::*;

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
