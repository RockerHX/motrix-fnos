use crate::config::aria2::Aria2Config;
use std::path::PathBuf;

use super::{ARIA2_LOG_LEVEL, ARIA2_LOG_MAX_FILES, ARIA2_LOG_MAX_SIZE_MIB};

pub(super) fn detect_ca_certificate_path() -> Option<PathBuf> {
    ca_certificate_candidates()
        .into_iter()
        .find(|path| path.is_file())
}

fn ca_certificate_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if cfg!(target_os = "macos") {
        candidates.push(PathBuf::from("/etc/ssl/cert.pem"));
        candidates.push(PathBuf::from("/opt/homebrew/etc/ca-certificates/cert.pem"));
        candidates.push(PathBuf::from("/usr/local/etc/ca-certificates/cert.pem"));
    }

    candidates.push(PathBuf::from("/etc/ssl/certs/ca-certificates.crt"));
    candidates.push(PathBuf::from("/etc/pki/tls/certs/ca-bundle.crt"));
    candidates.push(PathBuf::from("/etc/ssl/ca-bundle.pem"));

    candidates
}

pub fn process_args(config: &Aria2Config) -> Vec<String> {
    let mut args = vec![
        "--enable-rpc=true".to_string(),
        format!("--rpc-listen-port={}", config.rpc_port),
        "--rpc-listen-all=false".to_string(),
        format!("--rpc-secret={}", config.rpc_secret),
        "--no-conf=true".to_string(),
        "--continue=true".to_string(),
        "--pause=true".to_string(),
        "--enable-dht=true".to_string(),
        "--enable-peer-exchange=true".to_string(),
        "--bt-enable-lpd=true".to_string(),
        "--listen-port=6881-6999".to_string(),
        "--dht-listen-port=6881-6999".to_string(),
        "--save-session-interval=30".to_string(),
        "--force-save=true".to_string(),
        "--console-log-level=warn".to_string(),
        format!("--log-level={ARIA2_LOG_LEVEL}"),
        format!("--log-max-size={ARIA2_LOG_MAX_SIZE_MIB}M"),
        format!("--log-max-files={ARIA2_LOG_MAX_FILES}"),
    ];

    if let Some(session_path) = config.session_path.as_deref() {
        args.push(format!("--input-file={session_path}"));
        args.push(format!("--save-session={session_path}"));
        if let Some(runtime_dir) = PathBuf::from(session_path).parent() {
            args.push(format!(
                "--dht-file-path={}",
                runtime_dir.join("dht.dat").display()
            ));
        }
    }

    if let Some(log_path) = config.log_path.as_deref() {
        args.push(format!("--log={log_path}"));
    }

    if let Some(path) = detect_ca_certificate_path() {
        args.push(format!("--ca-certificate={}", path.display()));
    }

    args
}

pub fn summarize_args(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.starts_with("--rpc-secret=") {
                "--rpc-secret=***".to_string()
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
