use super::status::process_status;
use super::types::{Aria2ProcessStatus, ManagedAria2Process};
use crate::app::HttpAppState;
use crate::aria2::save_session;
use crate::database::tasks::persist_download_task_states;
use crate::debug_logs::DebugLogStore;
use crate::runtime::{Aria2ActivitySignals, Aria2ActivitySnapshot, Aria2LifecyclePhase};
use crate::tasks::{list_tasks, tell_active_task_activity};
use std::fmt;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum Aria2StopError {
    Busy(String),
    Failed(String),
}

impl fmt::Display for Aria2StopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(message) | Self::Failed(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Aria2StopError {}

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
    stop_process_with_timeout_inner(process, Some(debug_logs), process_exit_timeout)
}

pub(super) fn stop_process_with_timeout_inner(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: Option<&DebugLogStore>,
    process_exit_timeout: Duration,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        if let Some(debug_logs) = debug_logs {
            debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        }
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(child) = guard.as_mut() {
        let pid = child.id();
        if child.stop_requested {
            child.interrupt_and_wait(process_exit_timeout)?;
        } else if !child.is_running()? {
            return Err("Aria2 已退出，无法确认本次原生状态提交，已保留运行态".to_string());
        } else {
            if let Some(debug_logs) = debug_logs {
                debug_logs.info("aria2", format!("准备停止 Aria2 进程，PID {}", pid));
            }
            child.interrupt_and_wait(process_exit_timeout)?;
            if let Some(debug_logs) = debug_logs {
                debug_logs.info("aria2", format!("Aria2 进程已停止，PID {}", pid));
            }
        }
        let _ = guard.take();
    } else if let Some(debug_logs) = debug_logs {
        debug_logs.info("aria2", "停止 Aria2 进程：当前没有运行中的进程");
    }

    Ok(Aria2ProcessStatus {
        running: false,
        pid: None,
        binary_source: None,
        message: "Aria2 进程已停止".to_string(),
    })
}

// 调用方须持有生命周期锁，避免收尾与新引擎启动交错。
pub(crate) fn reconcile_pending_native_stop(state: &HttpAppState) -> Result<(), String> {
    let mut process = state
        .aria2_process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;
    let Some(child) = process.as_mut().filter(|child| child.stop_requested) else {
        return Ok(());
    };
    if !child.native_stop_completed()? {
        return Err("Aria2 生命周期请求被拒绝：引擎正在停止并保存原生状态，请稍后重试".to_string());
    }
    state.clear_aria2_runtime()?;
    let pid = child.id();
    process.take();
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Stopped)?;
    state.core.debug_logs.info(
        "aria2",
        format!("已确认 Aria2 延迟正常退出并清理运行态，PID {pid}"),
    );
    Ok(())
}

pub async fn stop_aria2(state: &HttpAppState) -> Result<Aria2ProcessStatus, Aria2StopError> {
    stop_aria2_inner(state, true).await
}

pub(crate) async fn stop_aria2_after_shutdown(
    state: &HttpAppState,
    deadline: Instant,
) -> Result<Aria2ProcessStatus, Aria2StopError> {
    stop_aria2_after_shutdown_until(state, deadline).await
}

