use crate::app::{HttpAppState, RuntimeEvent, TasksSnapshotPayload};
use crate::database::tasks::persist_download_task_states;
use crate::runtime::{
    auto_stop_aria2, current_activity_snapshot, ensure_aria2_ready, Aria2LifecyclePhase,
};
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TASK_MONITOR_INTERVAL: Duration = Duration::from_millis(500);
const TASK_MONITOR_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
struct IdleStopDebouncer {
    idle_since: Option<Instant>,
    last_failure: Option<(String, Instant)>,
}

impl IdleStopDebouncer {
    fn observe(
        &mut self,
        activity: crate::runtime::Aria2ActivitySnapshot,
        now: Instant,
        debounce: Duration,
    ) -> bool {
        if !activity.is_idle() {
            self.idle_since = None;
            return false;
        }

        let idle_since = *self.idle_since.get_or_insert(now);
        now.duration_since(idle_since) >= debounce
    }

    fn reset(&mut self) {
        self.idle_since = None;
    }

    fn should_log_failure(&mut self, error: &str, now: Instant) -> bool {
        let should_log = match self.last_failure.as_ref() {
            Some((last_error, logged_at))
                if last_error == error
                    && now.duration_since(*logged_at) < TASK_MONITOR_ERROR_LOG_INTERVAL =>
            {
                false
            }
            _ => true,
        };
        if should_log {
            self.last_failure = Some((error.to_string(), now));
        }
        should_log
    }
}

pub fn spawn_task_monitor(state: Arc<HttpAppState>) {
    tokio::spawn(async move {
        let mut last_error: Option<String> = None;
        let mut last_error_logged_at: Option<Instant> = None;
        let mut idle_stop = IdleStopDebouncer::default();
        loop {
            tokio::time::sleep(TASK_MONITOR_INTERVAL).await;
            if state.core.shutdown.is_exiting() {
                state
                    .core
                    .debug_logs
                    .info("runtime.monitor", "服务正在退出，停止后台任务状态同步");
                break;
            }

            match monitor_tasks_once(&state).await {
                Ok(()) => {
                    if last_error.take().is_some() {
                        last_error_logged_at = None;
                        state
                            .core
                            .debug_logs
                            .info("runtime.monitor", "后台任务状态同步已恢复正常");
                    }
                    if let Err(error) = maybe_auto_stop(&state, &mut idle_stop).await {
                        idle_stop.reset();
                        state.core.debug_logs.warn(
                            "runtime.auto_stop",
                            format!("Aria2 自动停止未完成：{}", error),
                        );
                    }
                }
                Err(error) => {
                    idle_stop.reset();
                    let should_log = last_error.as_deref() != Some(error.as_str())
                        || last_error_logged_at
                            .map(|logged_at| logged_at.elapsed() >= TASK_MONITOR_ERROR_LOG_INTERVAL)
                            .unwrap_or(true);
                    if should_log {
                        last_error_logged_at = Some(Instant::now());
                        state.core.debug_logs.warn(
                            "runtime.monitor",
                            format!("后台任务状态同步失败：{}", error),
                        );
                    }
                    last_error = Some(error);
                }
            }
        }
    });
}

async fn maybe_auto_stop(
    state: &Arc<HttpAppState>,
    idle_stop: &mut IdleStopDebouncer,
) -> Result<(), String> {
    let policy = state.aria2_lifecycle.policy();
    if !policy.auto_stop_enabled || state.aria2_runtime_snapshot().is_none() {
        idle_stop.reset();
        return Ok(());
    }

    let lifecycle = state.aria2_lifecycle.snapshot()?;
    if !matches!(
        lifecycle.phase,
        Aria2LifecyclePhase::Ready | Aria2LifecyclePhase::Faulted
    ) || lifecycle.retry_after.is_some()
    {
        idle_stop.reset();
        return Ok(());
    }

    let activity = current_activity_snapshot(state).await?;
    if !idle_stop.observe(activity, Instant::now(), policy.idle_debounce) {
        return Ok(());
    }

    // 防抖窗口结束后重新读取一次内存和生命周期状态，避免把短暂空闲误判为可停止。
    idle_stop.reset();
    if !current_activity_snapshot(state).await?.is_idle() {
        return Ok(());
    }

    match auto_stop_aria2(state).await {
        Ok(status) => {
            state.aria2_lifecycle.clear_failure()?;
            state.core.debug_logs.info(
                "runtime.auto_stop",
                format!("Aria2 空闲自动停止完成：{}", status.message),
            );
        }
        Err(error) => {
            state.aria2_lifecycle.record_failure(error.clone())?;
            let snapshot = state.aria2_lifecycle.snapshot()?;
            if idle_stop.should_log_failure(&error, Instant::now()) {
                let retry_after = snapshot
                    .retry_after
                    .map(|duration| format!("{} 秒后", duration.as_secs()))
                    .unwrap_or_else(|| "稍后".to_string());
                state.core.debug_logs.warn(
                    "runtime.auto_stop",
                    format!(
                        "Aria2 自动停止失败（第 {} 次），{}重试：{}",
                        snapshot.consecutive_failures, retry_after, error
                    ),
                );
            }
        }
    }
    Ok(())
}

pub async fn monitor_tasks_once(state: &Arc<HttpAppState>) -> Result<(), String> {
    let previous_tasks = visible_tasks_snapshot(state)?;
    if !previous_tasks.iter().any(should_monitor_task) {
        return Ok(());
    }

    let config = ensure_aria2_ready(state).await?;
    let tasks = refresh_tasks_from_aria2(
        &state.core.download_tasks,
        &state.runtime.app_data_dir,
        &state.aria2_rpc,
        &config,
        Some(&state.core.debug_logs),
    )
    .await?;
    persist_download_task_states(&state.core.database.pool, &tasks).await?;
    let next_tasks = visible_tasks(tasks);
    if next_tasks != previous_tasks {
        broadcast_tasks_snapshot(state)?;
    }
    Ok(())
}

pub fn visible_tasks_snapshot(state: &HttpAppState) -> Result<Vec<DownloadTask>, String> {
    crate::tasks::list_tasks(&state.core.download_tasks).map(visible_tasks)
}

pub(crate) fn current_tasks_snapshot(state: &HttpAppState) -> Result<TasksSnapshotPayload, String> {
    tasks_snapshot(state, false)
}

pub fn broadcast_tasks_snapshot(state: &HttpAppState) -> Result<(), String> {
    let snapshot = tasks_snapshot(state, true)?;
    let _ = state
        .runtime_events
        .send(RuntimeEvent::TasksSnapshot(snapshot))?;
    Ok(())
}

fn tasks_snapshot(
    state: &HttpAppState,
    advance_revision: bool,
) -> Result<TasksSnapshotPayload, String> {
    let mut revision = state
        .tasks_snapshot_revision
        .lock()
        .map_err(|_| "无法读取任务快照版本".to_string())?;
    let tasks = visible_tasks_snapshot(state)?;
    if advance_revision {
        *revision = revision
            .checked_add(1)
            .ok_or_else(|| "任务快照版本已耗尽".to_string())?;
    }
    Ok(TasksSnapshotPayload {
        revision: *revision,
        tasks,
    })
}

fn should_monitor_task(task: &DownloadTask) -> bool {
    let has_gid = task
        .gid
        .as_deref()
        .map(|gid| !gid.trim().is_empty())
        .unwrap_or(false);

    if !has_gid {
        return false;
    }

    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

fn visible_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .collect()
}

#[cfg(test)]
mod tests;
