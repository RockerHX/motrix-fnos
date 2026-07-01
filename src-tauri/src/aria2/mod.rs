use crate::app::{Aria2RuntimeInfo, ManagedAria2Process};
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub binary_source: Option<Aria2BinarySource>,
    pub message: String,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidecarOwnership {
    OwnSidecar,
    ExternalOrUnknown,
}

pub fn classify_saved_sidecar(
    saved: Option<&Aria2RuntimeInfo>,
    candidate_port: u16,
) -> SidecarOwnership {
    match saved {
        Some(runtime)
            if runtime.binary_source == Aria2BinarySource::Sidecar
                && runtime.actual_port == candidate_port
                && !runtime.rpc_secret.trim().is_empty()
                && runtime.pid > 0 =>
        {
            SidecarOwnership::OwnSidecar
        }
        _ => SidecarOwnership::ExternalOrUnknown,
    }
}

pub fn cleanup_saved_sidecar_if_owned(
    saved: Option<&Aria2RuntimeInfo>,
    candidate_port: u16,
    debug_logs: &DebugLogStore,
) -> bool {
    if classify_saved_sidecar(saved, candidate_port) != SidecarOwnership::OwnSidecar {
        return false;
    }

    let Some(runtime) = saved else {
        return false;
    };

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

#[derive(Debug, serde::Deserialize)]
struct JsonRpcResponse {
    result: Option<Aria2VersionResult>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, serde::Deserialize)]
struct EmptyJsonRpcResponse {
    error: Option<JsonRpcError>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2VersionResult {
    version: String,
}

#[derive(Debug, serde::Deserialize)]
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

pub fn process_status(
    process: &Mutex<Option<ManagedAria2Process>>,
) -> Result<Aria2ProcessStatus, String> {
    let guard = process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;

    Ok(match guard.as_ref() {
        Some(child) => Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            binary_source: Some(match child {
                ManagedAria2Process::External(_) => Aria2BinarySource::ExternalPath,
                ManagedAria2Process::Sidecar(_) => Aria2BinarySource::Sidecar,
            }),
            message: "Aria2 进程已启动".to_string(),
        },
        None => Aria2ProcessStatus {
            running: false,
            pid: None,
            binary_source: None,
            message: "Aria2 进程未启动".to_string(),
        },
    })
}

pub fn start_process(
    app: &AppHandle,
    process: &Mutex<Option<ManagedAria2Process>>,
    config: &Aria2Config,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = match process.lock() {
        Ok(guard) => guard,
        Err(_) => {
            debug_logs.error("aria2", "无法写入 Aria2 进程状态");
            return Err("无法写入 Aria2 进程状态".to_string());
        }
    };

    if let Some(child) = guard.as_ref() {
        debug_logs.info("aria2", format!("Aria2 进程已在运行，PID {}", child.id()));
        return Ok(Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            binary_source: Some(match child {
                ManagedAria2Process::External(_) => Aria2BinarySource::ExternalPath,
                ManagedAria2Process::Sidecar(_) => Aria2BinarySource::Sidecar,
            }),
            message: "Aria2 进程已在运行".to_string(),
        });
    }

    if rpc_port_in_use(config) {
        let error = format!(
            "Aria2 RPC 端口 {}:{} 已被其他进程占用，请先退出残留的 Aria2 Next 进程后重试",
            config.rpc_host, config.rpc_port
        );
        debug_logs.error("aria2", &error);
        return Err(error);
    }

    let args = process_args(config);
    log_start_summary(debug_logs, config, &args);
    let managed = match config.binary_source {
        Aria2BinarySource::ExternalPath => match start_external_process(config, &args) {
            Ok(process) => process,
            Err(error) => {
                debug_logs.error("aria2", format!("启动外部 Aria2 Next 失败：{}", error));
                return Err(error);
            }
        },
        Aria2BinarySource::Sidecar => match start_sidecar_process(app, config, &args) {
            Ok(process) => process,
            Err(error) => {
                debug_logs.error(
                    "aria2",
                    format!("启动内置 Aria2 Next sidecar 失败：{}", error),
                );
                return Err(error);
            }
        },
    };
    let pid = managed.id();
    let source = match &managed {
        ManagedAria2Process::External(_) => Aria2BinarySource::ExternalPath,
        ManagedAria2Process::Sidecar(_) => Aria2BinarySource::Sidecar,
    };
    *guard = Some(managed);
    debug_logs.info(
        "aria2",
        format!(
            "Aria2 进程启动成功，来源 {}，PID {}",
            source_label(&source),
            pid
        ),
    );

    Ok(Aria2ProcessStatus {
        running: true,
        pid: Some(pid),
        binary_source: Some(source.clone()),
        message: format!("Aria2 进程启动成功（{}）", source_label(&source)),
    })
}

