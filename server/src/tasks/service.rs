use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::state::ShutdownState;
use crate::tasks::files::{
    archive_task_torrent_metadata, cleanup_empty_torrent_task_dir, read_saved_torrent_metadata,
    remove_restore_metadata, save_restore_torrent_metadata,
};
use crate::tasks::prepare::{prepare_bt_download_task_with_logs, PrepareBtDownloadTaskRequest};
use crate::tasks::{
    add_torrent_to_aria2, add_uri_to_aria2, delete_task_files, is_stale_aria2_gid_error,
    mark_task_files_confirmed, mark_task_redownloaded, mark_task_removed, mark_task_resumed,
    pause_task, prepare_task_with_logs, prepare_torrent_task_with_logs, readd_task_to_aria2,
    remove_task, remove_task_record, set_task_metadata_torrent_path,
    should_readd_task_after_resume_error, store_created_task, store_created_task_with_id,
    sync_task_progress_after_pause_by_gid, sync_task_progress_from_aria2_by_gid, task_gid,
    task_snapshot, unpause_task, CreateDownloadTaskRequest, CreateTaskAdvancedOptions,
    CreateTorrentDownloadTaskRequest, DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, TaskMemoryState,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::repository::TaskRepository;

mod control;
mod create;
mod delete;
mod magnet;
mod query;

#[derive(Clone, Copy)]
pub struct RuntimeGuard<'a> {
    shutdown: &'a ShutdownState,
}

impl<'a> RuntimeGuard<'a> {
    pub fn new(shutdown: &'a ShutdownState) -> Self {
        Self { shutdown }
    }

    pub fn ensure_running(&self) -> Result<(), String> {
        if self.shutdown.is_exiting() {
            Err("应用正在退出，不能执行任务操作".to_string())
        } else {
            Ok(())
        }
    }

    pub fn is_exiting(&self) -> bool {
        self.shutdown.is_exiting()
    }
}

pub struct TaskService<'a> {
    repository: Box<dyn TaskRepository + 'a>,
    download_tasks: &'a TaskMemoryState,
    next_task_id: &'a AtomicU64,
    app_data_dir: &'a Path,
    debug_logs: &'a DebugLogStore,
    runtime_guard: RuntimeGuard<'a>,
}

impl<'a> TaskService<'a> {
    pub fn new(
        repository: Box<dyn TaskRepository + 'a>,
        download_tasks: &'a TaskMemoryState,
        next_task_id: &'a AtomicU64,
        app_data_dir: &'a Path,
        debug_logs: &'a DebugLogStore,
        runtime_guard: RuntimeGuard<'a>,
    ) -> Self {
        Self {
            repository,
            download_tasks,
            next_task_id,
            app_data_dir,
            debug_logs,
            runtime_guard,
        }
    }

    pub fn ensure_not_exiting(&self) -> Result<(), String> {
        self.runtime_guard.ensure_running()
    }

    pub async fn list_download_tasks(
        &self,
        config: &Aria2Config,
    ) -> Result<Vec<DownloadTask>, String> {
        query::list_download_tasks(self, config).await
    }

    pub fn list_removed_download_tasks(&self) -> Result<Vec<DownloadTask>, String> {
        query::list_removed_download_tasks(self)
    }
}

#[cfg(test)]
mod tests;
