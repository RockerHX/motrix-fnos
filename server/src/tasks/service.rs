use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::state::ShutdownState;
use crate::tasks::{
    add_uri_to_aria2, delete_task_files, is_stale_aria2_gid_error, mark_task_paused,
    mark_task_redownloaded, mark_task_removed, mark_task_resumed, pause_task,
    prepare_task_with_logs, readd_task_to_aria2, refresh_tasks_from_aria2, remove_task,
    remove_task_record, should_readd_task_after_resume_error, store_created_task,
    sync_task_progress_after_pause_by_gid, sync_task_progress_from_aria2_by_gid, task_gid,
    task_snapshot, unpause_task, CreateDownloadTaskRequest, DownloadTask, DownloadTaskStatus,
    TaskMemoryState,
};
use std::sync::atomic::AtomicU64;

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
    repository: TaskRepository<'a>,
    download_tasks: &'a TaskMemoryState,
    next_task_id: &'a AtomicU64,
    debug_logs: &'a DebugLogStore,
    runtime_guard: RuntimeGuard<'a>,
}

impl<'a> TaskService<'a> {
    pub fn new(
        repository: TaskRepository<'a>,
        download_tasks: &'a TaskMemoryState,
        next_task_id: &'a AtomicU64,
        debug_logs: &'a DebugLogStore,
        runtime_guard: RuntimeGuard<'a>,
    ) -> Self {
        Self {
            repository,
            download_tasks,
            next_task_id,
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
        let prepared = prepare_task_with_logs(payload, self.debug_logs)?;
        let gid = add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await?;
        let task = store_created_task(self.download_tasks, self.next_task_id, prepared, gid)?;
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

        let tasks =
            refresh_tasks_from_aria2(self.download_tasks, config, Some(self.debug_logs)).await?;
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
        let gid = task_gid(self.download_tasks, task_id)?;
        let task_before_resume = task_snapshot(self.download_tasks, task_id)?;
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
            save_dir: task.save_dir.clone(),
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
        let gid = task_gid(self.download_tasks, task_id)?;
        if let Err(error) = remove_task(config, &gid, Some(self.debug_logs)).await {
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
        let task = mark_task_removed(self.download_tasks, task_id, delete_files)?;
        self.sync_task_to_database(&task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已删除，ID {}，GID {}，删除本地文件 {}",
                task_id,
                gid,
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
