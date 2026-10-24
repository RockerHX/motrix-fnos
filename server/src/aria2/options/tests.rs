use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::settings::service::{
    DEFAULT_HTTP_MAX_CONNECTIONS, HTTP_MAX_CONNECTIONS_LIMIT, MAX_CONNECT_TIMEOUT_LIMIT,
    MAX_TRIES_LIMIT,
};
use serde_json::json;

fn test_config() -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: 6800,
        rpc_secret: "test-secret".to_string(),
        session_path: None,
        state_dir: None,
        log_path: None,
    }
}

#[test]
fn global_options_map_legacy_http_connection_fields() {
    let options = global_options_from_values(8, 1024, 2048, 6, 90, 8);
    let request = build_change_global_option_request(&test_config(), &options);

    assert_eq!(
        request["params"][1],
        json!({
            "max-concurrent-downloads": "8",
            "max-connection-per-server": "6",
            "split": "6",
            "connect-timeout": "90",
            "max-tries": "8",
            "max-overall-download-limit": "1024",
            "max-overall-upload-limit": "2048",
        })
    );
}

#[test]
fn global_options_clamp_download_tuning_fields() {
    let options = global_options_from_values(0, 0, 0, 0, 999, 0);

    assert_eq!(options.http_max_connections, DEFAULT_HTTP_MAX_CONNECTIONS);
    assert_eq!(options.connect_timeout, MAX_CONNECT_TIMEOUT_LIMIT);
    assert_eq!(options.max_tries, 1);

    let upper = global_options_from_values(
        1,
        0,
        0,
        HTTP_MAX_CONNECTIONS_LIMIT + 1,
        MAX_CONNECT_TIMEOUT_LIMIT + 1,
        MAX_TRIES_LIMIT + 1,
    );
    assert_eq!(upper.http_max_connections, HTTP_MAX_CONNECTIONS_LIMIT);
    assert_eq!(upper.connect_timeout, MAX_CONNECT_TIMEOUT_LIMIT);
    assert_eq!(upper.max_tries, MAX_TRIES_LIMIT);
}

#[test]
fn native_global_options_use_stream_connection_limit() {
    let mut config = test_config();
    config.state_dir = Some("/private/native-state".to_string());
    let options = global_options_from_values(8, 0, 0, 6, 90, 8);
    let request = build_change_global_option_request(&config, &options);

    assert_eq!(request["params"][1]["stream-max-connections"], "6");
    assert!(request["params"][1].get("split").is_none());
    assert!(request["params"][1]
        .get("max-connection-per-server")
        .is_none());
}
