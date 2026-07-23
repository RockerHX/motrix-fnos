pub mod aria2_rpc;
pub mod files;
mod magnet_refresh;
pub mod model;
pub mod options;
pub mod prepare;
pub mod progress;
mod refresh;
pub mod repository;
pub mod service;
pub mod session;
pub mod state;
mod status;

use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::files::find_single_torrent_file;
use aria2_rpc::tell_status;
pub use aria2_rpc::{
    add_torrent_to_aria2, add_uri_to_aria2, change_task_options, pause_task, remove_task,
    unpause_task,
};
pub use files::{delete_task_files, validate_task_files};
use magnet_refresh::{resolve_followed_metadata, stale_magnet_metadata_status};
pub use model::{
    is_pending_magnet_metadata_task, should_force_pause_task_on_startup, should_pause_task_on_exit,
    CreateDownloadTaskRequest, CreateTaskAdvancedOptions, CreateTorrentDownloadTaskRequest,
    DownloadTask, DownloadTaskFile, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, PreparedDownloadTask, DEFAULT_TASK_CATEGORY,
};
pub use options::{sanitize_aria2_options, sanitize_create_task_options};
pub use prepare::{
    default_download_dir_string, prepare_task, prepare_task_with_logs,
    prepare_torrent_task_with_logs,
};
use progress::{
    apply_aria2_status, apply_aria2_status_by_gid, apply_magnet_metadata_confirmation,
    is_aria2_status_error, parse_aria2_u64,
};
use refresh::task_status_error;
pub use refresh::{
    is_stale_aria2_gid_error, refresh_tasks_from_aria2, should_readd_task_after_resume_error,
    sync_task_progress_after_pause_by_gid, sync_task_progress_from_aria2_by_gid,
};
use session::readd_download_task;
pub use session::{readd_task_to_aria2, sync_session_tasks_from_aria2};
use state::{apply_paused_state, apply_readded_gid, should_refresh_task};
pub use state::{
    list_tasks, mark_magnet_task_reparsing, mark_task_files_confirmed, mark_task_paused,
    mark_task_paused_by_gid, mark_task_redownloaded, mark_task_removed, mark_task_restored,
    mark_task_resumed, mark_unfinished_tasks_paused, remove_task_record, replace_task_snapshot,
    set_task_metadata_torrent_path, store_created_task, store_created_task_with_id, task_gid,
    task_snapshot, TaskMemoryState,
};
use status::Aria2TaskStatus;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
