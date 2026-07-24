use super::*;
use crate::tasks::files::{stage_task_files, StagedTaskFiles};

impl<'a> TaskService<'a> {
    pub async fn delete_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
        delete_files: bool,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Delete,
                "prepared",
                task_operation_context(
                    Some(snapshot.clone()),
                    vec![
                        task_download_dir(&snapshot).to_string(),
                        snapshot.file_path.clone().unwrap_or_default(),
                    ],
                ),
            )
            .await?;
        let mut task_before_delete = snapshot.clone();
        if matches!(
            task_before_delete.source_type,
            DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
        ) {
            match archive_task_torrent_metadata(self.app_data_dir, &task_before_delete) {
                Ok(path) => {
                    task_before_delete = set_task_metadata_torrent_path(
                        self.download_tasks,
                        task_id,
                        path.display().to_string(),
                    )?;
                }
                Err(error) => self.debug_logs.warn(
                    "tasks.restore",
                    format!(
                        "删除任务前未能归档 BT metadata，后续恢复可能受限：{}",
                        error
                    ),
                ),
            }
        }
        let gid = task_before_delete
            .gid
            .clone()
            .filter(|gid| !gid.trim().is_empty());
        if let Some(gid) = gid.as_deref() {
            let request_id = operation.id.clone();
            if let Err(error) = remove_task_with_request_id(
                self.aria2_rpc,
                config,
                gid,
                Some(&request_id),
                Some(self.debug_logs),
            )
            .await
            {
                if is_aria2_outcome_unknown_error(&error) {
                    self.record_unknown_aria2_outcome(&mut operation, error.clone())
                        .await?;
                    return Err(error);
                }
                if is_stale_aria2_gid_error(&error) {
                    self.debug_logs.warn(
                        "tasks.control",
                        format!(
                            "删除任务时 Aria2 已无此 GID，继续删除本地任务记录，ID {}，GID {}：{}",
                            task_id, gid, error
                        ),
                    );
                } else {
                    return Err(self
                        .rollback_task_operation_state(
                            snapshot,
                            &mut operation,
                            "aria2_remove_failed",
                            error,
                        )
                        .await);
                }
            }
        }
        let mut context = operation.context.clone();
        context
            .completed_side_effects
            .push("aria2_task_removed".to_string());
        if let Err(error) = self
            .update_task_operation(&mut operation, "aria2_removed", context)
            .await
        {
            return Err(self
                .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                .await);
        }
        let staged = if delete_files {
            match stage_task_files(&task_before_delete) {
                Ok(staged) => staged,
                Err(error) => {
                    return Err(self
                        .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                        .await);
                }
            }
        } else {
            None
        };
        if let Some(staged_files) = staged.as_ref() {
            let mut context = operation.context.clone();
            context
                .critical_paths
                .push(staged_files.backup_dir().display().to_string());
            context
                .completed_side_effects
                .push("task_files_staged".to_string());
            if let Err(error) = self
                .update_task_operation(&mut operation, "files_staged", context)
                .await
            {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }
        }
        let mut task = match mark_task_removed(self.download_tasks, task_id, false) {
            Ok(task) => task,
            Err(error) => {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }
        };
        task.files_deleted = delete_files;
        if let Err(error) = replace_task_snapshot(self.download_tasks, task.clone()) {
            return Err(self
                .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                .await);
        }
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_removed")
            .await
        {
            return Err(self
                .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                .await);
        }
        remove_magnet_metadata_dir(self.app_data_dir, &task_before_delete);
        if let Some(staged) = staged {
            if let Err(error) = staged.commit() {
                operation.require_manual_review("file_cleanup_pending", error.clone());
                if let Err(update_error) = self.repository.update_operation(&operation).await {
                    self.debug_logs.error(
                        "tasks.operation",
                        format!(
                            "任务文件暂存清理失败且未能更新操作记录，operationId {}：{}",
                            operation.id, update_error
                        ),
                    );
                }
                self.debug_logs.warn(
                    "tasks.control",
                    format!(
                        "任务记录已删除，但暂存文件尚未清理，ID {}：{}",
                        task_id, error
                    ),
                );
            } else {
                operation
                    .context
                    .completed_side_effects
                    .push("task_files_deleted".to_string());
                self.complete_task_operation(&mut operation, "completed")
                    .await;
            }
        } else {
            self.complete_task_operation(&mut operation, "completed")
                .await;
        }
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
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let task = task_snapshot(self.download_tasks, task_id)?;
        if task.status != DownloadTaskStatus::Removed {
            return Err("只有已删除任务可以永久删除".to_string());
        }

        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::PermanentDelete,
                "prepared",
                task_operation_context(Some(task.clone()), Vec::new()),
            )
            .await?;
        operation.complete("record_deleted");
        if !self
            .repository
            .delete_task_record_with_operation(task_id, &operation)
            .await?
        {
            return Err(format!("下载任务不存在：{}", task_id));
        }
        if let Err(error) = remove_task_record(self.download_tasks, task_id) {
            operation.require_manual_review("memory_record_cleanup_failed", error.clone());
            let _ = self.repository.update_operation(&operation).await;
            return Err(error);
        }
        remove_restore_metadata(self.app_data_dir, task_id);
        self.debug_logs.info(
            "tasks.control",
            format!("已永久删除回收站任务记录，ID {}", task_id),
        );
        Ok(())
    }

    async fn rollback_delete_after_aria2_removal(
        &self,
        snapshot: DownloadTask,
        staged: Option<StagedTaskFiles>,
        operation: &mut TaskOperation,
        reason: impl Into<String>,
    ) -> String {
        let mut errors = vec![reason.into()];
        if let Some(error) = restore_staged_files(staged) {
            errors.push(error);
        }
        if let Err(error) = replace_task_snapshot(self.download_tasks, snapshot.clone()) {
            errors.push(format!("恢复内存任务状态失败：{}", error));
        }
        operation.require_manual_review("task_remove_needs_reconcile", errors.join("；"));
        if let Err(error) = self
            .repository
            .persist_task_state_with_operation(&snapshot, operation)
            .await
        {
            errors.push(format!("恢复数据库任务状态失败：{}", error));
            self.debug_logs.error(
                "tasks.operation",
                format!(
                    "删除任务回滚后未能记录待对账操作，operationId {}：{}",
                    operation.id, error
                ),
            );
        }
        errors.join("；")
    }
}

fn restore_staged_files(staged: Option<StagedTaskFiles>) -> Option<String> {
    staged.and_then(|staged| staged.restore().err())
}

pub(super) fn remove_magnet_metadata_dir(app_data_dir: &Path, task: &DownloadTask) {
    // 磁链临时目录只按任务 ID 定位；稳定恢复 metadata 位于另一棵私有目录，不随临时目录清理。
    let expected_dir = magnet::magnet_metadata_task_dir(app_data_dir, task.id);
    if !expected_dir.exists() {
        return;
    }
    let Ok(expected_dir) = expected_dir.canonicalize() else {
        return;
    };
    let _ = fs::remove_dir_all(expected_dir);
}
