use crate::app::{HttpAppState, RuntimeEvent, TasksSnapshotPayload};
use crate::database::tasks::persist_download_task_states;
use crate::runtime::{
    auto_stop_aria2, current_activity_snapshot, ensure_aria2_ready, Aria2LifecyclePhase,
};
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TASK_MONITOR_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
struct IdleStopDebouncer {
    idle_since: Option<Instant>,
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
}

pub fn spawn_task_monitor(state: Arc<HttpAppState>) {
    tokio::spawn(async move {
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

            if let Err(error) = monitor_task_tick(&state, &mut idle_stop).await {
                state.core.debug_logs.error(
                    "runtime.monitor",
                    format!("后台监控生命周期状态不可用，停止本轮监控：{}", error),
                );
                break;
            }
        }
    });
}

async fn monitor_task_tick(
    state: &Arc<HttpAppState>,
    idle_stop: &mut IdleStopDebouncer,
) -> Result<(), String> {
    let lifecycle = state.aria2_lifecycle.snapshot()?;
    if lifecycle.retry_after.is_some() {
        idle_stop.reset();
        return Ok(());
    }

    let result = match monitor_tasks_once(state).await {
        Ok(()) => maybe_auto_stop(state, idle_stop, lifecycle.consecutive_failures > 0).await,
        Err(error) => Err(error),
    };

    match result {
        Ok(()) => record_monitor_recovery(state)?,
        Err(error) => {
            idle_stop.reset();
            record_monitor_failure(state, error)?;
        }
    }

    Ok(())
}

fn record_monitor_recovery(state: &HttpAppState) -> Result<(), String> {
    if state.aria2_lifecycle.snapshot()?.consecutive_failures == 0 {
        return Ok(());
    }
    state.aria2_lifecycle.clear_failure()?;
    state
        .core
        .debug_logs
        .info("runtime.monitor", "Aria2 后台监控已恢复正常");
    Ok(())
}

fn record_monitor_failure(state: &HttpAppState, error: String) -> Result<(), String> {
    state.aria2_lifecycle.record_failure(error.clone())?;
    let snapshot = state.aria2_lifecycle.snapshot()?;
    let retry_after = snapshot
        .retry_after
        .map(|duration| format!("{} 秒后", duration.as_secs()))
        .unwrap_or_else(|| "稍后".to_string());
    state.core.debug_logs.warn(
        "runtime.monitor",
        format!(
            "Aria2 后台监控失败（第 {} 次），{}重试：{}",
            snapshot.consecutive_failures, retry_after, error
        ),
    );
    Ok(())
}

async fn maybe_auto_stop(
    state: &Arc<HttpAppState>,
    idle_stop: &mut IdleStopDebouncer,
    retrying_after_failure: bool,
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
    ) {
        idle_stop.reset();
        return Ok(());
    }

    let activity = current_activity_snapshot(state, None).await?;
    if !activity.is_idle() {
        idle_stop.reset();
        return Ok(());
    }
    if !retrying_after_failure && !idle_stop.observe(activity, Instant::now(), policy.idle_debounce)
    {
        return Ok(());
    }

    // 防抖窗口结束后重新读取一次内存和生命周期状态，避免把短暂空闲误判为可停止。
    idle_stop.reset();
    if !current_activity_snapshot(state, None).await?.is_idle() {
        return Ok(());
    }

    let status = auto_stop_aria2(state).await?;
    state.core.debug_logs.info(
        "runtime.auto_stop",
        format!("Aria2 空闲自动停止完成：{}", status.message),
    );
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
    let tasks = visible_tasks_snapshot(state)?
        .into_iter()
        .map(Into::into)
        .collect();
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
