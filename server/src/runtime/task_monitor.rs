use crate::app::{HttpAppState, RuntimeEvent, TasksSnapshotPayload};
use crate::database::tasks::persist_download_task_states;
use crate::runtime::ensure_aria2_ready;
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::sync::Arc;
use std::time::Duration;

const TASK_MONITOR_INTERVAL: Duration = Duration::from_millis(500);

pub fn spawn_task_monitor(state: Arc<HttpAppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(TASK_MONITOR_INTERVAL).await;
            if state.core.shutdown.is_exiting() {
                state
                    .core
                    .debug_logs
                    .info("runtime.monitor", "服务正在退出，停止后台任务状态同步");
                break;
            }

            if let Err(error) = monitor_tasks_once(&state).await {
                state.core.debug_logs.warn(
                    "runtime.monitor",
                    format!("后台任务状态同步失败：{}", error),
                );
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
        &config,
        Some(&state.core.debug_logs),
    )
    .await?;
    persist_download_task_states(&state.core.database.pool, &tasks).await?;
    let next_tasks = visible_tasks(tasks);
    if next_tasks != previous_tasks {
        let _ = state
            .runtime_events
            .send(RuntimeEvent::TasksSnapshot(TasksSnapshotPayload {
                tasks: next_tasks,
            }))?;
    }
    Ok(())
}

pub fn visible_tasks_snapshot(state: &HttpAppState) -> Result<Vec<DownloadTask>, String> {
    crate::tasks::list_tasks(&state.core.download_tasks).map(visible_tasks)
}

pub fn broadcast_tasks_snapshot(state: &HttpAppState) -> Result<(), String> {
    let tasks = visible_tasks_snapshot(state)?;
    let _ = state
        .runtime_events
        .send(RuntimeEvent::TasksSnapshot(TasksSnapshotPayload { tasks }))?;
    Ok(())
}

fn should_monitor_task(task: &DownloadTask) -> bool {
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
