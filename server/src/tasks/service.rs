use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::state::ShutdownState;
use crate::tasks::files::{cleanup_empty_torrent_task_dir, read_saved_torrent_metadata};
use crate::tasks::prepare::{prepare_bt_download_task_with_logs, PrepareBtDownloadTaskRequest};
use crate::tasks::{
    add_torrent_to_aria2, add_uri_to_aria2, delete_task_files, is_stale_aria2_gid_error,
    mark_task_files_confirmed, mark_task_paused, mark_task_redownloaded, mark_task_removed,
    mark_task_resumed, pause_task, prepare_task_with_logs, prepare_torrent_task_with_logs,
    readd_task_to_aria2, remove_task, remove_task_record, should_readd_task_after_resume_error,
    store_created_task, store_created_task_with_id, sync_task_progress_after_pause_by_gid,
    sync_task_progress_from_aria2_by_gid, task_gid, task_snapshot, unpause_task,
    CreateDownloadTaskRequest, CreateTaskAdvancedOptions, CreateTorrentDownloadTaskRequest,
    DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode, DownloadTaskStatus,
    TaskMemoryState,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::repository::TaskRepository;

mod control;
mod create;
mod delete;
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

    pub async fn confirm_download_task_files(
        &self,
        config: &Aria2Config,
        task_id: u64,
        selected_file_indexes: Vec<u32>,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let mut selected = selected_file_indexes
            .into_iter()
            .filter(|index| *index > 0)
            .collect::<Vec<_>>();
        selected.sort_unstable();
        selected.dedup();
        if selected.is_empty() {
            return Err("请至少选择一个文件".to_string());
        }

        let task = task_snapshot(self.download_tasks, task_id)?;
        if !task.confirmation_required {
            return Err("当前任务不需要确认文件".to_string());
        }
        let select_file = selected
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let selected_set = selected
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let task_file_indexes = task
            .files
            .iter()
            .map(|file| file.index)
            .collect::<std::collections::BTreeSet<_>>();
        if !selected_set.is_subset(&task_file_indexes) {
            return Err("选择的文件索引不在任务文件列表中".to_string());
        }

        let torrent_data = read_saved_torrent_metadata(&task)?;
        let mut options = serde_json::Map::new();
        options.insert("select-file".to_string(), serde_json::json!(select_file));
        let prepared = prepare_bt_download_task_with_logs(
            PrepareBtDownloadTaskRequest {
                source_url: task.url.clone(),
                display_name: task.file_name.clone(),
                base_save_dir: task.save_dir.clone(),
                source_type: DownloadTaskSourceType::Magnet,
                start_mode: DownloadTaskStartMode::Now,
                category: Some(task.category.clone()),
                advanced_options: CreateTaskAdvancedOptions::default(),
                aria2_options: options,
                task_kind: "磁链",
            },
            Some(self.debug_logs),
        )?;
        let gid =
            match add_torrent_to_aria2(config, &prepared, &torrent_data, Some(self.debug_logs))
                .await
            {
                Ok(gid) => gid,
                Err(error) => {
                    cleanup_empty_torrent_task_dir(&prepared);
                    return Err(error);
                }
            };
        delete::remove_magnet_metadata_dir(self.app_data_dir, &task);
        let mut task = mark_task_files_confirmed(
            self.download_tasks,
            task_id,
            gid.clone(),
            prepared.save_dir.clone(),
            &selected,
        )?;

        match sync_task_progress_from_aria2_by_gid(
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await
        {
            Ok(synced_task) => task = synced_task,
            Err(error) => self.debug_logs.warn(
                "tasks.control",
                format!(
                    "确认文件后同步最新进度失败，使用最后已知进度，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            ),
        }
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!("任务文件已确认并开始下载，ID {}，GID {}", task_id, gid),
        );
        Ok(task)
    }

    async fn sync_task_to_database(&self, task: &DownloadTask) -> Result<(), String> {
        query::sync_task_to_database(self, task).await
    }
}

fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}

#[cfg(test)]
mod tests;
