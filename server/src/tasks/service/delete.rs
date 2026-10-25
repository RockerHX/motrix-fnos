use super::*;
use crate::tasks::files::{prepare_task_file_staging, StagedTaskFiles};
use crate::tasks::operation::FILE_CLEANUP_PENDING_PHASE;

impl<'a> TaskService<'a> {
    pub async fn delete_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        self.ensure_task_engine_compatible(config, task_id)?;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.status == DownloadTaskStatus::Removed {
            return Err("任务已在回收站中".to_string());
        }
        self.ensure_file_cleanup_not_pending(task_id).await?;
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
        let native = snapshot.engine_generation == crate::config::aria2::EngineGeneration::NativeV2;
        if !native
            && matches!(
                task_before_delete.source_type,
                DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
            )
        {
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
        if native && snapshot.status != DownloadTaskStatus::Complete {
            match self
                .pause_retained_task(config, &snapshot, &mut operation)
                .await
            {
                Ok(paused) => {
                    task_before_delete = paused;
                    replace_task_snapshot(self.download_tasks, task_before_delete.clone())?;
                }
                Err(error) => {
                    if self.has_unknown_aria2_outcome(&operation) {
                        return Err(error);
                    }
                    if error.contains("task_pause_unconfirmed") {
                        return Err(self
                            .rollback_task_operation_state(
                                snapshot,
                                &mut operation,
                                "aria2_pause_unconfirmed",
                                error,
                            )
                            .await);
                    }
                    return Err(self
                        .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                        .await);
                }
            }
        } else if snapshot.status != DownloadTaskStatus::Complete {
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
        }
        let mut context = operation.context.clone();
        context.completed_side_effects.push(
            if native {
                "aria2_task_retained"
            } else {
                "aria2_task_removed"
            }
            .to_string(),
        );
        if let Err(error) = self
            .update_task_operation(
                &mut operation,
                if native {
                    "aria2_retained"
                } else {
                    "aria2_removed"
                },
                context,
            )
            .await
        {
            return Err(self
                .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                .await);
        }
        let task = match mark_task_removed(self.download_tasks, task_id, false) {
            Ok(task) => task,
            Err(error) => {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                    .await);
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_removed")
            .await
        {
            return Err(self
                .rollback_delete_after_aria2_removal(snapshot, None, &mut operation, error)
                .await);
        }
        if !native {
            remove_magnet_metadata_dir(self.app_data_dir, &task_before_delete);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已移入回收站并保留下载文件，ID {}，GID {}",
                task_id,
                gid.as_deref().unwrap_or("-")
            ),
        );
        Ok(task)
    }

