use crate::app::{HttpAppState, RuntimeEvent, TasksSnapshotPayload};
use crate::database::tasks::persist_download_task_states;
use crate::runtime::ensure_aria2_ready;
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::sync::Arc;
use std::time::{Duration, Instant};

const TASK_MONITOR_INTERVAL: Duration = Duration::from_millis(500);
const TASK_MONITOR_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(10);

pub fn spawn_task_monitor(state: Arc<HttpAppState>) {
    tokio::spawn(async move {
        let mut last_error: Option<String> = None;
        let mut last_error_logged_at: Option<Instant> = None;
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
                }
                Err(error) => {
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
