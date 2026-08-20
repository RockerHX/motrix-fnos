use crate::app::HttpAppState;
use crate::aria2::{
    change_global_log_level, Aria2LogLevel, Aria2LogModeChange, Aria2LogModeWorkerAction,
};
use crate::runtime::{process_status, Aria2LifecyclePhase};
use std::sync::Arc;
use std::time::Duration;

const RESTORE_RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub(crate) enum Aria2LogModeUpdateError {
    Conflict(String),
    Failed(String),
    OutcomeUnknown(String),
}

pub(crate) async fn update_aria2_log_mode(
    state: &Arc<HttpAppState>,
    detailed: bool,
) -> Result<(), Aria2LogModeUpdateError> {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    let phase = state
        .aria2_lifecycle
        .snapshot()
        .map_err(Aria2LogModeUpdateError::Failed)?
        .phase;
    if !matches!(
        phase,
        Aria2LifecyclePhase::Stopped | Aria2LifecyclePhase::Ready | Aria2LifecyclePhase::Faulted
    ) {
        return Err(Aria2LogModeUpdateError::Conflict(format!(
            "Aria2 当前处于 {:?} 阶段，暂不能切换日志模式",
            phase
        )));
    }

    if detailed {
        match apply_log_level_if_running(state, Aria2LogLevel::Debug).await {
            Ok(ApplyLogLevelResult::Applied | ApplyLogLevelResult::NotRunning) => {
                state.aria2_log_mode.enable_detailed();
            }
            Err(Aria2LogModeUpdateError::OutcomeUnknown(error)) => {
                state.aria2_log_mode.disable_detailed();
                drop(_operation);
                spawn_aria2_log_mode_worker(Arc::clone(state));
                return Err(Aria2LogModeUpdateError::OutcomeUnknown(error));
            }
            Err(error) => return Err(error),
        }
    } else {
        let change = state.aria2_log_mode.disable_detailed();
        match apply_log_level_if_running(state, Aria2LogLevel::Warn).await {
            Ok(ApplyLogLevelResult::Applied | ApplyLogLevelResult::NotRunning) => {
                state.aria2_log_mode.mark_applied(change);
            }
            Err(error) => {
                drop(_operation);
                spawn_aria2_log_mode_worker(Arc::clone(state));
                return Err(error);
            }
        }
    }

    drop(_operation);
    spawn_aria2_log_mode_worker(Arc::clone(state));
    Ok(())
}

pub(crate) fn spawn_aria2_log_mode_worker(state: Arc<HttpAppState>) {
    if !state.aria2_log_mode.try_start_worker() {
        return;
    }

    tokio::spawn(async move {
        run_aria2_log_mode_worker(state).await;
    });
}

async fn run_aria2_log_mode_worker(state: Arc<HttpAppState>) {
    loop {
        let action = state.aria2_log_mode.worker_action();
        let Some(action) = action else {
            state.aria2_log_mode.finish_worker();
            if state.aria2_log_mode.worker_action().is_some()
                && state.aria2_log_mode.try_start_worker()
            {
                continue;
            }
            return;
        };

        match action {
            Aria2LogModeWorkerAction::WaitUntil(deadline) => {
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => {
                        if let Some(change) = state.aria2_log_mode.expire_if_due() {
                            apply_and_record(&state, change).await;
                        }
                    }
                    _ = state.aria2_log_mode.wait_for_change() => {}
                }
            }
            Aria2LogModeWorkerAction::RetryRestore => {
                tokio::select! {
                    _ = tokio::time::sleep(RESTORE_RETRY_DELAY) => {
                        if let Some(change) = state.aria2_log_mode.pending_restore() {
                            apply_and_record(&state, change).await;
                        }
                    }
                    _ = state.aria2_log_mode.wait_for_change() => {}
                }
            }
        }
    }
}

async fn apply_and_record(state: &Arc<HttpAppState>, change: Aria2LogModeChange) {
    let _operation = state.aria2_lifecycle.lock_lifecycle_operation().await;
    match apply_log_level_if_running(state, change.level()).await {
        Ok(ApplyLogLevelResult::Applied | ApplyLogLevelResult::NotRunning) => {
            state.aria2_log_mode.mark_applied(change);
        }
        Err(
            Aria2LogModeUpdateError::Conflict(error)
            | Aria2LogModeUpdateError::Failed(error)
            | Aria2LogModeUpdateError::OutcomeUnknown(error),
        ) => {
            state.core.debug_logs.warn(
                "aria2.log_mode",
                format!("恢复普通 Aria2 日志失败，将自动重试：{error}"),
            );
        }
    }
}

enum ApplyLogLevelResult {
    Applied,
    NotRunning,
}

async fn apply_log_level_if_running(
    state: &HttpAppState,
    level: Aria2LogLevel,
) -> Result<ApplyLogLevelResult, Aria2LogModeUpdateError> {
    let process = process_status(&state.aria2_process).map_err(Aria2LogModeUpdateError::Failed)?;
    if !process.running {
        return Ok(ApplyLogLevelResult::NotRunning);
    }

    let lifecycle = state
        .aria2_lifecycle
        .snapshot()
        .map_err(Aria2LogModeUpdateError::Failed)?;
    if lifecycle.phase != Aria2LifecyclePhase::Ready {
        return Err(Aria2LogModeUpdateError::Conflict(format!(
            "Aria2 当前处于 {:?} 阶段，暂不能切换日志模式",
            lifecycle.phase
        )));
    }

    let runtime = state.aria2_runtime_snapshot().ok_or_else(|| {
        Aria2LogModeUpdateError::Conflict("Aria2 运行态未记录，拒绝切换日志模式".to_string())
    })?;
    if process.pid != Some(runtime.pid) {
        return Err(Aria2LogModeUpdateError::Conflict(
            "Aria2 进程身份与运行态不一致，拒绝切换日志模式".to_string(),
        ));
    }

    change_global_log_level(&state.aria2_rpc, &state.aria2_config(), level)
        .await
        .map_err(|error| {
            let message = format!("修改 Aria2 日志级别失败：{error}");
            if error.write_outcome_is_unknown() {
                Aria2LogModeUpdateError::OutcomeUnknown(message)
            } else {
                Aria2LogModeUpdateError::Failed(message)
            }
        })?;
    Ok(ApplyLogLevelResult::Applied)
}