pub fn stop_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = match process.lock() {
        Ok(guard) => guard,
        Err(_) => {
            debug_logs.error("aria2", "无法写入 Aria2 进程状态");
            return Err("无法写入 Aria2 进程状态".to_string());
        }
    };

    if let Some(child) = guard.take() {
        let pid = child.id();
        debug_logs.info("aria2", format!("准备停止 Aria2 进程，PID {}", pid));
        if let Err(error) = child.kill() {
            debug_logs.warn(
                "aria2",
                format!(
                    "停止 Aria2 进程句柄失败，尝试按 PID 兜底终止，PID {}：{}",
                    pid, error
                ),
            );
        }
        if !wait_until_process_exits(pid, Duration::from_millis(800)) && !terminate_process(pid) {
            let error = format!("停止 Aria2 进程后 PID {} 仍然存活", pid);
            debug_logs.error("aria2", &error);
            return Err(error);
        }
        debug_logs.info("aria2", format!("Aria2 进程已停止，PID {}", pid));
    } else {
        debug_logs.info("aria2", "停止 Aria2 进程：当前没有运行中的进程");
    }

    Ok(Aria2ProcessStatus {
        running: false,
        pid: None,
        binary_source: None,
        message: "Aria2 进程已停止".to_string(),
    })
}

fn start_external_process(
    config: &Aria2Config,
    args: &[String],
) -> Result<ManagedAria2Process, String> {
    let aria2_path = config
        .aria2_path
        .as_deref()
        .ok_or_else(|| "未配置 Aria2 Next 路径，请设置 MOTRIX_FNOS_ARIA2_PATH".to_string())?;

    if !Path::new(aria2_path).is_file() {
        return Err(format!("Aria2 Next 路径不存在或不是文件：{}", aria2_path));
    }

    let child = Command::new(aria2_path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("启动外部 Aria2 Next 失败：{}", error))?;

    Ok(ManagedAria2Process::External(child))
}

fn start_sidecar_process(
    app: &AppHandle,
    config: &Aria2Config,
    args: &[String],
) -> Result<ManagedAria2Process, String> {
    let command = app
        .shell()
        .sidecar(&config.sidecar_name)
        .map_err(|error| format!("加载内置 Aria2 Next sidecar 失败：{}", error))?;
    let (mut rx, child) = command
        .args(args)
        .spawn()
        .map_err(|error| format!("启动内置 Aria2 Next sidecar 失败：{}", error))?;

    tauri::async_runtime::spawn(async move { while rx.recv().await.is_some() {} });

    Ok(ManagedAria2Process::Sidecar(child))
}