    pub async fn permanently_delete_removed_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
        delete_files: bool,
    ) -> Result<Option<DownloadTask>, String> {
        self.ensure_not_exiting()?;
        self.ensure_task_engine_compatible(config, task_id)?;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.status != DownloadTaskStatus::Removed {
            return Err("只有回收站任务可以永久删除".to_string());
        }
        self.ensure_file_cleanup_not_pending(task_id).await?;
        if delete_files && !snapshot.files_deleted {
            self.validate_task_authorization(&snapshot)?;
            self.prepare_existing_task_destination(&snapshot)?;
            self.validate_payload_ownership(&snapshot)?;
            if self.download_tasks.list()?.iter().any(|other| {
                other.id != task_id && snapshot.gid.is_some() && other.gid == snapshot.gid
            }) {
                return Err(
                    "task_shared_resource_conflict：引擎任务由其他记录引用，已保留文件".to_string(),
                );
            }
        }
        let mut staged = if delete_files && !snapshot.files_deleted {
            prepare_task_file_staging(&snapshot)?
        } else {
            None
        };
        let mut context = task_operation_context(Some(snapshot.clone()), Vec::new());
        if delete_files && !snapshot.files_deleted {
            context
                .completed_side_effects
                .push("delete_files_requested".to_string());
        }
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::PermanentDelete,
                "prepared",
                context,
            )
            .await?;
        if let Err(error) = self
            .release_retained_task(config, &snapshot, &mut operation)
            .await
        {
            if !self.has_unknown_aria2_outcome(&operation) {
                operation.require_manual_review("engine_release_failed", error.clone());
                self.repository.update_operation(&operation).await?;
            }
            return Err(error);
        }
        if let Some(staged_files) = staged.as_mut() {
            if let Err(error) = self
                .validate_task_authorization(&snapshot)
                .and_then(|_| self.prepare_existing_task_destination(&snapshot))
                .and_then(|_| self.validate_payload_ownership(&snapshot))
            {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }
            let mut context = operation.context.clone();
            let backup_path = staged_files.backup_dir().display().to_string();
            context.critical_paths.push(backup_path);
            if let Err(error) = self
                .update_task_operation(&mut operation, "file_staging_in_progress", context)
                .await
            {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }
            if let Err(error) = staged_files.stage() {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }

            let mut context = operation.context.clone();
            context
                .file_cleanup_paths
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
        if staged.is_some() {
            let mut task = snapshot.clone();
            task.gid = None;
            task.files_deleted = true;
            task.completed_length = 0;
            for file in &mut task.files {
                file.completed_length = 0;
            }
            if let Err(error) = self
                .persist_task_with_operation(&task, &mut operation, FILE_CLEANUP_PENDING_PHASE)
                .await
            {
                return Err(self
                    .rollback_delete_after_aria2_removal(snapshot, staged, &mut operation, error)
                    .await);
            }
            replace_task_snapshot(self.download_tasks, task.clone())?;
            return Ok(Some(task));
        }
        self.finish_permanent_delete(&snapshot, &mut operation)
            .await?;
        Ok(None)
    }

    pub(crate) async fn finish_permanent_delete(
        &self,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<(), String> {
        operation.complete("record_deleted");
        if !self
            .repository
            .delete_task_record_with_operation(task.id, &operation)
            .await?
        {
            return Err(format!("下载任务不存在：{}", task.id));
        }
        if let Err(error) = remove_task_record(self.download_tasks, task.id) {
            operation.require_manual_review("memory_record_cleanup_failed", error.clone());
            let _ = self.repository.update_operation(&operation).await;
            return Err(error);
        }
        let metadata_dir = self
            .app_data_dir
            .join("task-metadata")
            .join(format!("task-{}", task.id));
        if !self.download_tasks.list()?.iter().any(|other| {
            other
                .metadata_torrent_path
                .as_deref()
                .is_some_and(|path| Path::new(path).starts_with(&metadata_dir))
        }) {
            remove_restore_metadata(self.app_data_dir, task.id);
        }
        remove_magnet_metadata_dir(self.app_data_dir, &task);
        self.debug_logs.info(
            "tasks.control",
            format!("已永久删除回收站任务记录，ID {}", task.id),
        );
        Ok(())
    }

    async fn rollback_delete_after_aria2_removal(
        &self,
        mut snapshot: DownloadTask,
        staged: Option<StagedTaskFiles>,
        operation: &mut TaskOperation,
        reason: impl Into<String>,
    ) -> String {
        let mut errors = vec![reason.into()];
        if snapshot.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
            && !matches!(
                snapshot.status,
                DownloadTaskStatus::Complete | DownloadTaskStatus::Removed
            )
        {
            if let Ok(paused) = task_snapshot(self.download_tasks, snapshot.id) {
                if paused.status == DownloadTaskStatus::Paused
                    || paused.status == DownloadTaskStatus::Complete
                {
                    snapshot = paused;
                }
            }
            if snapshot.status != DownloadTaskStatus::Complete {
                snapshot.status = DownloadTaskStatus::Paused;
            }
            snapshot.download_speed = 0;
        }
        let mut staged = staged;
        if let Some(files) = staged.as_mut() {
            if self.prepare_task_restore_destination(&snapshot).is_ok() {
                if let Err(error) = files.restore() {
                    errors.push(error);
                }
            } else {
                errors.push(format!(
                    "目录授权已撤销，保留删除暂存文件：{}",
                    files.backup_dir().display()
                ));
            }
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

    pub(super) async fn pause_retained_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<DownloadTask, String> {
        let gid = task
            .gid
            .as_deref()
            .filter(|gid| !gid.trim().is_empty())
            .ok_or_else(|| "task_resume_state_invalid：原 GID 缺失，已保留文件".to_string())?;
        self.update_task_operation(
            operation,
            "aria2_pause_requested",
            operation.context.clone(),
        )
        .await?;
        let status = crate::tasks::aria2_rpc::tell_status(
            self.aria2_rpc,
            config,
            gid,
            Some(self.debug_logs),
        )
        .await
        .map_err(|error| format!("task_resume_state_invalid：无法确认原任务：{error}"))?;
        if !matches!(status.status.as_str(), "paused" | "complete" | "error") {
            if let Err(error) = pause_task_with_request_id(
                self.aria2_rpc,
                config,
                gid,
                Some(&operation.id),
                Some(self.debug_logs),
            )
            .await
            {
                if is_aria2_outcome_unknown_error(&error) {
                    self.record_unknown_aria2_outcome(operation, error.clone())
                        .await?;
                }
                return Err(error);
            }
        }
        let status = crate::tasks::refresh::read_task_status_after_pause(
            self.aria2_rpc,
            config,
            gid,
            Some(self.debug_logs),
        )
        .await?;
        let mut stopped = task.clone();
        if stopped.status != DownloadTaskStatus::Removed
            && stopped.status != DownloadTaskStatus::Complete
        {
            stopped.status = DownloadTaskStatus::Paused;
        }
        stopped.download_speed = 0;
        replace_task_snapshot(self.download_tasks, stopped)?;
        let expected_dir =
            if task.source_type == DownloadTaskSourceType::Magnet && task.file_path.is_none() {
                magnet::magnet_metadata_task_dir(self.app_data_dir, task.id)
            } else {
                PathBuf::from(task_download_dir(task))
            };
        let paused = crate::tasks::session::reconcile_native_task(task, &status, &expected_dir)?;
        replace_task_snapshot(self.download_tasks, paused.clone())?;
        let mut context = operation.context.clone();
        context
            .completed_side_effects
            .push("aria2_task_paused".to_string());
        self.update_task_operation(operation, "aria2_paused", context)
            .await?;
        crate::aria2::save_session(self.aria2_rpc, config, Some(self.debug_logs)).await?;
        Ok(paused)
    }

    pub(super) async fn release_retained_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<(), String> {
        let Some(gid) = task.gid.as_deref().filter(|gid| !gid.trim().is_empty()) else {
            return Ok(());
        };
        if self
            .download_tasks
            .list()?
            .iter()
            .any(|other| other.id != task.id && other.gid.as_deref() == Some(gid))
        {
            return Ok(());
        }
        self.update_task_operation(
            operation,
            "engine_release_requested",
            operation.context.clone(),
        )
        .await?;
        match remove_task_with_request_id(
            self.aria2_rpc,
            config,
            gid,
            Some(&operation.id),
            Some(self.debug_logs),
        )
        .await
        {
            Ok(_) => {}
            Err(error) if is_stale_aria2_gid_error(&error) => {}
            Err(error) => {
                if is_aria2_outcome_unknown_error(&error) {
                    self.record_unknown_aria2_outcome(operation, error.clone())
                        .await?;
                }
                return Err(error);
            }
        }
        crate::aria2::save_session(self.aria2_rpc, config, Some(self.debug_logs)).await?;
        if config.engine_generation() == crate::config::aria2::EngineGeneration::NativeV2 {
            let still_present =
                crate::tasks::task_exists(self.aria2_rpc, config, gid, Some(self.debug_logs))
                    .await?;
            if still_present {
                remove_task_result(
                    self.aria2_rpc,
                    config,
                    gid,
                    Some(&format!("{}:remove-result", operation.id)),
                    Some(self.debug_logs),
                )
                .await
                .map_err(|error| {
                    if is_aria2_outcome_unknown_error(&error) {
                        format!("task_outcome_unknown：原 GID 结果清理未确认：{error}")
                    } else {
                        error
                    }
                })?;
            }
            if crate::tasks::task_exists(self.aria2_rpc, config, gid, Some(self.debug_logs)).await?
            {
                return Err("task_outcome_unknown：原 GID 尚未移除，保留任务记录".to_string());
            }
        }
        let mut context = operation.context.clone();
        context
            .completed_side_effects
            .push("aria2_task_removed".to_string());
        self.update_task_operation(operation, "engine_released", context)
            .await
    }

    fn prepare_existing_task_destination(&self, task: &DownloadTask) -> Result<(), String> {
        if crate::tasks::is_pending_magnet_metadata_task(task) {
            return Ok(());
        }
        match fs::symlink_metadata(task_download_dir(task)) {
            Ok(_) => self.prepare_task_destination(task),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!(
                "检查任务保存目录失败：{}（{}）",
                task_download_dir(task),
                error
            )),
        }
    }

    fn validate_payload_ownership(&self, task: &DownloadTask) -> Result<(), String> {
        if crate::tasks::is_pending_magnet_metadata_task(task) {
            return Ok(());
        }
        let target = if matches!(
            task.source_type,
            DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
        ) {
            PathBuf::from(task_download_dir(task))
        } else {
            let Some(path) = task
                .file_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
            else {
                return Ok(());
            };
            PathBuf::from(path)
        };
        let target = canonical_or_original(&target);
        let accessible = self.accessible_paths()?;
        if accessible
            .iter()
            .map(PathBuf::from)
            .map(|root| canonical_or_original(&root))
            .any(|root| target == root)
        {
            return Err("拒绝删除授权目录根，已保留任务文件".to_string());
        }

        for other in self
            .download_tasks
            .list()?
            .into_iter()
            .filter(|other| other.id != task.id)
        {
            let Some(other_target) = (if matches!(
                other.source_type,
                DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
            ) {
                Some(PathBuf::from(task_download_dir(&other)))
            } else {
                other
                    .file_path
                    .as_deref()
                    .filter(|path| !path.trim().is_empty())
                    .map(PathBuf::from)
            }) else {
                continue;
            };
            let other_target = canonical_or_original(&other_target);
            if target == other_target
                || (matches!(
                    task.source_type,
                    DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
                ) && (target.starts_with(&other_target) || other_target.starts_with(&target)))
            {
                return Err(
                    "task_shared_resource_conflict：文件由其他任务引用，已保留文件".to_string(),
                );
            }
        }
        Ok(())
    }
}

fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn remove_magnet_metadata_dir(app_data_dir: &Path, task: &DownloadTask) {
    // 磁链临时目录只按任务 ID 定位；稳定恢复 metadata 位于另一棵私有目录，不随临时目录清理。
    let expected_dir = magnet::magnet_metadata_task_dir(app_data_dir, task.id);
    let Ok(metadata) = fs::symlink_metadata(&expected_dir) else {
        return;
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return;
    }
    let _ = fs::remove_dir_all(expected_dir);
}
