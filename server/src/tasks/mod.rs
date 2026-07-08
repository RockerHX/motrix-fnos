pub mod aria2_rpc;
pub mod files;
pub mod model;
pub mod options;
pub mod prepare;
pub mod progress;
pub mod repository;
pub mod service;
pub mod session;
pub mod state;

use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use aria2_rpc::tell_status;
pub use aria2_rpc::{
    add_torrent_to_aria2, add_uri_to_aria2, pause_task, remove_task, unpause_task,
};
pub use files::delete_task_files;
pub use model::{
    should_force_pause_task_on_startup, should_pause_task_on_exit, CreateDownloadTaskRequest,
    CreateTaskAdvancedOptions, CreateTorrentDownloadTaskRequest, DownloadTask,
    DownloadTaskSourceType, DownloadTaskStartMode, DownloadTaskStatus, PreparedDownloadTask,
    DEFAULT_TASK_CATEGORY,
};
pub use options::{sanitize_aria2_options, sanitize_create_task_options};
pub use prepare::{
    default_download_dir_string, prepare_task, prepare_task_with_logs,
    prepare_torrent_task_with_logs,
};
use progress::{
    apply_aria2_status, apply_aria2_status_by_gid, is_aria2_status_error, parse_aria2_u64,
};
use serde::Deserialize;
use session::readd_download_task;
pub use session::{readd_task_to_aria2, sync_session_tasks_from_aria2};
use state::{apply_paused_state, apply_readded_gid, should_refresh_task};
pub use state::{
    list_tasks, mark_task_paused, mark_task_paused_by_gid, mark_task_redownloaded,
    mark_task_removed, mark_task_resumed, mark_unfinished_tasks_paused, remove_task_record,
    store_created_task, task_gid, task_snapshot, TaskMemoryState,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Aria2TaskStatus {
    gid: Option<String>,
    status: String,
    total_length: String,
    completed_length: String,
    download_speed: String,
    error_code: Option<String>,
    error_message: Option<String>,
    dir: Option<String>,
    files: Option<Vec<Aria2FileStatus>>,
    followed_by: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2FileStatus {
    path: String,
    #[serde(default)]
    uris: Vec<Aria2UriStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2UriStatus {
    uri: String,
}

pub async fn refresh_tasks_from_aria2(
    tasks: &TaskMemoryState,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let snapshot = list_tasks(tasks)?;
    let candidates: Vec<DownloadTask> = snapshot
        .iter()
        .filter(|task| should_refresh_task(task))
        .filter(|task| {
            task.gid
                .as_deref()
                .map(|gid| !gid.trim().is_empty())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if candidates.is_empty() {
        return Ok(snapshot);
    }

    let client = reqwest::Client::new();
    let mut updates = Vec::new();
    for candidate in candidates {
        let Some(gid) = candidate.gid.clone() else {
            continue;
        };
        match tell_status(&client, config, &gid, debug_logs).await {
            Ok(status) if is_stale_aria2_gid_status(&status) => {
                match readd_download_task(config, &candidate, debug_logs).await {
                    Ok(new_gid) => updates.push(TaskRefreshUpdate::Readded {
                        task_id: candidate.id,
                        old_gid: gid,
                        new_gid,
                    }),
                    Err(error) => updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: task_status_error(error),
                    }),
                }
            }
            Ok(status) => updates.push(TaskRefreshUpdate::Status { gid, status }),
            Err(error) if is_stale_aria2_gid_error(&error) => {
                match readd_download_task(config, &candidate, debug_logs).await {
                    Ok(new_gid) => updates.push(TaskRefreshUpdate::Readded {
                        task_id: candidate.id,
                        old_gid: gid,
                        new_gid,
                    }),
                    Err(error) => updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: task_status_error(error),
                    }),
                }
            }
            Err(error) => updates.push(TaskRefreshUpdate::Status {
                gid,
                status: task_status_error(error),
            }),
        }
    }

    let mut guard = tasks.with_tasks_mut(|tasks| {
        for update in &updates {
            match update {
                TaskRefreshUpdate::Status { gid, status } => {
                    for task in tasks
                        .iter_mut()
                        .filter(|task| task.gid.as_ref() == Some(gid))
                    {
                        apply_aria2_status(task, status);
                    }
                }
                TaskRefreshUpdate::Readded {
                    task_id,
                    old_gid,
                    new_gid,
                } => {
                    if let Some(task) = tasks
                        .iter_mut()
                        .find(|task| task.id == *task_id && task.gid.as_ref() == Some(old_gid))
                    {
                        apply_readded_gid(task, new_gid);
                    }
                }
            }
        }

        tasks.clone()
    })?;

    Ok(std::mem::take(&mut guard))
}

pub async fn sync_task_progress_from_aria2_by_gid(
    tasks: &TaskMemoryState,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    let client = reqwest::Client::new();
    let status = tell_status(&client, config, gid, debug_logs).await?;
    apply_aria2_status_by_gid(tasks, gid, &status)
}

pub async fn sync_task_progress_after_pause_by_gid(
    tasks: &TaskMemoryState,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    const MAX_ATTEMPTS: usize = 8;
    const RETRY_INTERVAL_MS: u64 = 150;

    let client = reqwest::Client::new();
    let mut previous_completed = None;
    let mut latest_status = None;

    for attempt in 0..MAX_ATTEMPTS {
        let status = tell_status(&client, config, gid, debug_logs).await?;
        let completed = parse_aria2_u64(&status.completed_length);
        let settled = pause_status_is_settled(&status, previous_completed);
        previous_completed = Some(completed);
        latest_status = Some(status);

        if settled {
            break;
        }

        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    let status =
        latest_status.ok_or_else(|| "暂停后同步 Aria2 任务状态失败：未获取到状态".to_string())?;
    if !matches!(status.status.as_str(), "paused" | "complete" | "error") {
        log_info(
            debug_logs,
            "tasks.control",
            format!(
                "暂停后 Aria2 状态尚未稳定，使用最后一次进度，GID {}，状态 {}",
                gid, status.status
            ),
        );
    }
    apply_aria2_status_by_gid(tasks, gid, &status)
}

fn pause_status_is_settled(status: &Aria2TaskStatus, previous_completed: Option<u64>) -> bool {
    matches!(status.status.as_str(), "paused" | "complete" | "error")
        && previous_completed == Some(parse_aria2_u64(&status.completed_length))
}

enum TaskRefreshUpdate {
    Status {
        gid: String,
        status: Aria2TaskStatus,
    },
    Readded {
        task_id: u64,
        old_gid: String,
        new_gid: String,
    },
}

fn task_status_error(message: String) -> Aria2TaskStatus {
    Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: None,
        error_message: Some(message),
        dir: None,
        files: None,
        followed_by: None,
    }
}

fn is_stale_aria2_gid_status(status: &Aria2TaskStatus) -> bool {
    status.status == "error"
        && status
            .error_message
            .as_deref()
            .map(is_stale_aria2_gid_error)
            .unwrap_or(false)
}

pub fn is_stale_aria2_gid_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("no uri available") || normalized.contains("is not found")
}

pub fn should_readd_task_after_resume_error(task: &DownloadTask, message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    is_stale_aria2_gid_error(&normalized)
        || (normalized.contains("cannot be unpaused now")
            && task.status == DownloadTaskStatus::Error)
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn log_info(debug_logs: Option<&DebugLogStore>, module: &str, message: impl Into<String>) {
    if let Some(debug_logs) = debug_logs {
        debug_logs.info(module, message);
    }
}

fn log_error(debug_logs: Option<&DebugLogStore>, module: &str, message: impl Into<String>) {
    if let Some(debug_logs) = debug_logs {
        debug_logs.error(module, message);
    }
}

fn redact_url_for_log(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests;