async fn stop_aria2_after_shutdown_until(
    state: &HttpAppState,
    deadline: Instant,
) -> Result<Aria2ProcessStatus, Aria2StopError> {
    let _operation =
        tokio::time::timeout_at(deadline, state.aria2_lifecycle.lock_lifecycle_operation())
            .await
            .map_err(|_| {
                Aria2StopError::Failed("退出总预算耗尽，未能取得 Aria2 生命周期锁".to_string())
            })?;
    reconcile_pending_native_stop(state).map_err(Aria2StopError::Failed)?;
    let quiescing = state
        .aria2_lifecycle
        .begin_quiescing()
        .map_err(Aria2StopError::Busy)?;
    if !tokio::time::timeout_at(
        deadline,
        current_activity_snapshot(state, Some(&state.core.debug_logs)),
    )
    .await
    .map_err(|_| Aria2StopError::Failed("退出总预算耗尽，未能确认 Aria2 活动状态".to_string()))?
    .map_err(Aria2StopError::Failed)?
    .is_idle()
    {
        return Err(Aria2StopError::Busy(
            "Aria2 仍有活动或在途操作，暂不能停止".to_string(),
        ));
    }
    if !tokio::time::timeout_at(
        deadline,
        current_activity_snapshot(state, Some(&state.core.debug_logs)),
    )
    .await
    .map_err(|_| Aria2StopError::Failed("退出总预算耗尽，未能再次确认 Aria2 活动状态".to_string()))?
    .map_err(Aria2StopError::Failed)?
    .is_idle()
    {
        return Err(Aria2StopError::Busy(
            "Aria2 仍有活动或在途操作，暂不能停止".to_string(),
        ));
    }
    let permit = state
        .aria2_lifecycle
        .acquire_stop_permit(quiescing)
        .map_err(Aria2StopError::Busy)?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(Aria2StopError::Failed(
            "退出总预算耗尽，未执行 Aria2 进程停止".to_string(),
        ));
    }
    let process_exit_timeout = remaining.min(state.aria2_lifecycle.policy().process_exit_timeout);

    match stop_process_with_timeout_inner(
        &state.aria2_process,
        Some(&state.core.debug_logs),
        process_exit_timeout,
    ) {
        Ok(status) => {
            state
                .clear_aria2_runtime()
                .map_err(Aria2StopError::Failed)?;
            permit
                .complete(Aria2LifecyclePhase::Stopped)
                .map_err(Aria2StopError::Failed)?;
            Ok(status)
        }
        Err(error) => {
            permit
                .complete(Aria2LifecyclePhase::Faulted)
                .map_err(Aria2StopError::Failed)?;
            Err(Aria2StopError::Failed(error))
        }
    }
}

async fn stop_aria2_inner(
    state: &HttpAppState,
    save_session_before_stop: bool,
) -> Result<Aria2ProcessStatus, Aria2StopError> {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    reconcile_pending_native_stop(state).map_err(Aria2StopError::Failed)?;
    let quiescing = state
        .aria2_lifecycle
        .begin_quiescing()
        .map_err(Aria2StopError::Busy)?;
    if !current_activity_snapshot(state, Some(&state.core.debug_logs))
        .await
        .map_err(Aria2StopError::Failed)?
        .is_idle()
    {
        return Err(Aria2StopError::Busy(
            "Aria2 仍有活动或在途操作，暂不能停止".to_string(),
        ));
    }
    if save_session_before_stop && state.aria2_runtime_snapshot().is_some() {
        let tasks = list_tasks(&state.core.download_tasks).map_err(Aria2StopError::Failed)?;
        persist_download_task_states(&state.core.database.pool, &tasks)
            .await
            .map_err(Aria2StopError::Failed)?;
        let config = state.aria2_config();
        save_session(&state.aria2_rpc, &config, Some(&state.core.debug_logs))
            .await
            .map_err(|error| {
                Aria2StopError::Failed(format!("手动停止前保存 Aria2 session 失败：{}", error))
            })?;
    }
    if !current_activity_snapshot(state, Some(&state.core.debug_logs))
        .await
        .map_err(Aria2StopError::Failed)?
        .is_idle()
    {
        return Err(Aria2StopError::Busy(
            "Aria2 仍有活动或在途操作，暂不能停止".to_string(),
        ));
    }
    let permit = state
        .aria2_lifecycle
        .acquire_stop_permit(quiescing)
        .map_err(Aria2StopError::Busy)?;

    match stop_process_with_timeout_inner(
        &state.aria2_process,
        Some(&state.core.debug_logs),
        state.aria2_lifecycle.policy().process_exit_timeout,
    ) {
        Ok(status) => {
            state
                .clear_aria2_runtime()
                .map_err(Aria2StopError::Failed)?;
            permit
                .complete(Aria2LifecyclePhase::Stopped)
                .map_err(Aria2StopError::Failed)?;
            Ok(status)
        }
        Err(error) => {
            permit
                .complete(Aria2LifecyclePhase::Faulted)
                .map_err(Aria2StopError::Failed)?;
            Err(Aria2StopError::Failed(error))
        }
    }
}

