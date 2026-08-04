use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::runtime::Aria2LifecycleCoordinator;
use crate::state::ShutdownState;
use crate::tasks::files::{
    archive_task_torrent_metadata, cleanup_empty_torrent_task_dir, read_saved_torrent_metadata,
    remove_restore_metadata, save_restore_torrent_metadata, task_download_dir,
};
use crate::tasks::prepare::{prepare_bt_download_task_with_logs, PrepareBtDownloadTaskRequest};
use crate::tasks::{
    add_torrent_to_aria2, add_uri_to_aria2, find_aria2_task_for_request,
    is_aria2_outcome_unknown_error, is_stale_aria2_gid_error, mark_magnet_task_reparsing,
    mark_task_files_confirmed, mark_task_redownloaded, mark_task_removed, mark_task_restored,
    mark_task_resumed, pause_task, pause_task_with_request_id, prepare_task_with_logs,
    prepare_torrent_task_with_logs, readd_task_to_aria2, remove_task, remove_task_record,
    remove_task_with_request_id, replace_task_snapshot, set_task_metadata_torrent_path,
    should_readd_task_after_resume_error, store_created_task_with_id,
    sync_task_progress_after_pause_by_gid, sync_task_progress_from_aria2_by_gid, task_gid,
    task_snapshot, unpause_task, unpause_task_with_request_id, validate_task_files,
    Aria2TaskCreationError, Aria2TaskRequest, CreateDownloadTaskRequest, CreateTaskAdvancedOptions,
    CreateTorrentDownloadTaskRequest, DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, PreparedDownloadTask, TaskMemoryState, TaskOperation, TaskOperationContext,
    TaskOperationType,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::repository::TaskRepository;

mod control;
mod create;
mod delete;
mod magnet;
mod proxy;
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
    aria2_rpc: &'a Aria2RpcClient,
    aria2_lifecycle: &'a std::sync::Arc<Aria2LifecycleCoordinator>,
    proxy_update_lock: &'a tokio::sync::Mutex<()>,
    runtime_guard: RuntimeGuard<'a>,
}

impl<'a> TaskService<'a> {
    pub fn new(
        repository: Box<dyn TaskRepository + 'a>,
        download_tasks: &'a TaskMemoryState,
        next_task_id: &'a AtomicU64,
        app_data_dir: &'a Path,
        debug_logs: &'a DebugLogStore,
        aria2_rpc: &'a Aria2RpcClient,
        aria2_lifecycle: &'a std::sync::Arc<Aria2LifecycleCoordinator>,
        proxy_update_lock: &'a tokio::sync::Mutex<()>,
        runtime_guard: RuntimeGuard<'a>,
    ) -> Self {
        Self {
            repository,
            download_tasks,
            next_task_id,
            app_data_dir,
            debug_logs,
            aria2_rpc,
            aria2_lifecycle,
            proxy_update_lock,
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

    async fn prepare_aria2_task_request(
        &self,
        operation: &mut TaskOperation,
        task: &PreparedDownloadTask,
    ) -> Result<(), String> {
        let mut context = operation.context.clone();
        context.aria2_request = Some(Aria2TaskRequest {
            request_id: operation.id.clone(),
            source_url: task.url.clone(),
            save_dir: task
                .aria2_save_dir
                .clone()
                .unwrap_or_else(|| task.save_dir.clone()),
            file_name: task.file_name.clone(),
        });
        self.update_task_operation(operation, "aria2_request_prepared", context)
            .await
    }

    pub(super) async fn add_uri_for_task_operation(
        &self,
        config: &Aria2Config,
        operation: &mut TaskOperation,
        task: &PreparedDownloadTask,
    ) -> Result<String, String> {
        self.prepare_aria2_task_request(operation, task).await?;
        let request_id = operation.id.clone();
        match add_uri_to_aria2(
            self.aria2_rpc,
            config,
            task,
            Some(&request_id),
            Some(self.debug_logs),
        )
        .await
        {
            Ok(gid) => Ok(gid),
            Err(error) if error.is_outcome_unknown() => {
                self.reconcile_unknown_aria2_task_creation(config, operation, error)
                    .await
            }
            Err(error) => Err(error.to_string()),
        }
    }

    pub(super) async fn add_torrent_for_task_operation(
        &self,
        config: &Aria2Config,
        operation: &mut TaskOperation,
        task: &PreparedDownloadTask,
        torrent_data: &[u8],
    ) -> Result<String, String> {
        self.prepare_aria2_task_request(operation, task).await?;
        let request_id = operation.id.clone();
        match add_torrent_to_aria2(
            self.aria2_rpc,
            config,
            task,
            torrent_data,
            Some(&request_id),
            Some(self.debug_logs),
        )
        .await
        {
            Ok(gid) => Ok(gid),
            Err(error) if error.is_outcome_unknown() => {
                self.reconcile_unknown_aria2_task_creation(config, operation, error)
                    .await
            }
            Err(error) => Err(error.to_string()),
        }
    }

    async fn reconcile_unknown_aria2_task_creation(
        &self,
        config: &Aria2Config,
        operation: &mut TaskOperation,
        error: Aria2TaskCreationError,
    ) -> Result<String, String> {
        let Some(request) = operation.context.aria2_request.as_ref() else {
            return Err(error.to_string());
        };
        let mut excluded_gids = match self.download_tasks.list() {
            Ok(tasks) => tasks
                .into_iter()
                .filter_map(|task| task.gid)
                .collect::<BTreeSet<_>>(),
            Err(list_error) => {
                let message = format!("{}；读取现有任务以核对未知结果失败：{}", error, list_error);
                self.record_unknown_aria2_outcome(operation, message.clone())
                    .await?;
                return Err(message);
            }
        };
        if let Some(old_gid) = operation.context.old_gid.as_ref() {
            excluded_gids.insert(old_gid.clone());
        }
        match find_aria2_task_for_request(
            self.aria2_rpc,
            config,
            request,
            &excluded_gids,
            Some(self.debug_logs),
        )
        .await
        {
            // 调用方会将确认的 GID 与后续任务状态在同一条既有流程中持久化，
            // 此处只返回对账结果，避免重复记录 `aria2_task_created` 副作用。
            Ok(Some(gid)) => Ok(gid),
            Ok(None) => {
                self.record_unknown_aria2_outcome(operation, error.to_string())
                    .await?;
                Err(error.to_string())
            }
            Err(reconcile_error) => {
                let message = format!("{}；即时对账失败：{}", error, reconcile_error);
                self.record_unknown_aria2_outcome(operation, message.clone())
                    .await?;
                Err(message)
            }
        }
    }

    pub(super) async fn record_unknown_aria2_outcome(
        &self,
        operation: &mut TaskOperation,
        error: String,
    ) -> Result<(), String> {
        let mut context = operation.context.clone();
        context
            .completed_side_effects
            .push("aria2_request_outcome_unknown".to_string());
        operation.error_message = Some(error);
        self.update_task_operation(operation, "aria2_outcome_unknown", context)
            .await
    }

    pub(super) fn has_unknown_aria2_outcome(&self, operation: &TaskOperation) -> bool {
        operation.phase == "aria2_outcome_unknown"
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

    pub fn list_download_task_snapshot(&self) -> Result<Vec<DownloadTask>, String> {
        query::list_download_task_snapshot(self)
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
        aria2_request: None,
        critical_paths,
        completed_side_effects: Vec::new(),
        proxy_enabled: None,
        task_snapshot,
    }
}

#[cfg(test)]
mod tests;
