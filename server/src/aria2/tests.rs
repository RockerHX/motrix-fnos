use super::sidecar::classify_saved_sidecar_from_command_line;
use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};

fn test_config(path: Option<&str>) -> Aria2Config {
    Aria2Config {
        aria2_path: path.map(ToOwned::to_owned),
        binary_source: if path.is_some() {
            Aria2BinarySource::ExternalPath
        } else {
            Aria2BinarySource::Sidecar
        },
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: 6800,
        rpc_secret: String::new(),
        session_path: None,
        log_path: None,
    }
}

fn runtime_info(port: u16, source: Aria2BinarySource) -> SavedAria2Runtime {
    SavedAria2Runtime {
        pid: 42,
        actual_port: port,
        rpc_secret: "secret".to_string(),
        binary_source: source,
        sidecar_name: Some("aria2-next".to_string()),
        app_data_dir: Some("/tmp/motrix-fnos".to_string()),
        aria2_session_path: None,
        aria2_log_path: None,
    }
}

#[test]
fn config_status_uses_sidecar_when_external_path_is_missing() {
    let status = Aria2ConfigStatus::from_config(&test_config(None));

    assert!(status.configured);
    assert_eq!(status.binary_source, Aria2BinarySource::Sidecar);
    assert_eq!(status.sidecar_name, "aria2-next");
}

#[test]
fn saved_sidecar_is_owned_only_when_record_matches_candidate() {
    let runtime = runtime_info(6800, Aria2BinarySource::Sidecar);

    assert_eq!(
        classify_saved_sidecar_from_command_line(
            Some(&runtime),
            6800,
            Some("./aria2-next --rpc-listen-port=6800 --rpc-secret=secret")
        ),
        SidecarOwnership::OwnSidecar
    );
    assert_eq!(
        classify_saved_sidecar_from_command_line(
            Some(&runtime),
            16800,
            Some("./aria2-next --rpc-listen-port=6800 --rpc-secret=secret")
        ),
        SidecarOwnership::ExternalOrUnknown
    );
}

#[test]
fn runtime_config_sets_actual_port_and_secret() {
    let config = runtime_config(&test_config(None), 16800, "secret".to_string());

    assert_eq!(config.rpc_port, 16800);
    assert_eq!(config.rpc_secret, "secret");
}

#[test]
fn process_args_include_session_paths_when_configured() {
    let mut config = test_config(None);
    config.session_path = Some("/tmp/motrix-fnos/aria2/aria2.session".to_string());
    config.log_path = Some("/tmp/motrix-fnos/aria2/aria2.log".to_string());
    let args = process_args(&config);

    assert!(args.contains(&"--pause=true".to_string()));
    assert!(args.contains(&"--enable-dht=true".to_string()));
    assert!(args.contains(&"--enable-peer-exchange=true".to_string()));
    assert!(args.contains(&"--bt-enable-lpd=true".to_string()));
    assert!(args.contains(&"--listen-port=6881-6999".to_string()));
    assert!(args.contains(&"--dht-listen-port=6881-6999".to_string()));
    assert!(args.contains(&"--save-session-interval=30".to_string()));
    assert!(args.contains(&"--force-save=true".to_string()));
    assert!(args.contains(&"--input-file=/tmp/motrix-fnos/aria2/aria2.session".to_string()));
    assert!(args.contains(&"--save-session=/tmp/motrix-fnos/aria2/aria2.session".to_string()));
    assert!(args.contains(&"--dht-file-path=/tmp/motrix-fnos/aria2/dht.dat".to_string()));
    assert!(args.contains(&"--log=/tmp/motrix-fnos/aria2/aria2.log".to_string()));

    let log_args = args
        .iter()
        .filter(|arg| arg.starts_with("--log-") || arg.starts_with("--log="))
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        log_args,
        vec![
            "--log-level=warn",
            "--log-max-size=10M",
            "--log-max-files=3",
            "--log=/tmp/motrix-fnos/aria2/aria2.log",
        ]
    );
}

#[test]
fn summarized_process_args_redact_rpc_secret() {
    let mut config = test_config(None);
    config.rpc_secret = "super-secret".to_string();
    let summary = summarize_args(&process_args(&config));

    assert!(summary.contains("--rpc-secret=***"));
    assert!(!summary.contains("super-secret"));
}

#[test]
fn rpc_port_candidates_use_primary_then_fallback_range() {
    let candidates = rpc_port_candidates();

    assert_eq!(candidates.first(), Some(&6800));
    assert_eq!(candidates[1], 16800);
    assert_eq!(candidates.last(), Some(&16820));
    assert_eq!(candidates.len(), 22);
}