fn process_args(config: &Aria2Config) -> Vec<String> {
    let mut args = vec![
        "--enable-rpc=true".to_string(),
        format!("--rpc-listen-port={}", config.rpc_port),
        "--rpc-listen-all=false".to_string(),
        format!("--rpc-secret={}", config.rpc_secret),
        "--no-conf=true".to_string(),
        "--continue=true".to_string(),
        "--console-log-level=warn".to_string(),
    ];

    if let Some(path) = detect_ca_certificate_path() {
        args.push(format!("--ca-certificate={}", path.display()));
    }

    args
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
    saved: Option<&Aria2RuntimeInfo>,
    debug_logs: &DebugLogStore,
) -> Option<u16> {
    for port in rpc_port_candidates() {
        let mut candidate_config = config.clone();
        candidate_config.rpc_port = port;
        if !rpc_port_in_use(&candidate_config) {
            return Some(port);
        }

        if classify_saved_sidecar(saved, port) == SidecarOwnership::OwnSidecar {
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

fn source_label(source: &Aria2BinarySource) -> &'static str {
    match source {
        Aria2BinarySource::ExternalPath => "外部路径",
        Aria2BinarySource::Sidecar => "内置 sidecar",
    }
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

fn log_start_summary(debug_logs: &DebugLogStore, config: &Aria2Config, args: &[String]) {
    debug_logs.info(
        "aria2",
        format!(
            "准备启动 Aria2 Next，来源 {}，target {}，RPC {}:{}，参数 {}",
            source_label(&config.binary_source),
            config.target_triple,
            config.rpc_host,
            config.rpc_port,
            summarize_args(args)
        ),
    );

    if let Some(path) = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--ca-certificate="))
    {
        debug_logs.info("aria2.ca", format!("CA 证书探测成功：{}", path));
    } else {
        debug_logs.warn("aria2.ca", "未探测到可用 CA 证书路径");
    }
}

fn summarize_args(args: &[String]) -> String {
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
    fn start_external_process_returns_clear_error_for_invalid_path() {
        let process = Mutex::new(None);
        let error = start_process_without_sidecar_for_test(
            &process,
            &test_config(Some("/definitely/missing/aria2")),
        )
        .expect_err("invalid path should fail");

        assert!(error.contains("路径不存在"));
    }

    fn start_process_without_sidecar_for_test(
        process: &Mutex<Option<ManagedAria2Process>>,
        config: &Aria2Config,
    ) -> Result<Aria2ProcessStatus, String> {
        let mut guard = process.lock().map_err(|_| "lock failed".to_string())?;
        if guard.is_some() {
            return Ok(Aria2ProcessStatus {
                running: true,
                pid: None,
                binary_source: Some(Aria2BinarySource::ExternalPath),
                message: "Aria2 进程已在运行".to_string(),
            });
        }
        let args = process_args(config);
        let managed = start_external_process(config, &args)?;
        let pid = managed.id();
        *guard = Some(managed);
        Ok(Aria2ProcessStatus {
            running: true,
            pid: Some(pid),
            binary_source: Some(Aria2BinarySource::ExternalPath),
            message: "Aria2 进程启动成功".to_string(),
        })
    }

    fn runtime_info(port: u16, source: Aria2BinarySource) -> Aria2RuntimeInfo {
        Aria2RuntimeInfo {
            pid: 42,
            actual_port: port,
            rpc_secret: "secret".to_string(),
            rpc_endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
            binary_source: source,
        }
    }

    #[test]
    fn saved_sidecar_is_owned_only_when_record_matches_candidate() {
        let runtime = runtime_info(6800, Aria2BinarySource::Sidecar);

        assert_eq!(
            classify_saved_sidecar(Some(&runtime), 6800),
            SidecarOwnership::OwnSidecar
        );
        assert_eq!(
            classify_saved_sidecar(Some(&runtime), 16800),
            SidecarOwnership::ExternalOrUnknown
        );
        assert_eq!(
            classify_saved_sidecar(None, 6800),
            SidecarOwnership::ExternalOrUnknown
        );
    }

    #[test]
    fn external_or_incomplete_runtime_is_not_owned_sidecar() {
        let external = runtime_info(6800, Aria2BinarySource::ExternalPath);
        let mut missing_secret = runtime_info(6800, Aria2BinarySource::Sidecar);
        missing_secret.rpc_secret.clear();

        assert_eq!(
            classify_saved_sidecar(Some(&external), 6800),
            SidecarOwnership::ExternalOrUnknown
        );
        assert_eq!(
            classify_saved_sidecar(Some(&missing_secret), 6800),
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
    fn process_args_include_rpc_defaults() {
        let args = process_args(&test_config(None));

        assert!(args.contains(&"--enable-rpc=true".to_string()));
        assert!(args.contains(&"--rpc-listen-port=6800".to_string()));
        assert!(args.contains(&"--rpc-listen-all=false".to_string()));
        assert!(args.contains(&"--rpc-secret=".to_string()));
        assert!(args.contains(&"--no-conf=true".to_string()));
    }

    #[test]
    fn process_args_include_runtime_secret_when_configured() {
        let mut config = test_config(None);
        config.rpc_secret = "secret".to_string();
        let args = process_args(&config);

        assert!(args.contains(&"--rpc-secret=secret".to_string()));
    }

    #[test]
    fn rpc_port_candidates_use_primary_then_fallback_range() {
        let candidates = rpc_port_candidates();

        assert_eq!(candidates.first(), Some(&6800));
        assert_eq!(candidates[1], 16800);
        assert_eq!(candidates.last(), Some(&16820));
        assert_eq!(candidates.len(), 22);
    }

    #[test]
    fn occupied_external_port_is_not_selected() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
        let occupied = listener
            .local_addr()
            .expect("test listener should have local addr")
            .port();
        let config = test_config(None);

        assert_eq!(select_available_rpc_port_from(&config, [occupied]), None);
    }

    #[test]
    fn select_available_rpc_port_skips_occupied_candidate() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
        let occupied = listener
            .local_addr()
            .expect("test listener should have local addr")
            .port();
        let free = std::net::TcpListener::bind(("127.0.0.1", 0))
            .expect("free probe should bind")
            .local_addr()
            .expect("free probe should have local addr")
            .port();
        let config = test_config(None);

        assert_eq!(
            select_available_rpc_port_from(&config, [occupied, free]),
            Some(free)
        );
    }

    #[test]
    fn select_available_rpc_port_returns_none_when_candidates_are_occupied() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
        let occupied = listener
            .local_addr()
            .expect("test listener should have local addr")
            .port();
        let config = test_config(None);

        assert_eq!(select_available_rpc_port_from(&config, [occupied]), None);
    }

    #[test]
    fn rpc_ports_exhausted_message_mentions_candidate_range() {
        let message = rpc_ports_exhausted_message();

        assert!(message.contains("6800"));
        assert!(message.contains("16800-16820"));
    }

    #[test]
    fn rpc_port_in_use_detects_listening_port() {
        let listener =
            std::net::TcpListener::bind(("127.0.0.1", 0)).expect("test listener should bind");
        let port = listener
            .local_addr()
            .expect("test listener should have local addr")
            .port();
        let mut config = test_config(None);
        config.rpc_port = port;

        assert!(rpc_port_in_use(&config));
    }

    #[test]
    fn ca_certificate_candidates_include_platform_defaults() {
        let candidates = ca_certificate_candidates();

        if cfg!(target_os = "macos") {
            assert_eq!(
                candidates.first(),
                Some(&PathBuf::from("/etc/ssl/cert.pem"))
            );
        }
        assert!(candidates.contains(&PathBuf::from("/etc/ssl/certs/ca-certificates.crt")));
    }

    #[test]
    fn process_args_include_detected_ca_certificate_when_available() {
        if let Some(path) = detect_ca_certificate_path() {
            let args = process_args(&test_config(None));
            assert!(args.contains(&format!("--ca-certificate={}", path.display())));
        }
    }

    #[test]
    fn ping_rpc_returns_failure_when_server_is_unavailable() {
        let mut config = test_config(None);
        config.rpc_port = 9;

        let status = tauri::async_runtime::block_on(ping_rpc(&config, None));

        assert!(!status.connected);
        assert!(status.version.is_none());
        assert!(status.message.contains("RPC"));
    }

    #[test]
    fn global_options_are_clamped_and_serialized_for_aria2() {
        let mut config = test_config(None);
        config.rpc_secret = "secret".to_string();
        let options = global_options_from_values(128, 1024, 2048);

        let request = build_change_global_option_request(&config, &options);

        assert_eq!(options.max_concurrent_downloads, 64);
        assert_eq!(request["method"], "aria2.changeGlobalOption");
        assert_eq!(request["params"][0], "token:secret");
        assert_eq!(request["params"][1]["max-concurrent-downloads"], "64");
        assert_eq!(request["params"][1]["max-overall-download-limit"], "1024");
        assert_eq!(request["params"][1]["max-overall-upload-limit"], "2048");
    }
}
