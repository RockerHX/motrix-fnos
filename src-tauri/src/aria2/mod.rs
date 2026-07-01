use crate::app::{Aria2RuntimeInfo, ManagedAria2Process};
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

pub use motrix_fnos_server::aria2::{
    apply_global_options, generate_rpc_secret, global_options_from_values, ping_rpc, process_args,
    rpc_ports_exhausted_message, runtime_config, save_session, summarize_args, Aria2ConfigStatus,
    Aria2GlobalOptions, Aria2RpcStatus,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub binary_source: Option<Aria2BinarySource>,
    pub message: String,
}

pub fn select_rpc_port_with_saved_runtime(
    config: &Aria2Config,
    saved: Option<&Aria2RuntimeInfo>,
    debug_logs: &DebugLogStore,
) -> Option<u16> {
    let saved = saved.map(saved_runtime_info);
    motrix_fnos_server::aria2::select_rpc_port_with_saved_runtime(config, saved.as_ref(), debug_logs)
}

pub fn process_status(
    process: &Mutex<Option<ManagedAria2Process>>,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;

    Ok(match guard.as_ref() {
        Some(child) if managed_process_is_running(child) => Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            binary_source: Some(managed_process_source(child)),
            message: "Aria2 进程已启动".to_string(),
        },
        Some(child) => {
            let pid = child.id();
            let source = managed_process_source(child);
            let _ = guard.take();
            Aria2ProcessStatus {
                running: false,
                pid: Some(pid),
                binary_source: Some(source),
                message: format!("Aria2 进程已退出，PID {}", pid),
            }
        }
        None => Aria2ProcessStatus {
            running: false,
            pid: None,
            binary_source: None,
            message: "Aria2 进程未启动".to_string(),
        },
    })
}

fn managed_process_source(process: &ManagedAria2Process) -> Aria2BinarySource {
    match process {
        ManagedAria2Process::External(_) => Aria2BinarySource::ExternalPath,
        ManagedAria2Process::Sidecar(_) => Aria2BinarySource::Sidecar,
    }
}

fn managed_process_is_running(process: &ManagedAria2Process) -> bool {
    process_is_running(process.id())
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
        let pid = child.id();
        if managed_process_is_running(child) {
            debug_logs.info("aria2", format!("Aria2 进程已在运行，PID {}", pid));
            return Ok(Aria2ProcessStatus {
                running: true,
                pid: Some(pid),
                binary_source: Some(managed_process_source(child)),
                message: "Aria2 进程已在运行".to_string(),
            });
        }

        debug_logs.warn("aria2", format!("清理已退出的 Aria2 进程句柄，PID {}", pid));
        let _ = guard.take();
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
        if !managed_process_is_running(&child) {
            debug_logs.warn(
                "aria2",
                format!("停止 Aria2 进程：PID {} 已不存在，清理本地句柄", pid),
            );
        } else {
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
            if !wait_until_process_exits(pid, Duration::from_millis(800))
                && !terminate_process(pid)
            {
                let error = format!("停止 Aria2 进程后 PID {} 仍然存活", pid);
                debug_logs.error("aria2", &error);
                return Err(error);
            }
            debug_logs.info("aria2", format!("Aria2 进程已停止，PID {}", pid));
        }
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

fn rpc_port_in_use(config: &Aria2Config) -> bool {
    let Ok(addresses) = (config.rpc_host.as_str(), config.rpc_port).to_socket_addrs() else {
        return false;
    };

    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok())
}

fn source_label(source: &Aria2BinarySource) -> &'static str {
    match source {
        Aria2BinarySource::ExternalPath => "外部路径",
        Aria2BinarySource::Sidecar => "内置 sidecar",
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

fn saved_runtime_info(runtime: &Aria2RuntimeInfo) -> motrix_fnos_server::aria2::SavedAria2Runtime {
    motrix_fnos_server::aria2::SavedAria2Runtime {
        pid: runtime.pid,
        actual_port: runtime.actual_port,
        rpc_secret: runtime.rpc_secret.clone(),
        binary_source: runtime.binary_source.clone(),
        sidecar_name: runtime.sidecar_name.clone(),
        app_data_dir: runtime.app_data_dir.clone(),
        aria2_session_path: runtime.aria2_session_path.clone(),
        aria2_log_path: runtime.aria2_log_path.clone(),
    }
}
