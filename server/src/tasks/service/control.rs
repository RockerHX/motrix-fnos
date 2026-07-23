use super::*;
use crate::tasks::files::{stage_task_files, StagedTaskFiles};

impl<'a> TaskService<'a> {
    pub async fn pause_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        let gid = task_gid(self.download_tasks, task_id)?;
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Pause,
                "prepared",
                task_operation_context(Some(snapshot.clone()), Vec::new()),
            )
            .await?;
        if let Err(error) = pause_task(config, &gid, Some(self.debug_logs)).await {
            self.fail_task_operation(&mut operation, "aria2_pause_failed", &error)
                .await;
            return Err(error);
        }
        let mut pause_context = operation.context.clone();
        pause_context
            .completed_side_effects
            .push("aria2_task_paused".to_string());
        if let Err(error) = self
            .update_task_operation(&mut operation, "aria2_paused", pause_context)
            .await
        {
            let _ = unpause_task(config, &gid, Some(self.debug_logs)).await;
            return Err(self
                .rollback_task_operation_state(
                    snapshot,
                    &mut operation,
                    "aria2_record_failed",
                    error,
                )
                .await);
        }
        let task = match sync_task_progress_after_pause_by_gid(
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await
        {
            Ok(task) => task,
            Err(error) => {
                let _ = unpause_task(config, &gid, Some(self.debug_logs)).await;
                return Err(self
                    .rollback_task_operation_state(snapshot, &mut operation, "sync_failed", error)
                    .await);
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_paused")
            .await
        {
            let _ = unpause_task(config, &gid, Some(self.debug_logs)).await;
            return Err(self
                .rollback_task_operation_state(
                    snapshot,
                    &mut operation,
                    "task_persist_failed",
                    error,
                )
                .await);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
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
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let task_before_resume = task_snapshot(self.download_tasks, task_id)?;
        if task_before_resume.confirmation_required {
            return Err("请先确认要下载的文件".to_string());
        }
        let gid = task_gid(self.download_tasks, task_id)?;
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Resume,
                "prepared",
                task_operation_context(Some(task_before_resume.clone()), Vec::new()),
            )
            .await?;
        let mut readded = false;
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
                readded = true;
                match readd_task_to_aria2(
                    self.download_tasks,
                    config,
                    task_id,
                    Some(self.debug_logs),
                )
                .await
                {
                    Ok(task) => task,
                    Err(error) => {
                        self.fail_task_operation(&mut operation, "aria2_readd_failed", &error)
                            .await;
                        return Err(error);
                    }
                }
            }
            Err(error) => {
                self.fail_task_operation(&mut operation, "aria2_resume_failed", &error)
                    .await;
                return Err(error);
            }
        };
        let mut resume_context = operation.context.clone();
        if readded {
            resume_context.new_gid = task.gid.clone();
            resume_context
                .completed_side_effects
                .push("aria2_task_readded".to_string());
        } else {
            resume_context
                .completed_side_effects
                .push("aria2_task_resumed".to_string());
        }
        if let Err(error) = self
            .update_task_operation(&mut operation, "aria2_resumed", resume_context)
            .await
        {
            if readded {
                if let Some(new_gid) = task.gid.as_deref() {
                    let _ = remove_task(config, new_gid, Some(self.debug_logs)).await;
                }
            } else {
                let _ = pause_task(config, &gid, Some(self.debug_logs)).await;
            }
            return Err(self
                .rollback_task_operation_state(
                    task_before_resume,
                    &mut operation,
                    "aria2_record_failed",
                    error,
                )
                .await);
        }
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_resumed")
            .await
        {
            if readded {
                if let Some(new_gid) = task.gid.as_deref() {
                    let _ = remove_task(config, new_gid, Some(self.debug_logs)).await;
                }
            } else {
                let _ = pause_task(config, &gid, Some(self.debug_logs)).await;
            }
            return Err(self
                .rollback_task_operation_state(
                    task_before_resume,
                    &mut operation,
                    "task_persist_failed",
                    error,
                )
                .await);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
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
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.status != DownloadTaskStatus::Complete {
            return Err("只有已完成任务可以重新下载".to_string());
        }

        validate_task_files(&snapshot)?;
        let torrent_data = match snapshot.source_type {
            DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet => Some(
                read_saved_torrent_metadata(&snapshot)
                    .map_err(|error| format!("重新下载前无法读取源 metadata：{}", error))?,
            ),
            DownloadTaskSourceType::Url => None,
        };
        let prepared = crate::tasks::PreparedDownloadTask {
            url: snapshot.url.clone(),
            file_name: snapshot.file_name.clone(),
            output_file_name: Some(snapshot.file_name.clone()),
            save_dir: task_download_dir(&snapshot).to_string(),
            aria2_save_dir: None,
            category: snapshot.category.clone(),
            source_type: snapshot.source_type,
            start_mode: DownloadTaskStartMode::Paused,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        };
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Redownload,
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
        let gid_result = match snapshot.source_type {
            DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet => {
                add_torrent_to_aria2(
                    config,
                    &prepared,
                    torrent_data
                        .as_deref()
                        .ok_or_else(|| "重新下载缺少源 metadata".to_string())?,
                    Some(self.debug_logs),
                )
                .await
            }
            DownloadTaskSourceType::Url => {
                add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await
            }
        };
        let gid = match gid_result {
            Ok(gid) => gid,
            Err(error) => {
                self.fail_task_operation(&mut operation, "aria2_failed", &error)
                    .await;
                return Err(error);
            }
        };
        if let Err(error) = self
            .record_aria2_task_created(&mut operation, gid.clone())
            .await
        {
            let _ = remove_task(config, &gid, Some(self.debug_logs)).await;
            self.fail_task_operation(&mut operation, "aria2_record_failed", &error)
                .await;
            return Err(error);
        }

        let pending = match mark_task_redownloaded(self.download_tasks, task_id, gid.clone()) {
            Ok(task) => task,
            Err(error) => {
                return self
                    .rollback_redownload(config, snapshot, gid, None, &mut operation, error)
                    .await;
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&pending, &mut operation, "new_task_persisted")
            .await
        {
            return self
                .rollback_redownload(config, snapshot, gid, None, &mut operation, error)
                .await;
        }

        let staged = match stage_task_files(&snapshot) {
            Ok(staged) => staged,
            Err(error) => {
                return self
                    .rollback_redownload(config, snapshot, gid, None, &mut operation, error)
                    .await;
            }
        };
        if let Some(staged_files) = staged.as_ref() {
            let mut context = operation.context.clone();
            context
                .critical_paths
                .push(staged_files.backup_dir().display().to_string());
            context
                .completed_side_effects
                .push("old_files_staged".to_string());
            if let Err(error) = self
                .update_task_operation(&mut operation, "files_staged", context)
                .await
            {
                return self
                    .rollback_redownload(config, snapshot, gid, staged, &mut operation, error)
                    .await;
            }
        }

        if matches!(
            snapshot.source_type,
            DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet
        ) {
            if let Err(error) = fs::create_dir_all(task_download_dir(&snapshot)) {
                return self
                    .rollback_redownload(
                        config,
                        snapshot,
                        gid,
                        staged,
                        &mut operation,
                        format!("重建 BT 任务保存目录失败：{}", error),
                    )
                    .await;
            }
        }

        if let Err(error) = unpause_task(config, &gid, Some(self.debug_logs)).await {
            return self
                .rollback_redownload(config, snapshot, gid, staged, &mut operation, error)
                .await;
        }

        let active = match mark_task_resumed(self.download_tasks, task_id) {
            Ok(task) => task,
            Err(error) => {
                let _ = pause_task(config, &gid, Some(self.debug_logs)).await;
                return self
                    .rollback_redownload(config, snapshot, gid, staged, &mut operation, error)
                    .await;
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&active, &mut operation, "task_resumed")
            .await
        {
            let _ = pause_task(config, &gid, Some(self.debug_logs)).await;
            return self
                .rollback_redownload(config, snapshot, gid, staged, &mut operation, error)
                .await;
        }

        if let Some(staged) = staged {
            match staged.commit() {
                Ok(()) => operation
                    .context
                    .completed_side_effects
                    .push("old_files_cleaned".to_string()),
                Err(error) => {
                    self.debug_logs.warn(
                        "tasks.redownload",
                        format!("重新下载已启动，但旧文件暂存目录清理失败：{}", error),
                    );
                }
            }
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
        self.debug_logs.info(
            "tasks.control",
            format!("任务已重新下载，ID {}，GID {}", task_id, gid),
        );
        Ok(active)
    }

    async fn rollback_redownload(
        &self,
        config: &Aria2Config,
        snapshot: DownloadTask,
        gid: String,
        staged: Option<StagedTaskFiles>,
        operation: &mut TaskOperation,
        reason: String,
    ) -> Result<DownloadTask, String> {
        let remove_error = remove_task(config, &gid, Some(self.debug_logs)).await.err();
        let restore_error = staged.and_then(|staged| staged.restore().err());
        replace_task_snapshot(self.download_tasks, snapshot.clone())?;
        operation.fail("rolled_back", &reason);
        let persist_error = self
            .repository
            .persist_task_state_with_operation(&snapshot, operation)
            .await
            .err();

        let mut errors = vec![reason];
        if let Some(error) = remove_error {
            errors.push(format!("移除新 Aria2 任务失败：{}", error));
        }
        if let Some(error) = restore_error {
            errors.push(format!("恢复原文件失败：{}", error));
        }
        if let Some(error) = persist_error {
            errors.push(format!("恢复数据库任务状态失败：{}", error));
            self.fail_task_operation(operation, "rollback_persist_failed", errors.join("；"))
                .await;
        }
        Err(errors.join("；"))
    }
}
