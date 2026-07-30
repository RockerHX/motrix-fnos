use super::types::{Aria2ProcessStatus, ManagedAria2Process};
use crate::app::HttpAppState;
use crate::aria2::{save_session, terminate_process};
use crate::debug_logs::DebugLogStore;
use crate::runtime::{Aria2ActivitySignals, Aria2ActivitySnapshot, Aria2LifecyclePhase};
use crate::tasks::list_tasks;
use std::sync::Mutex;
use std::time::Duration;

pub fn stop_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    stop_process_with_timeout(process, debug_logs, Duration::from_secs(2))
}

pub fn stop_process_with_timeout(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: &DebugLogStore,
    process_exit_timeout: Duration,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(mut child) = guard.take() {
        let pid = child.id();
        if !child.is_running()? {
            debug_logs.warn(
                "aria2",
                format!("停止 Aria2 进程：PID {} 已不存在，清理本地句柄", pid),
            );
        } else {
            debug_logs.info("aria2", format!("准备停止 Aria2 进程，PID {}", pid));
            if let Err(error) = child.kill() {
                debug_logs.warn(
                    "aria2",
                    format!("{}，尝试按 PID 兜底终止，PID {}", error, pid),
                );
            }
            child.wait();
            if !wait_until_process_exits(pid, process_exit_timeout) && !terminate_process(pid) {
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

pub async fn stop_aria2(state: &HttpAppState) -> Result<Aria2ProcessStatus, String> {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    let snapshot = state.aria2_lifecycle.snapshot()?;
    if snapshot.in_flight_requests > 0 {
        return Err(format!(
            "Aria2 仍有 {} 个在途 RPC 请求，暂不能停止",
            snapshot.in_flight_requests
        ));
    }
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Stopping)?;

    match stop_process_with_timeout(
        &state.aria2_process,
        &state.core.debug_logs,
        state.aria2_lifecycle.policy().process_exit_timeout,
    ) {
        Ok(status) => {
            state.clear_aria2_runtime();
            state
                .aria2_lifecycle
                .set_phase(crate::runtime::Aria2LifecyclePhase::Stopped)?;
            Ok(status)
        }
        Err(error) => {
            let _ = state
                .aria2_lifecycle
                .set_phase(crate::runtime::Aria2LifecyclePhase::Faulted);
            Err(error)
        }
    }
}

pub async fn auto_stop_aria2(state: &HttpAppState) -> Result<Aria2ProcessStatus, String> {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    ensure_auto_stop_idle(state)?;

    if state.aria2_runtime_snapshot().is_none() {
        return Err("Aria2 运行态不存在，跳过自动停止".to_string());
    }

    let config = state.aria2_config();
    save_session(&state.aria2_rpc, &config, Some(&state.core.debug_logs))
        .await
        .map_err(|error| format!("自动停止前保存 Aria2 session 失败：{}", error))?;

    ensure_auto_stop_idle(state)?;
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Stopping)?;

    match stop_process_with_timeout(
        &state.aria2_process,
        &state.core.debug_logs,
        state.aria2_lifecycle.policy().process_exit_timeout,
    ) {
        Ok(status) => {
            state.clear_aria2_runtime();
            state
                .aria2_lifecycle
                .set_phase(Aria2LifecyclePhase::Stopped)?;
            Ok(status)
        }
        Err(error) => {
            let _ = state
                .aria2_lifecycle
                .set_phase(Aria2LifecyclePhase::Faulted);
            Err(error)
        }
    }
}

fn ensure_auto_stop_idle(state: &HttpAppState) -> Result<(), String> {
    let coordinator = state.aria2_lifecycle.snapshot()?;
    if !matches!(
        coordinator.phase,
        Aria2LifecyclePhase::Ready | Aria2LifecyclePhase::Faulted
    ) {
        return Err(format!(
            "Aria2 当前处于 {:?} 阶段，暂不能自动停止",
            coordinator.phase
        ));
    }
    if coordinator.active_leases > 0 || coordinator.in_flight_requests > 0 {
        return Err(format!(
            "Aria2 仍有在途生命周期操作（租约 {}，RPC {}），暂不能自动停止",
            coordinator.active_leases, coordinator.in_flight_requests
        ));
    }

    let activity = current_activity_snapshot(state)?;
    if !activity.is_idle() {
        return Err("Aria2 仍有活动、在途操作或人工处理状态，暂不能自动停止".to_string());
    }
    Ok(())
}

pub(crate) fn current_activity_snapshot(
    state: &HttpAppState,
) -> Result<Aria2ActivitySnapshot, String> {
    let tasks = list_tasks(&state.core.download_tasks)?;
    let active_operation_count = state.core.download_tasks.active_operation_count()?;
    Ok(Aria2ActivitySnapshot::from_tasks(
        &tasks,
        Aria2ActivitySignals {
            has_inflight_operation: active_operation_count > 0,
            ..Aria2ActivitySignals::default()
        },
    ))
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
