use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::state::ShutdownState;
use crate::tasks::files::{cleanup_empty_torrent_task_dir, read_saved_torrent_metadata};
use crate::tasks::prepare::{prepare_bt_download_task_with_logs, PrepareBtDownloadTaskRequest};
use crate::tasks::{
    add_torrent_to_aria2, add_uri_to_aria2, delete_task_files, is_stale_aria2_gid_error,
    mark_task_files_confirmed, mark_task_paused, mark_task_redownloaded, mark_task_removed,
    mark_task_resumed, pause_task, prepare_task_with_logs, prepare_torrent_task_with_logs,
    readd_task_to_aria2, refresh_tasks_from_aria2, remove_task, remove_task_record,
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

    pub async fn create_download_task(
        &self,
        config: &Aria2Config,
        payload: CreateDownloadTaskRequest,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        if payload
            .save_dir
            .as_deref()
            .map(|save_dir| save_dir.trim().is_empty())
            .unwrap_or(true)
        {
            return Err("请选择已授权的保存目录".to_string());
        }
        let mut prepared = prepare_task_with_logs(payload, self.debug_logs)?;
        let task = if prepared.source_type == DownloadTaskSourceType::Magnet {
            let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
            let metadata_dir = magnet_metadata_task_dir(self.app_data_dir, task_id);
            fs::create_dir_all(&metadata_dir).map_err(|error| {
                format!(
                    "创建磁链 metadata 临时目录失败：{}（{}）",
                    metadata_dir.display(),
                    error
                )
            })?;
            prepared.aria2_save_dir = Some(metadata_dir.display().to_string());
            let gid = match add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await {
                Ok(gid) => gid,
                Err(error) => {
                    let _ = fs::remove_dir_all(&metadata_dir);
                    return Err(error);
                }
            };
            store_created_task_with_id(self.download_tasks, task_id, prepared, gid)?
        } else {
            let gid = match add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await {
                Ok(gid) => gid,
                Err(error) => {
                    cleanup_empty_torrent_task_dir(&prepared);
                    return Err(error);
                }
            };
            store_created_task(self.download_tasks, self.next_task_id, prepared, gid)?
        };
        self.repository.upsert_task(&task).await?;
        self.debug_logs.info(
            "tasks.create",
            format!(
                "下载任务已写入内存列表和 SQLite，ID {}，GID {}",
                task.id,
                task.gid.as_deref().unwrap_or("-")
            ),
        );
        Ok(task)
    }

    pub async fn create_torrent_download_task(
        &self,
        config: &Aria2Config,
        payload: CreateTorrentDownloadTaskRequest,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        if payload.save_dir.trim().is_empty() {
            return Err("请选择已授权的保存目录".to_string());
        }
        let torrent_data = payload.torrent_data.clone();
        let prepared = prepare_torrent_task_with_logs(payload, self.debug_logs)?;
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
        let task = store_created_task(self.download_tasks, self.next_task_id, prepared, gid)?;
        self.repository.upsert_task(&task).await?;
        self.debug_logs.info(
            "tasks.create",
            format!(
                "种子任务已写入内存列表和 SQLite，ID {}，GID {}",
                task.id,
                task.gid.as_deref().unwrap_or("-")
            ),
        );
        Ok(task)
    }

    pub async fn list_download_tasks(
        &self,
        config: &Aria2Config,
    ) -> Result<Vec<DownloadTask>, String> {
        if self.runtime_guard.is_exiting() {
            self.debug_logs.info(
                "tasks.list",
                "应用正在退出，跳过 Aria2 刷新并返回内存任务快照",
            );
            return Ok(visible_tasks(crate::tasks::list_tasks(
                self.download_tasks,
            )?));
        }

        let tasks = refresh_tasks_from_aria2(
            self.download_tasks,
            self.app_data_dir,
            config,
            Some(self.debug_logs),
        )
        .await?;
        self.sync_tasks_to_database(&tasks).await?;

        Ok(visible_tasks(tasks))
    }

    pub fn list_removed_download_tasks(&self) -> Result<Vec<DownloadTask>, String> {
        let tasks = crate::tasks::list_tasks(self.download_tasks)?;
        Ok(removed_tasks(tasks))
    }

    pub async fn pause_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let gid = task_gid(self.download_tasks, task_id)?;
        pause_task(config, &gid, Some(self.debug_logs)).await?;
        if let Err(error) = sync_task_progress_after_pause_by_gid(
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await
        {
            self.debug_logs.warn(
                "tasks.control",
                format!(
                    "暂停后同步最新进度失败，使用最后已知进度，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            );
        }
        let task = mark_task_paused(self.download_tasks, task_id)?;
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!("任务已暂停，ID {}，GID {}", task_id, gid),
        );
        Ok(task)
    }

    pub async fn resume_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let task_before_resume = task_snapshot(self.download_tasks, task_id)?;
        if task_before_resume.confirmation_required {
            return Err("请先确认要下载的文件".to_string());
        }
        let gid = task_gid(self.download_tasks, task_id)?;
        let task = match unpause_task(config, &gid, Some(self.debug_logs)).await {
            Ok(_) => {
                if let Err(error) = sync_task_progress_from_aria2_by_gid(
                    self.download_tasks,
                    config,
                    &gid,
                    Some(self.debug_logs),
                )
                .await
                {
                    self.debug_logs.warn(
                        "tasks.control",
                        format!(
                            "恢复后同步最新进度失败，使用最后已知进度，ID {}，GID {}：{}",
                            task_id, gid, error
                        ),
                    );
                }
                mark_task_resumed(self.download_tasks, task_id)?
            }
            Err(error) if should_readd_task_after_resume_error(&task_before_resume, &error) => {
                self.debug_logs.warn(
                    "tasks.restore",
                    format!("恢复任务时发现旧 GID 已失效，准备重新加入任务：{}", error),
                );
                readd_task_to_aria2(self.download_tasks, config, task_id, Some(self.debug_logs))
                    .await?
            }
            Err(error) => return Err(error),
        };
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已恢复，ID {}，旧 GID {}，当前 GID {}",
                task_id,
                gid,
                task.gid.as_deref().unwrap_or("-")
            ),
        );
        Ok(task)
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
        remove_magnet_metadata_dir(self.app_data_dir, &task);
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

    pub async fn redownload_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let task = task_snapshot(self.download_tasks, task_id)?;
        if task.status != DownloadTaskStatus::Complete {
            return Err("只有已完成任务可以重新下载".to_string());
        }

        delete_task_files(&task)?;
        let prepared = crate::tasks::PreparedDownloadTask {
            url: task.url.clone(),
            file_name: task.file_name.clone(),
            output_file_name: Some(task.file_name.clone()),
            save_dir: task.save_dir.clone(),
            aria2_save_dir: None,
            category: task.category.clone(),
            source_type: if task.url.to_ascii_lowercase().starts_with("magnet:?") {
                DownloadTaskSourceType::Magnet
            } else {
                DownloadTaskSourceType::Url
            },
            start_mode: DownloadTaskStartMode::Now,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        };
        let gid = add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await?;
        let task = mark_task_redownloaded(self.download_tasks, task_id, gid.clone())?;
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已重新下载，ID {}，GID {}，原本地文件已删除",
                task_id, gid
            ),
        );
        Ok(task)
    }

    pub async fn delete_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
        delete_files: bool,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let task_before_delete = task_snapshot(self.download_tasks, task_id)?;
        let gid = task_before_delete
            .gid
            .clone()
            .filter(|gid| !gid.trim().is_empty());
        if let Some(gid) = gid.as_deref() {
            if let Err(error) = remove_task(config, gid, Some(self.debug_logs)).await {
                if is_stale_aria2_gid_error(&error) {
                    self.debug_logs.warn(
                        "tasks.control",
                        format!(
                            "删除任务时 Aria2 已无此 GID，继续删除本地任务记录，ID {}，GID {}：{}",
                            task_id, gid, error
                        ),
                    );
                } else {
                    return Err(error);
                }
            }
        }
        remove_magnet_metadata_dir(self.app_data_dir, &task_before_delete);
        let task = mark_task_removed(self.download_tasks, task_id, delete_files)?;
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已删除，ID {}，GID {}，删除本地文件 {}",
                task_id,
                gid.as_deref().unwrap_or("-"),
                if delete_files { "是" } else { "否" }
            ),
        );
        Ok(task)
    }

    pub async fn permanently_delete_removed_task(&self, task_id: u64) -> Result<(), String> {
        self.ensure_not_exiting()?;
        let task = task_snapshot(self.download_tasks, task_id)?;
        if task.status != DownloadTaskStatus::Removed {
            return Err("只有已删除任务可以永久删除".to_string());
        }

        if !self.repository.delete_task_record(task_id).await? {
            return Err(format!("下载任务不存在：{}", task_id));
        }
        remove_task_record(self.download_tasks, task_id)?;
        self.debug_logs.info(
            "tasks.control",
            format!("已永久删除回收站任务记录，ID {}", task_id),
        );
        Ok(())
    }

    async fn sync_tasks_to_database(&self, tasks: &[DownloadTask]) -> Result<(), String> {
        self.repository.persist_task_states(tasks).await
    }

    async fn sync_task_to_database(&self, task: &DownloadTask) -> Result<(), String> {
        self.repository.persist_task_state(task).await
    }
}

fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}

fn remove_magnet_metadata_dir(app_data_dir: &Path, task: &DownloadTask) {
    let expected_dir = magnet_metadata_task_dir(app_data_dir, task.id);
    if !expected_dir.exists() {
        return;
    }
    let Ok(expected_dir) = expected_dir.canonicalize() else {
        return;
    };
    if let Some(metadata_torrent_path) = task.metadata_torrent_path.as_deref() {
        if let Some(dir) = Path::new(metadata_torrent_path).parent() {
            if let Ok(dir) = dir.canonicalize() {
                if dir != expected_dir {
                    return;
                }
            }
        }
    }
    let _ = fs::remove_dir_all(expected_dir);
}

fn visible_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .collect()
}

fn removed_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status == DownloadTaskStatus::Removed)
        .collect()
}

#[cfg(test)]
mod tests;