pub async fn auto_stop_aria2(state: &HttpAppState) -> Result<Aria2ProcessStatus, String> {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    if state.core.shutdown.is_exiting() {
        return Err("服务正在退出，跳过 Aria2 自动停止".to_string());
    }
    reconcile_pending_native_stop(state)?;
    ensure_auto_stop_idle(state).await?;
    let quiescing = state.aria2_lifecycle.begin_quiescing()?;

    if state.aria2_runtime_snapshot().is_none() {
        return Err("Aria2 运行态不存在，跳过自动停止".to_string());
    }

    let tasks = list_tasks(&state.core.download_tasks)?;
    persist_download_task_states(&state.core.database.pool, &tasks)
        .await
        .map_err(|error| format!("自动停止前持久化任务状态失败：{}", error))?;

    let config = state.aria2_config();
    save_session(&state.aria2_rpc, &config, None)
        .await
        .map_err(|error| format!("自动停止前保存 Aria2 session 失败：{}", error))?;

    ensure_auto_stop_idle(state).await?;
    let permit = state.aria2_lifecycle.acquire_stop_permit(quiescing)?;

    match stop_process_with_timeout_inner(
        &state.aria2_process,
        None,
        state.aria2_lifecycle.policy().process_exit_timeout,
    ) {
        Ok(status) => {
            state.clear_aria2_runtime()?;
            permit.complete(Aria2LifecyclePhase::Stopped)?;
            Ok(status)
        }
        Err(error) => {
            permit.complete(Aria2LifecyclePhase::Faulted)?;
            Err(error)
        }
    }
}

async fn ensure_auto_stop_idle(state: &HttpAppState) -> Result<(), String> {
    let coordinator = state.aria2_lifecycle.snapshot()?;
    if !matches!(
        coordinator.phase,
        Aria2LifecyclePhase::Ready | Aria2LifecyclePhase::Quiescing | Aria2LifecyclePhase::Faulted
    ) {
        return Err(format!(
            "Aria2 当前处于 {:?} 阶段，暂不能自动停止",
            coordinator.phase
        ));
    }
    if coordinator.active_leases > 0
        || coordinator.in_flight_requests > 0
        || coordinator.queued_requests > 0
    {
        return Err(format!(
            "Aria2 仍有在途生命周期操作（租约 {}，RPC {}，排队 {}），暂不能自动停止",
            coordinator.active_leases, coordinator.in_flight_requests, coordinator.queued_requests
        ));
    }

    let activity = current_activity_snapshot(state, None).await?;
    if !activity.is_idle() {
        return Err("Aria2 仍有活动或在途操作，暂不能自动停止".to_string());
    }
    Ok(())
}

pub(crate) async fn current_activity_snapshot(
    state: &HttpAppState,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2ActivitySnapshot, String> {
    let tasks = list_tasks(&state.core.download_tasks)?;
    let active_operation_count = state.core.download_tasks.active_operation_count()?;
    let coordinator = state.aria2_lifecycle.snapshot()?;
    let process = process_status(&state.aria2_process)?;
    if state.aria2_runtime_snapshot().is_some() && !process.running {
        return Err("原生引擎已退出，无法确认状态提交，已保留运行态供恢复核对".to_string());
    }
    let engine_tasks = if process.running && state.aria2_runtime_snapshot().is_some() {
        tell_active_task_activity(&state.aria2_rpc, &state.aria2_config(), debug_logs).await?
    } else {
        Vec::new()
    };
    Ok(Aria2ActivitySnapshot::from_tasks(
        &tasks,
        Aria2ActivitySignals {
            has_engine_activity: !engine_tasks.is_empty(),
            has_bt_upload: engine_tasks.iter().any(|task| task.is_bt_uploading()),
            has_verification_activity: engine_tasks.iter().any(|task| task.is_verifying()),
            has_inflight_operation: active_operation_count > 0,
            has_queued_request: coordinator.queued_requests > 0,
            ..Aria2ActivitySignals::default()
        },
    ))
}

#[cfg(test)]
mod tests;
