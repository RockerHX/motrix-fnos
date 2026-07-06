mod config_status;
mod ports;
mod process_probe;
mod runtime_file;
mod sidecar;

pub use config_status::Aria2ConfigStatus;
pub use ports::{
    rpc_port_candidates, rpc_ports_exhausted_message, select_available_rpc_port,
    select_rpc_port_with_saved_runtime,
};
pub(crate) use process_probe::terminate_process;
pub use runtime_file::{runtime_config, SavedAria2Runtime};
pub use sidecar::{classify_saved_sidecar, cleanup_saved_sidecar_if_owned, SidecarOwnership};

#[cfg(test)]
use sidecar::classify_saved_sidecar_from_command_line;

use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_rpc_secret() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("motrix-fnos-{nanos}-{}", std::process::id())
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2RpcStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aria2GlobalOptions {
    pub max_concurrent_downloads: u32,
    pub download_limit: u64,
    pub upload_limit: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    result: Option<Aria2VersionResult>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct EmptyJsonRpcResponse {
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2VersionResult {
    version: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

fn detect_ca_certificate_path() -> Option<PathBuf> {
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

pub async fn ping_rpc(config: &Aria2Config, debug_logs: Option<&DebugLogStore>) -> Aria2RpcStatus {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(format!("token:{}", config.rpc_secret));
    }

    let request_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-version-check",
        "method": "aria2.getVersion",
        "params": params,
    });

    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.warn("aria2.rpc", format!("Aria2 RPC 暂不可用：{}", error));
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: format!("Aria2 RPC 连接失败：{}", error),
            };
        }
    };

    let rpc_response = match response.json::<JsonRpcResponse>().await {
        Ok(body) => body,
        Err(error) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.error("aria2.rpc", format!("Aria2 RPC 响应解析失败：{}", error));
            }
            return Aria2RpcStatus {
                connected: false,
                version: None,
                message: format!("Aria2 RPC 响应解析失败：{}", error),
            };
        }
    };

    if let Some(error) = rpc_response.error {
        if let Some(debug_logs) = debug_logs {
            debug_logs.error(
                "aria2.rpc",
                format!("Aria2 RPC 返回错误：{}", error.message),
            );
        }
        return Aria2RpcStatus {
            connected: false,
            version: None,
            message: format!("Aria2 RPC 返回错误：{}", error.message),
        };
    }

    match rpc_response.result {
        Some(result) => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.info(
                    "aria2.rpc",
                    format!("Aria2 RPC ready，版本 {}", result.version),
                );
            }
            Aria2RpcStatus {
                connected: true,
                version: Some(result.version.clone()),
                message: format!("Aria2 RPC 连接正常，版本 {}", result.version),
            }
        }
        None => {
            if let Some(debug_logs) = debug_logs {
                debug_logs.error("aria2.rpc", "Aria2 RPC 响应缺少版本信息");
            }
            Aria2RpcStatus {
                connected: false,
                version: None,
                message: "Aria2 RPC 响应缺少版本信息".to_string(),
            }
        }
    }
}

pub async fn save_session(
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_save_session_request(config);
    let response = reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("保存 Aria2 session 失败：无法连接 RPC（{}）", error))?;

    let rpc_response = response
        .json::<EmptyJsonRpcResponse>()
        .await
        .map_err(|error| format!("保存 Aria2 session 失败：响应解析失败（{}）", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("保存 Aria2 session 失败：{}", error.message));
    }

    if let Some(debug_logs) = debug_logs {
        debug_logs.info("aria2.session", "Aria2 session 已保存");
    }

    Ok(())
}

fn build_save_session_request(config: &Aria2Config) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-save-session",
        "method": "aria2.saveSession",
        "params": params,
    })
}

pub async fn apply_global_options(
    config: &Aria2Config,
    options: &Aria2GlobalOptions,
    debug_logs: Option<&DebugLogStore>,
) -> Result<(), String> {
    let request_body = build_change_global_option_request(config, options);
    let response = reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("应用 Aria2 下载配置失败：无法连接 RPC（{}）", error))?;

    let rpc_response = response
        .json::<EmptyJsonRpcResponse>()
        .await
        .map_err(|error| format!("应用 Aria2 下载配置失败：响应解析失败（{}）", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("应用 Aria2 下载配置失败：{}", error.message));
    }

    if let Some(debug_logs) = debug_logs {
        debug_logs.info(
            "aria2.options",
            format!(
                "已应用 Aria2 下载配置：最大并发 {}，下载限速 {} B/s，上传限速 {} B/s",
                options.max_concurrent_downloads, options.download_limit, options.upload_limit
            ),
        );
    }

    Ok(())
}

pub fn global_options_from_values(
    max_concurrent_downloads: u32,
    download_limit: u64,
    upload_limit: u64,
) -> Aria2GlobalOptions {
    Aria2GlobalOptions {
        max_concurrent_downloads: max_concurrent_downloads.clamp(1, 64),
        download_limit,
        upload_limit,
    }
}

fn build_change_global_option_request(
    config: &Aria2Config,
    options: &Aria2GlobalOptions,
) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    params.push(serde_json::json!({
        "max-concurrent-downloads": options.max_concurrent_downloads.to_string(),
        "max-overall-download-limit": options.download_limit.to_string(),
        "max-overall-upload-limit": options.upload_limit.to_string(),
    }));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-change-global-option",
        "method": "aria2.changeGlobalOption",
        "params": params,
    })
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
        "--save-session-interval=30".to_string(),
        "--force-save=true".to_string(),
        "--console-log-level=warn".to_string(),
    ];

    if let Some(session_path) = config.session_path.as_deref() {
        args.push(format!("--input-file={session_path}"));
        args.push(format!("--save-session={session_path}"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::aria2::Aria2BinarySource;

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
        assert!(args.contains(&"--save-session-interval=30".to_string()));
        assert!(args.contains(&"--force-save=true".to_string()));
        assert!(args.contains(&"--input-file=/tmp/motrix-fnos/aria2/aria2.session".to_string()));
        assert!(args.contains(&"--save-session=/tmp/motrix-fnos/aria2/aria2.session".to_string()));
        assert!(args.contains(&"--log=/tmp/motrix-fnos/aria2/aria2.log".to_string()));
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
}
