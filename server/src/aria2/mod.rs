use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use serde::{Deserialize, Serialize};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ConfigStatus {
    pub configured: bool,
    pub path: Option<String>,
    pub path_exists: bool,
    pub binary_source: Aria2BinarySource,
    pub sidecar_name: String,
    pub target_triple: String,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_secret_configured: bool,
    pub ca_certificate_path: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAria2Runtime {
    pub pid: u32,
    pub actual_port: u16,
    pub rpc_secret: String,
    pub binary_source: Aria2BinarySource,
    pub sidecar_name: Option<String>,
    pub app_data_dir: Option<String>,
    pub aria2_session_path: Option<String>,
    pub aria2_log_path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarOwnership {
    OwnSidecar,
    ExternalOrUnknown,
}

pub fn generate_rpc_secret() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("motrix-fnos-{nanos}-{}", std::process::id())
}

pub fn runtime_config(base: &Aria2Config, actual_port: u16, rpc_secret: String) -> Aria2Config {
    let mut config = base.clone();
    config.rpc_port = actual_port;
    config.rpc_secret = rpc_secret;
    config
}

pub fn classify_saved_sidecar(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    debug_logs: &DebugLogStore,
) -> SidecarOwnership {
    let Some(runtime) = saved else {
        return SidecarOwnership::ExternalOrUnknown;
    };

    let command_line = match read_process_command_line(runtime.pid) {
        Ok(command_line) => command_line,
        Err(error) => {
            debug_logs.warn(
                "aria2.cleanup",
                format!("残留 sidecar 命令行读取失败，按未知进程处理：{}", error),
            );
            return SidecarOwnership::ExternalOrUnknown;
        }
    };

    classify_saved_sidecar_from_command_line(Some(runtime), candidate_port, Some(&command_line))
}

fn classify_saved_sidecar_from_command_line(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    command_line: Option<&str>,
) -> SidecarOwnership {
    let Some(runtime) = saved else {
        return SidecarOwnership::ExternalOrUnknown;
    };

    if runtime.binary_source != Aria2BinarySource::Sidecar
        || runtime.actual_port != candidate_port
        || runtime.rpc_secret.trim().is_empty()
        || runtime.pid == 0
    {
        return SidecarOwnership::ExternalOrUnknown;
    }

    let Some(command_line) = command_line else {
        return SidecarOwnership::ExternalOrUnknown;
    };
    let evidence = analyze_sidecar_command_line(command_line, runtime, candidate_port);

    if evidence.contains_sidecar_name
        && evidence.contains_rpc_port
        && evidence.contains_rpc_secret
        && evidence.matched_count() >= 3
    {
        SidecarOwnership::OwnSidecar
    } else {
        SidecarOwnership::ExternalOrUnknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SidecarCommandLineEvidence {
    pub contains_sidecar_name: bool,
    pub contains_rpc_port: bool,
    pub contains_rpc_secret: bool,
    pub contains_app_data_path: bool,
    pub contains_session_path: bool,
    pub contains_log_path: bool,
}

impl SidecarCommandLineEvidence {
    pub fn matched_count(&self) -> usize {
        [
            self.contains_sidecar_name,
            self.contains_rpc_port,
            self.contains_rpc_secret,
            self.contains_app_data_path,
            self.contains_session_path,
            self.contains_log_path,
        ]
        .into_iter()
        .filter(|matched| *matched)
        .count()
    }
}

pub(crate) fn analyze_sidecar_command_line(
    command_line: &str,
    runtime: &SavedAria2Runtime,
    candidate_port: u16,
) -> SidecarCommandLineEvidence {
    let normalized_command = normalize_path_text(command_line);

    SidecarCommandLineEvidence {
        contains_sidecar_name: runtime
            .sidecar_name
            .as_deref()
            .map(|name| !name.trim().is_empty() && command_line.contains(name))
            .unwrap_or(false),
        contains_rpc_port: command_line_contains_rpc_port(command_line, candidate_port),
        contains_rpc_secret: !runtime.rpc_secret.trim().is_empty()
            && command_line.contains(&format!("--rpc-secret={}", runtime.rpc_secret)),
        contains_app_data_path: optional_path_matches(
            &normalized_command,
            runtime.app_data_dir.as_deref(),
        ),
        contains_session_path: optional_path_matches(
            &normalized_command,
            runtime.aria2_session_path.as_deref(),
        ),
        contains_log_path: optional_path_matches(
            &normalized_command,
            runtime.aria2_log_path.as_deref(),
        ),
    }
}

fn command_line_contains_rpc_port(command_line: &str, candidate_port: u16) -> bool {
    let plain = format!("--rpc-listen-port={candidate_port}");
    let quoted = format!("--rpc-listen-port=\"{candidate_port}\"");
    command_line.contains(&plain) || command_line.contains(&quoted)
}

fn optional_path_matches(normalized_command: &str, path: Option<&str>) -> bool {
    path.map(normalize_path_text)
        .filter(|path| !path.trim().is_empty())
        .map(|path| normalized_command.contains(&path))
        .unwrap_or(false)
}

fn normalize_path_text(value: &str) -> String {
    value.replace('\\', "/")
}

#[cfg(unix)]
pub(crate) fn read_process_command_line(pid: u32) -> Result<String, String> {
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map_err(|error| format!("读取进程命令行失败，PID {}：{}", pid, error))?;

    if !output.status.success() {
        return Err(format!(
            "读取进程命令行失败，PID {}：ps 退出状态 {}",
            pid, output.status
        ));
    }

    let command_line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if command_line.is_empty() {
        return Err(format!("读取进程命令行失败，PID {}：结果为空", pid));
    }

    Ok(command_line)
}

#[cfg(windows)]
pub(crate) fn read_process_command_line(pid: u32) -> Result<String, String> {
    let query = format!(
        "(Get-CimInstance Win32_Process -Filter \"ProcessId = {}\").CommandLine",
        pid
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &query])
        .output()
        .map_err(|error| format!("读取进程命令行失败，PID {}：{}", pid, error))?;

    if !output.status.success() {
        return Err(format!(
            "读取进程命令行失败，PID {}：PowerShell 退出状态 {}",
            pid, output.status
        ));
    }

    let command_line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if command_line.is_empty() {
        return Err(format!("读取进程命令行失败，PID {}：结果为空", pid));
    }

    Ok(command_line)
}

pub fn cleanup_saved_sidecar_if_owned(
    saved: Option<&SavedAria2Runtime>,
    candidate_port: u16,
    debug_logs: &DebugLogStore,
) -> bool {
    let Some(runtime) = saved else {
        return false;
    };
    if runtime.actual_port != candidate_port {
        debug_logs.warn(
            "aria2.cleanup",
            format!(
                "跳过残留 sidecar 清理：运行态端口 {} 与候选端口 {} 不一致",
                runtime.actual_port, candidate_port
            ),
        );
        return false;
    }

    if !terminate_process(runtime.pid) {
        debug_logs.warn(
            "aria2.cleanup",
            format!("本应用残留 sidecar PID {} 清理未确认成功", runtime.pid),
        );
        return false;
    }

    debug_logs.info(
        "aria2.cleanup",
        format!(
            "已清理本应用残留 Aria2 sidecar，PID {}，端口 {}",
            runtime.pid, runtime.actual_port
        ),
    );
    true
}

#[cfg(unix)]
pub(crate) fn terminate_process(pid: u32) -> bool {
    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
    if wait_until_process_exits(pid, Duration::from_millis(800)) {
        return true;
    }

    let _ = std::process::Command::new("kill")
        .arg("-KILL")
        .arg(pid.to_string())
        .status();
    wait_until_process_exits(pid, Duration::from_millis(800))
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}

#[cfg(windows)]
pub(crate) fn terminate_process(pid: u32) -> bool {
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
    wait_until_process_exits(pid, Duration::from_millis(800))
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

#[cfg(windows)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
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

impl Aria2ConfigStatus {
    pub fn from_config(config: &Aria2Config) -> Self {
        let path_exists = config
            .aria2_path
            .as_deref()
            .map(|path| Path::new(path).is_file())
            .unwrap_or(false);

        Self {
            configured: config.aria2_path.is_some()
                || config.binary_source == Aria2BinarySource::Sidecar,
            path: config.aria2_path.clone(),
            path_exists,
            binary_source: config.binary_source.clone(),
            sidecar_name: config.sidecar_name.clone(),
            target_triple: config.target_triple.clone(),
            rpc_host: config.rpc_host.clone(),
            rpc_port: config.rpc_port,
            rpc_secret_configured: !config.rpc_secret.is_empty(),
            ca_certificate_path: detect_ca_certificate_path()
                .map(|path| path.display().to_string()),
        }
    }
}

fn rpc_port_in_use(config: &Aria2Config) -> bool {
    let Ok(addresses) = (config.rpc_host.as_str(), config.rpc_port).to_socket_addrs() else {
        return false;
    };

    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok())
}

pub fn rpc_ports_exhausted_message() -> String {
    "Aria2 RPC 端口范围 6800, 16800-16820 均被占用，无法启动内置引擎".to_string()
}

pub fn rpc_port_candidates() -> Vec<u16> {
    std::iter::once(6800).chain(16800..=16820).collect()
}

pub fn select_available_rpc_port(config: &Aria2Config) -> Option<u16> {
    select_available_rpc_port_from(config, rpc_port_candidates())
}

pub fn select_rpc_port_with_saved_runtime(
    config: &Aria2Config,
    saved: Option<&SavedAria2Runtime>,
    debug_logs: &DebugLogStore,
) -> Option<u16> {
    for port in rpc_port_candidates() {
        let mut candidate_config = config.clone();
        candidate_config.rpc_port = port;
        if !rpc_port_in_use(&candidate_config) {
            return Some(port);
        }

        match classify_saved_sidecar(saved, port, debug_logs) {
            SidecarOwnership::OwnSidecar => {
                if !cleanup_saved_sidecar_if_owned(saved, port, debug_logs) {
                    debug_logs.error(
                        "aria2.cleanup",
                        format!(
                            "检测到本应用残留 sidecar 占用端口 {}，但清理失败，停止启动新 Aria2 避免继续下载",
                            port
                        ),
                    );
                    return None;
                }

                std::thread::sleep(Duration::from_millis(300));
                if !rpc_port_in_use(&candidate_config) {
                    return Some(port);
                }

                debug_logs.warn(
                    "aria2.cleanup",
                    format!("清理本应用残留 sidecar 后端口 {} 仍被占用", port),
                );
                return None;
            }
            SidecarOwnership::ExternalOrUnknown => {
                debug_logs.info(
                    "aria2.cleanup",
                    format!("端口 {} 已被占用但未确认属于本应用 sidecar，跳过该端口", port),
                );
            }
        }
    }

    None
}

fn select_available_rpc_port_from(
    config: &Aria2Config,
    candidates: impl IntoIterator<Item = u16>,
) -> Option<u16> {
    candidates.into_iter().find(|port| {
        let mut candidate_config = config.clone();
        candidate_config.rpc_port = *port;
        !rpc_port_in_use(&candidate_config)
    })
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
