use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::tasks::{
    prepare_task, CreateDownloadTaskRequest, CreateTaskAdvancedOptions, DownloadTaskSourceType,
    DownloadTaskStartMode, PreparedDownloadTask, TaskProxyBinding,
};

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
        log_path: None,
    }
}

#[test]
fn add_uri_request_contains_url_and_options() {
    let task = PreparedDownloadTask {
        url: "https://example.com/file.zip".to_string(),
        file_name: "custom.zip".to_string(),
        output_file_name: Some("custom.zip".to_string()),
        save_dir: "/downloads".to_string(),
        aria2_save_dir: None,
        category: "默认".to_string(),
        source_type: DownloadTaskSourceType::Url,
        start_mode: DownloadTaskStartMode::Now,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::from_iter([
            (
                "split".to_string(),
                serde_json::Value::String("8".to_string()),
            ),
            (
                "max-connection-per-server".to_string(),
                serde_json::Value::String("8".to_string()),
            ),
            (
                "max-download-limit".to_string(),
                serde_json::Value::String("524288".to_string()),
            ),
        ]),
        use_proxy: true,
        proxy_binding: TaskProxyBinding::profile(Some("http://127.0.0.1:7890".to_string())),
    };

    let request = build_add_uri_request_with_id(&test_config(), &task, "test-add-uri");

    assert_eq!(request["method"], "aria2.addUri");
    assert_eq!(request["params"][0][0], "https://example.com/file.zip");
    assert_eq!(request["params"][1]["dir"], "/downloads");
    assert_eq!(request["params"][1]["out"], "custom.zip");
    assert_eq!(request["params"][1]["split"], "8");
    assert_eq!(request["params"][1]["max-connection-per-server"], "8");
    assert_eq!(request["params"][1]["max-download-limit"], "524288");
    assert_eq!(request["params"][1]["all-proxy"], "http://127.0.0.1:7890");
    assert_eq!(request["params"][1]["pause"], "false");
}

#[test]
fn add_uri_request_does_not_force_inferred_display_name_as_output() {
    let save_dir = temporary_download_dir("inferred-output");
    let task = prepare_task(CreateDownloadTaskRequest {
        url: "https://example.com/download?id=123".to_string(),
        file_name: None,
        save_dir: Some(save_dir.clone()),
        source_type: DownloadTaskSourceType::Url,
        start_mode: DownloadTaskStartMode::Now,
        category: None,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
    })
    .expect("URL task should be prepared");

    assert_eq!(task.file_name, "download");
    assert_eq!(task.output_file_name, None);
    let request = build_add_uri_request_with_id(&test_config(), &task, "test-inferred-output");
    assert!(request["params"][1].get("out").is_none());

    let _ = std::fs::remove_dir_all(save_dir);
}

#[test]
fn add_uri_request_keeps_paused_magnet_metadata_resolution_running() {
    let task = PreparedDownloadTask {
        url: "magnet:?xt=urn:btih:test".to_string(),
        file_name: "磁力链接任务".to_string(),
        output_file_name: None,
        save_dir: "/downloads".to_string(),
        aria2_save_dir: Some("/app-data/magnet-metadata/task-1".to_string()),
        category: "默认".to_string(),
        source_type: DownloadTaskSourceType::Magnet,
        start_mode: DownloadTaskStartMode::Paused,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
    };

    let request = build_add_uri_request_with_id(&test_config(), &task, "test-magnet-paused");

    assert_eq!(request["method"], "aria2.addUri");
    assert_eq!(request["params"][0][0], "magnet:?xt=urn:btih:test");
    assert_eq!(
        request["params"][1]["dir"],
        "/app-data/magnet-metadata/task-1"
    );
    assert_eq!(request["params"][1]["pause"], "false");
    assert_eq!(request["params"][1]["pause-metadata"], "true");
    assert_eq!(request["params"][1]["bt-save-metadata"], "true");
    assert!(request["params"][1].get("all-proxy").is_none());
    assert!(request["params"][1]["bt-tracker"]
        .as_str()
        .expect("bt-tracker should be string")
        .contains("tracker.opentrackr.org"));
    assert!(request["params"][1].get("out").is_none());
}

#[test]
fn add_uri_request_sets_pause_metadata_for_started_magnet() {
    let task = PreparedDownloadTask {
        url: "magnet:?xt=urn:btih:test".to_string(),
        file_name: "磁力链接任务".to_string(),
        output_file_name: None,
        save_dir: "/downloads".to_string(),
        aria2_save_dir: None,
        category: "默认".to_string(),
        source_type: DownloadTaskSourceType::Magnet,
        start_mode: DownloadTaskStartMode::Now,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
    };

    let request = build_add_uri_request_with_id(&test_config(), &task, "test-magnet-running");

    assert_eq!(request["params"][1]["pause-metadata"], "true");
    assert_eq!(request["params"][1]["bt-save-metadata"], "true");
    assert_eq!(request["params"][1]["pause"], "false");
    assert!(request["params"][1]["bt-tracker"]
        .as_str()
        .expect("bt-tracker should be string")
        .contains("tracker.opentrackr.org"));
}

#[test]
fn add_torrent_request_contains_base64_payload_and_options() {
    let task = PreparedDownloadTask {
        url: "torrent:example.torrent".to_string(),
        file_name: "example".to_string(),
        output_file_name: None,
        save_dir: "/downloads".to_string(),
        aria2_save_dir: None,
        category: "默认".to_string(),
        source_type: DownloadTaskSourceType::Url,
        start_mode: DownloadTaskStartMode::Paused,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
    };

    let request = build_add_torrent_request_with_id(
        &test_config(),
        &task,
        b"torrent-bytes",
        "test-add-torrent",
    );

    assert_eq!(request["method"], "aria2.addTorrent");
    assert_eq!(request["params"][0], "dG9ycmVudC1ieXRlcw==");
    assert_eq!(request["params"][1], serde_json::json!([]));
    assert_eq!(request["params"][2]["dir"], "/downloads");
    assert_eq!(request["params"][2]["pause"], "true");
    assert_eq!(request["params"][2]["pause-metadata"], "true");
    assert_eq!(request["params"][2]["seed-time"], "0");
    assert!(request["params"][2]["bt-tracker"]
        .as_str()
        .expect("bt-tracker should be string")
        .contains("tracker.opentrackr.org"));
}

fn temporary_download_dir(label: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_nanos();
    std::env::temp_dir()
        .join(format!(
            "motrix-fnos-aria2-create-{label}-{}-{timestamp}",
            std::process::id()
        ))
        .display()
        .to_string()
}
