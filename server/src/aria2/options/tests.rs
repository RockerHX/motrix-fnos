use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::settings::service::{
    DEFAULT_MAX_CONNECTION_PER_SERVER, DEFAULT_MAX_TRIES, DEFAULT_MIN_SPLIT_SIZE, DEFAULT_SPLIT,
    MAX_CONNECTION_PER_SERVER_LIMIT, MAX_CONNECT_TIMEOUT_LIMIT, MAX_SPLIT_LIMIT, MAX_TRIES_LIMIT,
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
        log_path: None,
    }
}

#[test]
fn global_options_map_all_download_tuning_fields() {
    let options = global_options_from_values(8, 1024, 2048, 6, 7, "5M", 90, 8);
    let request = build_change_global_option_request(&test_config(), &options);

    assert_eq!(
        request["params"][1],
        json!({
            "max-concurrent-downloads": "8",
            "max-connection-per-server": "6",
            "split": "7",
            "min-split-size": "5M",
            "connect-timeout": "90",
            "max-tries": "8",
            "max-overall-download-limit": "1024",
            "max-overall-upload-limit": "2048",
        })
    );
}

#[test]
fn global_options_normalize_invalid_download_tuning_fields() {
    let options = global_options_from_values(0, 0, 0, 0, 999, "invalid", 0, 999);

    assert_eq!(
        options.max_connection_per_server,
        DEFAULT_MAX_CONNECTION_PER_SERVER
    );
    assert_eq!(options.split, MAX_SPLIT_LIMIT);
    assert_eq!(options.min_split_size, DEFAULT_MIN_SPLIT_SIZE);
    assert_eq!(options.connect_timeout, 1);
    assert_eq!(options.max_tries, MAX_TRIES_LIMIT);

    let upper = global_options_from_values(
        1,
        0,
        0,
        MAX_CONNECTION_PER_SERVER_LIMIT + 1,
        DEFAULT_SPLIT,
        "20M",
        MAX_CONNECT_TIMEOUT_LIMIT + 1,
        DEFAULT_MAX_TRIES,
    );
    assert_eq!(
        upper.max_connection_per_server,
        MAX_CONNECTION_PER_SERVER_LIMIT
    );
    assert_eq!(upper.connect_timeout, MAX_CONNECT_TIMEOUT_LIMIT);
}
