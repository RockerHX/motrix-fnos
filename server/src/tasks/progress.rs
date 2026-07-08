use crate::tasks::{DownloadTask, DownloadTaskStatus, TaskMemoryState};
use std::path::Path;

use super::{current_timestamp_ms, Aria2TaskStatus};
use crate::tasks::files::cleanup_aria2_control_file;

pub(crate) fn apply_aria2_status_by_gid(
    tasks: &TaskMemoryState,
    gid: &str,
    status: &Aria2TaskStatus,
) -> Result<DownloadTask, String> {
    tasks.with_tasks_mut(|guard| {
        let task = guard
            .iter_mut()
            .find(|task| task.gid.as_deref() == Some(gid))
            .ok_or_else(|| format!("下载任务不存在，GID {}", gid))?;
        apply_aria2_status(task, status);
        Ok(task.clone())
    })?
}

pub(crate) fn apply_aria2_status(task: &mut DownloadTask, status: &Aria2TaskStatus) {
    if follow_magnet_metadata_task(task, status) {
        return;
    }

    let next_total_length = parse_aria2_u64(&status.total_length);
    let next_completed_length = parse_aria2_u64(&status.completed_length);
    let should_preserve_progress = should_preserve_existing_progress(
        &status.status,
        next_total_length,
        next_completed_length,
        task.total_length,
    );
    let should_keep_completed_length = should_keep_non_decreasing_completed_length(
        &status.status,
        next_total_length,
        next_completed_length,
        task.total_length,
        task.completed_length,
    );

    task.status = map_aria2_status(&status.status);
    if !should_preserve_progress {
        task.total_length = next_total_length;
        task.completed_length = if should_keep_completed_length {
            task.completed_length
        } else {
            next_completed_length
        };
    }
    task.download_speed = parse_aria2_u64(&status.download_speed);
    task.error_code = normalize_aria2_error_code(status.error_code.as_deref());
    task.error_message =
        readable_aria2_error_message(task.error_code.as_deref(), status.error_message.as_deref());
    if let Some(dir) = status.dir.clone().filter(|dir| !dir.is_empty()) {
        task.save_dir = dir;
    }
    task.file_path = status
        .files
        .as_ref()
        .and_then(|files| files.first())
        .map(|file| file.path.clone())
        .filter(|path| !path.is_empty())
        .or_else(|| {
            Some(
                Path::new(&task.save_dir)
                    .join(&task.file_name)
                    .display()
                    .to_string(),
            )
        });
    if task.status == DownloadTaskStatus::Complete {
        cleanup_aria2_control_file(task);
    }
    task.updated_at = current_timestamp_ms();
}

fn follow_magnet_metadata_task(task: &mut DownloadTask, status: &Aria2TaskStatus) -> bool {
    if !task.url.to_ascii_lowercase().starts_with("magnet:?") {
        return false;
    }
    let Some(next_gid) = status
        .followed_by
        .as_ref()
        .and_then(|gids| gids.first())
        .map(String::as_str)
        .filter(|gid| !gid.trim().is_empty())
    else {
        return false;
    };

    task.gid = Some(next_gid.to_string());
    task.status = DownloadTaskStatus::Pending;
    task.total_length = 0;
    task.completed_length = 0;
    task.download_speed = 0;
    task.error_code = None;
    task.error_message = None;
    if let Some(dir) = status.dir.clone().filter(|dir| !dir.is_empty()) {
        task.save_dir = dir;
    }
    task.file_path = None;
    task.updated_at = current_timestamp_ms();
    true
}

fn should_preserve_existing_progress(
    status: &str,
    next_total_length: u64,
    next_completed_length: u64,
    current_total_length: u64,
) -> bool {
    next_total_length == 0
        && next_completed_length == 0
        && current_total_length > 0
        && matches!(status, "active" | "waiting" | "paused" | "error")
}

fn should_keep_non_decreasing_completed_length(
    status: &str,
    next_total_length: u64,
    next_completed_length: u64,
    current_total_length: u64,
    current_completed_length: u64,
) -> bool {
    current_total_length > 0
        && next_total_length == current_total_length
        && next_completed_length < current_completed_length
        && matches!(status, "active" | "waiting" | "paused")
}

pub(crate) fn is_aria2_status_error(status: &Aria2TaskStatus) -> bool {
    status.status == "error"
        || normalize_aria2_error_code(status.error_code.as_deref()).is_some()
        || status
            .error_message
            .as_deref()
            .map(|message| !message.trim().is_empty())
            .unwrap_or(false)
}

pub(crate) fn normalize_aria2_error_code(error_code: Option<&str>) -> Option<String> {
    error_code
        .map(str::trim)
        .filter(|code| !code.is_empty() && *code != "0")
        .map(ToOwned::to_owned)
}

fn readable_aria2_error_message(
    error_code: Option<&str>,
    error_message: Option<&str>,
) -> Option<String> {
    let message = error_message
        .map(str::trim)
        .filter(|message| !message.is_empty());

    match (error_code, message) {
        (Some("16"), Some("Download aborted.")) => Some(
            "Download aborted. 可能原因：Aria2 无法创建或写入目标文件，请检查下载目录权限和同名文件权限。"
                .to_string(),
        ),
        (_, Some(message)) => Some(message.to_string()),
        _ => None,
    }
}

fn map_aria2_status(status: &str) -> DownloadTaskStatus {
    match status {
        "active" | "waiting" => DownloadTaskStatus::Active,
        "paused" => DownloadTaskStatus::Paused,
        "complete" => DownloadTaskStatus::Complete,
        "error" => DownloadTaskStatus::Error,
        "removed" => DownloadTaskStatus::Removed,
        _ => DownloadTaskStatus::Pending,
    }
}

pub(crate) fn parse_aria2_u64(value: &str) -> u64 {
    value.parse::<u64>().unwrap_or_default()
}
