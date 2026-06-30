use crate::app::ManagedAria2Process;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
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

#[derive(Debug, serde::Deserialize)]
struct JsonRpcResponse {
    result: Option<Aria2VersionResult>,
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
            ca_certificate_path: detect_ca_certificate_path().map(|path| path.display().to_string()),
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
        debug_logs.info(
            "aria2",
            format!("Aria2 进程已在运行，PID {}", child.id()),
        );
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
                debug_logs.error("aria2", format!("启动内置 Aria2 Next sidecar 失败：{}", error));
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
        format!("Aria2 进程启动成功，来源 {}，PID {}", source_label(&source), pid),
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
        if let Err(error) = child.kill() {
            debug_logs.error("aria2", format!("停止 Aria2 进程失败：{}", error));
            return Err(format!("停止 Aria2 进程失败：{}", error));
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
        "--continue=true".to_string(),
        "--console-log-level=warn".to_string(),
    ];

    if !config.rpc_secret.is_empty() {
        args.push(format!("--rpc-secret={}", config.rpc_secret));
    }

    if let Some(path) = detect_ca_certificate_path() {
        args.push(format!("--ca-certificate={}", path.display()));
    }

    args
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
            debug_logs.error("aria2.rpc", format!("Aria2 RPC 返回错误：{}", error.message));
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
                debug_logs.info("aria2.rpc", format!("Aria2 RPC ready，版本 {}", result.version));
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

    #[test]
    fn process_args_include_rpc_defaults() {
        let args = process_args(&test_config(None));

        assert!(args.contains(&"--enable-rpc=true".to_string()));
        assert!(args.contains(&"--rpc-listen-port=6800".to_string()));
        assert!(args.contains(&"--rpc-listen-all=false".to_string()));
    }

    #[test]
    fn ca_certificate_candidates_include_platform_defaults() {
        let candidates = ca_certificate_candidates();

        if cfg!(target_os = "macos") {
            assert_eq!(candidates.first(), Some(&PathBuf::from("/etc/ssl/cert.pem")));
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
}
