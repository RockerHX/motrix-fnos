use super::resolve::resolve_aria2_binary;
use super::status::process_status;
use super::stop::stop_process;
use super::types::{Aria2ProcessStatus, ManagedAria2Process};
use crate::app::{HttpAppState, ServerRuntimeConfig};
use crate::aria2::{
    generate_rpc_secret, ping_rpc, process_args, rpc_ports_exhausted_message, runtime_config,
    select_rpc_port_with_saved_runtime, summarize_args, SavedAria2Runtime,
};
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::database::tasks::persist_download_task_states;
use crate::debug_logs::{emit_file_log, DebugLogLevel, DebugLogStore};
use crate::runtime::Aria2Lease;
use crate::state::Aria2RuntimeInfo;
use crate::tasks::{
    reconcile_session_task_proxies, sync_session_tasks_from_aria2, DownloadTaskStatus,
};
use std::net::{TcpStream, ToSocketAddrs};
use std::ops::Deref;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

pub fn start_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(child) = guard.as_mut() {
        let pid = child.id();
        let source = child.source();
        if child.is_running()? {
            debug_logs.info("aria2", format!("Aria2 进程已在运行，PID {}", pid));
            return Ok(Aria2ProcessStatus {
                running: true,
                pid: Some(pid),
                binary_source: Some(source),
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
    let resolved = resolve_aria2_binary(runtime, config)?;
    let child = Command::new(&resolved.path)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("启动 Aria2 Next 失败：{}", error))?;
    let pid = child.id();
    *guard = Some(ManagedAria2Process::new(child, resolved.source.clone()));
    debug_logs.info(
        "aria2",
        format!(
            "Aria2 进程启动成功，来源 {}，PID {}",
            source_label(&resolved.source),
            pid
        ),
    );

    Ok(Aria2ProcessStatus {
        running: true,
        pid: Some(pid),
        binary_source: Some(resolved.source.clone()),
        message: format!("Aria2 进程启动成功（{}）", source_label(&resolved.source)),
    })
}

pub struct ReadyAria2 {
    config: Aria2Config,
    _activity: Aria2Lease,
}

impl Deref for ReadyAria2 {
    type Target = Aria2Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

pub async fn ensure_aria2_ready(state: &HttpAppState) -> Result<ReadyAria2, String> {
    let activity = state.aria2_lifecycle.acquire_activity()?;
    let _operation = state
        .aria2_lifecycle
        .lock_lifecycle_operation_for_request()
        .await?;
    let config = ensure_aria2_ready_locked(state).await?;
    Ok(ReadyAria2 {
        config,
        _activity: activity,
    })
}

async fn ensure_aria2_ready_locked(state: &HttpAppState) -> Result<Aria2Config, String> {
    let process = process_status(&state.aria2_process)?;
    if process.running {
        let Some(runtime) = state.aria2_runtime_snapshot() else {
            return Err(lifecycle_error(
                state,
                "Aria2 进程已运行但运行态未记录，拒绝继续使用未知配置".to_string(),
            ));
        };
        if process.pid != Some(runtime.pid) {
            return Err(lifecycle_error(
                state,
                format!(
                    "Aria2 进程 PID {} 与运行态 PID {} 不一致",
                    process.pid.unwrap_or_default(),
                    runtime.pid
                ),
            ));
        }

        if state.aria2_lifecycle.snapshot()?.phase == crate::runtime::Aria2LifecyclePhase::Ready {
            return Ok(state.aria2_config());
        }
    }

    if let Err(error) = state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Starting)
    {
        return Err(lifecycle_error(state, error));
    }

    let mut started_process = false;
    if !process.running {
        state
            .core
            .debug_logs
            .info("aria2", "Aria2 进程未运行，准备自动启动");
        started_process = true;
        let base = state.base_aria2_config.clone();
        let saved_runtime = state.load_saved_aria2_runtime();
        let saved_runtime = saved_runtime.as_ref().map(saved_runtime_info);
        let port = match select_rpc_port_with_saved_runtime(
            &base,
            saved_runtime.as_ref(),
            &state.core.debug_logs,
        ) {
            Some(port) => port,
            None => return Err(lifecycle_error(state, rpc_ports_exhausted_message())),
        };
        let config = match state.with_aria2_runtime_paths(runtime_config(
            &base,
            port,
            generate_rpc_secret(),
        )) {
            Ok(config) => config,
            Err(error) => return Err(lifecycle_error(state, error)),
        };
        let status = match start_process(
            &state.aria2_process,
            &state.runtime,
            &config,
            &state.core.debug_logs,
        ) {
            Ok(status) => status,
            Err(error) => {
                return Err(lifecycle_error(
                    state,
                    format!("启动 Aria2 Next 失败：{}", shorten_start_error(error)),
                ));
            }
        };
        let (Some(pid), Some(source)) = (status.pid, status.binary_source.clone()) else {
            return Err(lifecycle_error(
                state,
                "Aria2 启动成功但未返回有效进程身份".to_string(),
            ));
        };
        let runtime = state.build_aria2_runtime_info(pid, &config, source, process_args(&config));
        if let Err(error) = state.set_aria2_runtime(runtime) {
            let stop_error = stop_process(&state.aria2_process, &state.core.debug_logs).err();
            state.clear_aria2_runtime();
            let message = match stop_error {
                Some(stop_error) => format!(
                    "写入 Aria2 运行态失败：{}；回收进程失败：{}",
                    error, stop_error
                ),
                None => format!("写入 Aria2 运行态失败：{}", error),
            };
            return Err(lifecycle_error(state, message));
        }
    }

    let config = state.aria2_config();
    if let Err(error) = wait_for_rpc_ready(
        &state.aria2_rpc,
        &config,
        &state.core.debug_logs,
        started_process,
    )
    .await
    {
        let status = match process_status(&state.aria2_process) {
            Ok(status) => status,
            Err(status_error) => return Err(lifecycle_error(state, status_error)),
        };
        if !status.running {
            state.clear_aria2_runtime();
            state.core.debug_logs.error(
                "aria2",
                format!("Aria2 进程已退出，RPC 无法就绪：{}", status.message),
            );
            return Err(lifecycle_error(
                state,
                format!(
                    "Aria2 Next 启动后已退出，RPC 未就绪，请查看 Aria2 日志（{}）",
                    normalize_rpc_error(&error)
                ),
            ));
        }
        return Err(lifecycle_error(state, error));
    }
    if started_process && has_session_restore_candidates(state)? {
        let _proxy_update_guard = state.download_proxy_update_lock.lock().await;
        let tasks = sync_session_tasks_from_aria2(
            &state.core.download_tasks,
            &state.aria2_rpc,
            &config,
            Some(&state.core.debug_logs),
        )
        .await
        .map_err(|error| lifecycle_error(state, error))?;
        persist_download_task_states(&state.core.database.pool, &tasks)
            .await
            .map_err(|error| lifecycle_error(state, error))?;
        reconcile_session_task_proxies(
            &state.core.download_tasks,
            &state.aria2_rpc,
            &config,
            Some(&state.core.debug_logs),
        )
        .await
        .map_err(|error| lifecycle_error(state, error))?;
    }
    if let Err(error) = state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
    {
        return Err(lifecycle_error(state, error));
    }
    Ok(config)
}

fn has_session_restore_candidates(state: &HttpAppState) -> Result<bool, String> {
    Ok(state.core.download_tasks.list()?.iter().any(|task| {
        !matches!(
            task.status,
            DownloadTaskStatus::Complete | DownloadTaskStatus::Removed
        )
    }))
}

pub async fn start_aria2(state: &HttpAppState) -> Result<Aria2ProcessStatus, String> {
    let _activity = state.aria2_lifecycle.acquire_activity()?;
    let _operation = state
        .aria2_lifecycle
        .lock_lifecycle_operation_for_request()
        .await?;
    let _config = ensure_aria2_ready_locked(state).await?;
    let status = process_status(&state.aria2_process)?;
    if !status.running {
        state.clear_aria2_runtime();
        return Err(lifecycle_error(
            state,
            format!("Aria2 RPC ready 后进程已退出：{}", status.message),
        ));
    }
    Ok(status)
}

fn lifecycle_error(state: &HttpAppState, error: String) -> String {
    let _ = state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Faulted);
    error
}

pub(crate) async fn wait_for_rpc_ready(
    client: &crate::aria2::Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: &DebugLogStore,
    log_success_to_debug: bool,
) -> Result<(), String> {
    // 子进程成功 spawn 不代表 RPC 已监听；固定次数重试用于覆盖 Aria2 初始化 session 和磁盘状态的时间窗口。
    const MAX_ATTEMPTS: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 300;

    let mut last_message = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        let status = ping_rpc(client, config, None).await;
        if status.connected {
            let message = format!("Aria2 RPC ready，第 {} 次检查成功", attempt + 1);
            if log_success_to_debug {
                debug_logs.info("aria2.rpc", message);
            } else {
                emit_file_log(DebugLogLevel::Info, "aria2.rpc", &message);
            }
            return Ok(());
        }

        last_message = status.message;
        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    if last_message.is_empty() {
        let error = "Aria2 Next 已启动但 RPC 未就绪，请稍后重试".to_string();
        debug_logs.error("aria2.rpc", &error);
        Err(error)
    } else {
        let error = format!(
            "Aria2 Next 已启动但 RPC 未就绪，请稍后重试（{}）",
            normalize_rpc_error(&last_message)
        );
        debug_logs.error("aria2.rpc", format!("RPC ready timeout：{}", error));
        Err(error)
    }
}

fn normalize_rpc_error(message: &str) -> String {
    if message.contains("error sending request")
        || message.contains("Connection refused")
        || message.contains("连接失败")
    {
        return "无法连接本地 RPC".to_string();
    }

    message.to_string()
}

fn shorten_start_error(message: String) -> String {
    if message.contains("permission") || message.contains("Permission") {
        return "内置 Aria2 Next 没有执行权限".to_string();
    }

    message
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

fn saved_runtime_info(runtime: &Aria2RuntimeInfo) -> SavedAria2Runtime {
    SavedAria2Runtime {
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
