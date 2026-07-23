use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::state::ShutdownState;
use crate::tasks::files::{
    archive_task_torrent_metadata, cleanup_empty_torrent_task_dir, read_saved_torrent_metadata,
    remove_restore_metadata, save_restore_torrent_metadata, task_download_dir,
};
use crate::tasks::prepare::{prepare_bt_download_task_with_logs, PrepareBtDownloadTaskRequest};
use crate::tasks::{
    add_torrent_to_aria2, add_uri_to_aria2, is_stale_aria2_gid_error, mark_magnet_task_reparsing,
    mark_task_files_confirmed, mark_task_redownloaded, mark_task_removed, mark_task_restored,
    mark_task_resumed, pause_task, prepare_task_with_logs, prepare_torrent_task_with_logs,
    readd_task_to_aria2, remove_task, remove_task_record, replace_task_snapshot,
    set_task_metadata_torrent_path, should_readd_task_after_resume_error,
    store_created_task_with_id, sync_task_progress_after_pause_by_gid,
    sync_task_progress_from_aria2_by_gid, task_gid, task_snapshot, unpause_task,
    validate_task_files, CreateDownloadTaskRequest, CreateTaskAdvancedOptions,
    CreateTorrentDownloadTaskRequest, DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, TaskMemoryState, TaskOperation, TaskOperationContext, TaskOperationType,
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
mod restore;

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

    pub(super) async fn begin_task_operation(
        &self,
        task_id: u64,
        operation_type: TaskOperationType,
        phase: impl Into<String>,
        context: TaskOperationContext,
    ) -> Result<TaskOperation, String> {
        let operation = TaskOperation::new(task_id, operation_type, phase, context);
        self.repository.begin_operation(&operation).await?;
        Ok(operation)
    }

    pub(super) async fn update_task_operation(
        &self,
        operation: &mut TaskOperation,
        phase: impl Into<String>,
        context: TaskOperationContext,
    ) -> Result<(), String> {
        operation.update_phase(phase, context);
        self.repository.update_operation(operation).await
    }

    pub(super) async fn record_aria2_task_created(
        &self,
        operation: &mut TaskOperation,
        gid: String,
    ) -> Result<(), String> {
        let mut context = operation.context.clone();
        context.new_gid = Some(gid);
        context
            .completed_side_effects
            .push("aria2_task_created".to_string());
        self.update_task_operation(operation, "aria2_created", context)
            .await
    }

    pub(super) async fn persist_task_with_operation(
        &self,
        task: &DownloadTask,
        operation: &mut TaskOperation,
        phase: impl Into<String>,
    ) -> Result<(), String> {
        let mut context = operation.context.clone();
        context
            .completed_side_effects
            .push("task_state_persisted".to_string());
        operation.update_phase(phase, context);
        self.repository
            .persist_task_state_with_operation(task, operation)
            .await
    }

    pub(super) async fn fail_task_operation(
        &self,
        operation: &mut TaskOperation,
        phase: impl Into<String>,
        error: impl Into<String>,
    ) {
        operation.fail(phase, error);
        if let Err(update_error) = self.repository.update_operation(operation).await {
            self.debug_logs.error(
                "tasks.operation",
                format!(
                    "记录失败任务操作失败，operationId {}：{}",
                    operation.id, update_error
                ),
            );
        }
    }

    pub(super) async fn complete_task_operation(
        &self,
        operation: &mut TaskOperation,
        phase: impl Into<String>,
    ) {
        operation.complete(phase);
        if let Err(error) = self.repository.update_operation(operation).await {
            self.debug_logs.warn(
                "tasks.operation",
                format!(
                    "任务已完成但操作记录未能标记完成，operationId {}：{}",
                    operation.id, error
                ),
            );
        }
    }

    pub(super) async fn rollback_task_operation_state(
        &self,
        snapshot: DownloadTask,
        operation: &mut TaskOperation,
        phase: impl Into<String>,
        reason: impl Into<String>,
    ) -> String {
        let reason = reason.into();
        let mut errors = vec![reason.clone()];
        if let Err(error) = replace_task_snapshot(self.download_tasks, snapshot.clone()) {
            errors.push(format!("恢复内存任务状态失败：{}", error));
        }
        operation.fail(phase, reason);
        if let Err(error) = self
            .repository
            .persist_task_state_with_operation(&snapshot, operation)
            .await
        {
            errors.push(format!("恢复数据库任务状态失败：{}", error));
            self.fail_task_operation(operation, "rollback_persist_failed", errors.join("；"))
                .await;
        }
        errors.join("；")
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

pub(super) fn task_operation_context(
    task_snapshot: Option<DownloadTask>,
    critical_paths: Vec<String>,
) -> TaskOperationContext {
    TaskOperationContext {
        old_gid: task_snapshot.as_ref().and_then(|task| task.gid.clone()),
        new_gid: None,
        critical_paths,
        completed_side_effects: Vec::new(),
        task_snapshot,
    }
}

#[cfg(test)]
mod tests;
