use crate::app::AppState;
use crate::commands::settings::load_app_config_from_pool;
use crate::config::aria2::Aria2Config;
use crate::database::tasks::persist_download_task_states;
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

const TASK_MONITOR_INTERVAL: Duration = Duration::from_secs(5);

pub fn spawn_task_monitor(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TASK_MONITOR_INTERVAL).await;
            let state = app_handle.state::<AppState>();
            if state.is_exiting.load(Ordering::SeqCst) {
                state
                    .debug_logs
                    .info("runtime.monitor", "应用正在退出，停止后台任务状态同步");
                break;
            }
            drop(state);

            if let Err(error) = monitor_tasks_once(&app_handle).await {
                let state = app_handle.state::<AppState>();
                state.debug_logs.warn(
                    "runtime.monitor",
                    format!("后台任务状态同步失败：{}", error),
                );
            }
        }
    });
}

async fn monitor_tasks_once(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let config = Aria2Config::from_env();
    let state = app_handle.state::<AppState>();
    let previous_tasks = snapshot_tasks(&state);
    if !previous_tasks.iter().any(should_monitor_task) {
        return Ok(());
    }
    let previous_statuses = previous_statuses(&previous_tasks);
    let tasks =
        refresh_tasks_from_aria2(&state.download_tasks, &config, Some(&state.debug_logs)).await?;
    persist_download_task_states(&state.database.pool, &tasks).await?;

    let app_config = load_app_config_from_pool(&state.database.pool).await?;
    if !app_config.notifications_enabled {
        return Ok(());
    }

    for task in &tasks {
        maybe_notify_task_transition(app_handle, &state, &previous_statuses, task);
    }

    Ok(())
}

fn snapshot_tasks(state: &AppState) -> Vec<DownloadTask> {
    state
        .download_tasks
        .lock()
        .map(|tasks| tasks.clone())
        .unwrap_or_default()
}

fn previous_statuses(tasks: &[DownloadTask]) -> HashMap<u64, DownloadTaskStatus> {
    tasks
        .iter()
        .map(|task| (task.id, task.status.clone()))
        .collect()
}

fn should_monitor_task(task: &DownloadTask) -> bool {
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

fn maybe_notify_task_transition(
    app_handle: &tauri::AppHandle,
    state: &AppState,
    previous_statuses: &HashMap<u64, DownloadTaskStatus>,
    task: &DownloadTask,
) {
    if !should_notify_transition(previous_statuses.get(&task.id), &task.status) {
        return;
    }
    if !reserve_notification_key(&state.notified_task_events, notification_key(task)) {
        return;
    }

    let (title, body) = task_notification_text(task);
    match app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
    {
        Ok(()) => state.debug_logs.info(
            "runtime.notification",
            format!(
                "已发送任务通知，ID {}，状态 {}",
                task.id,
                task.status.as_storage_value()
            ),
        ),
        Err(error) => state.debug_logs.warn(
            "runtime.notification",
            format!("发送任务通知失败，ID {}：{}", task.id, error),
        ),
    }
}

fn should_notify_transition(
    previous_status: Option<&DownloadTaskStatus>,
    next_status: &DownloadTaskStatus,
) -> bool {
    if !matches!(
        next_status,
        DownloadTaskStatus::Complete | DownloadTaskStatus::Error
    ) {
        return false;
    }

    previous_status
        .map(|status| status != next_status)
        .unwrap_or(false)
}

fn reserve_notification_key(
    notified_task_events: &std::sync::Mutex<HashSet<String>>,
    key: String,
) -> bool {
    notified_task_events
        .lock()
        .map(|mut events| events.insert(key))
        .unwrap_or(false)
}

fn notification_key(task: &DownloadTask) -> String {
    format!("{}:{}", task.id, task.status.as_storage_value())
}

fn task_notification_text(task: &DownloadTask) -> (&'static str, String) {
    match task.status {
        DownloadTaskStatus::Complete => ("下载完成", format!("{} 已下载完成", task.file_name)),
        DownloadTaskStatus::Error => (
            "下载失败",
            format!(
                "{} 下载失败：{}",
                task.file_name,
                task.error_message.as_deref().unwrap_or("未知错误")
            ),
        ),
        _ => ("下载任务状态更新", task.file_name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn task(status: DownloadTaskStatus) -> DownloadTask {
        DownloadTask {
            id: 1,
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            gid: Some("abc123".to_string()),
            status,
            total_length: 100,
            completed_length: 100,
            download_speed: 0,
            error_code: None,
            error_message: None,
            file_path: None,
            created_at: 1,
            updated_at: 2,
        }
    }

    #[test]
    fn notify_transition_only_for_new_terminal_status() {
        assert!(should_notify_transition(
            Some(&DownloadTaskStatus::Active),
            &DownloadTaskStatus::Complete
        ));
        assert!(should_notify_transition(
            Some(&DownloadTaskStatus::Active),
            &DownloadTaskStatus::Error
        ));
        assert!(!should_notify_transition(
            Some(&DownloadTaskStatus::Complete),
            &DownloadTaskStatus::Complete
        ));
        assert!(!should_notify_transition(
            Some(&DownloadTaskStatus::Active),
            &DownloadTaskStatus::Paused
        ));
    }

    #[test]
    fn notification_key_is_reserved_once() {
        let events = Mutex::new(HashSet::new());
        let task = task(DownloadTaskStatus::Complete);
        let key = notification_key(&task);

        assert!(reserve_notification_key(&events, key.clone()));
        assert!(!reserve_notification_key(&events, key));
    }
}
