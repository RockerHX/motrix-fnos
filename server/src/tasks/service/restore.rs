use super::*;
use crate::tasks::PreparedDownloadTask;

impl<'a> TaskService<'a> {
    pub async fn restore_removed_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
        use_proxy_override: Option<bool>,
        restart_from_zero: bool,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        self.ensure_task_engine_compatible(config, task_id)?;
        let _proxy_update_guard = self.proxy_update_lock.lock().await;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.source_type != DownloadTaskSourceType::Url && use_proxy_override.is_some() {
            return Err(super::proxy::BT_TASK_PROXY_UNSUPPORTED_MESSAGE.to_string());
        }
        if snapshot.status != DownloadTaskStatus::Removed {
            return Err("只有回收站任务可以恢复".to_string());
        }
        self.ensure_file_cleanup_not_pending(task_id).await?;
        self.validate_task_authorization(&snapshot)?;
        let native = snapshot.engine_generation == crate::config::aria2::EngineGeneration::NativeV2;
        if restart_from_zero && !snapshot.files_deleted {
            return Err("task_restore_options_invalid：仅已删除内容的任务可以从头下载".to_string());
        }
        if snapshot.files_deleted && !restart_from_zero {
            return Err(
                "task_restart_confirmation_required：下载内容已删除，请确认从头下载".to_string(),
            );
        }
        let completed_history = !snapshot.files_deleted
            && snapshot.removed_from_status == Some(DownloadTaskStatus::Complete);
        if native && !completed_history && snapshot.removed_from_status.is_none() {
            return Err(
                "task_resume_state_invalid：回收站任务缺少原始状态，已保留文件".to_string(),
            );
        }
        if !completed_history && snapshot.source_type != DownloadTaskSourceType::Url {
            self.ensure_bt_proxy_ready(config).await?;
        }
        let resolved_proxy = self
            .resolve_recreated_task_proxy(&snapshot, use_proxy_override)
            .await?;
        let mut task_to_restore = snapshot.clone();
        task_to_restore.use_proxy = resolved_proxy.use_proxy;
        task_to_restore.proxy_binding = resolved_proxy.binding;
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Restore,
                "prepared",
                task_operation_context(
                    Some(snapshot.clone()),
                    vec![task_download_dir(&snapshot).to_string()],
                ),
            )
            .await?;
        let retained = completed_history || (native && !restart_from_zero);
        let mut created_gid = None;
        let result = async {
            let (gid, reparsing_base_dir) = if retained {
                if !completed_history {
                    task_to_restore = self
                        .pause_retained_task(config, &task_to_restore, &mut operation)
                        .await?;
                    self.validate_task_authorization(&task_to_restore)?;
                    reconcile_task_proxy_option(
                        self.aria2_rpc,
                        config,
                        &task_to_restore,
                        None,
                        Some(self.debug_logs),
                    )
                    .await
                    .map_err(|error| error.to_string())?;
                    let mut context = operation.context.clone();
                    context
                        .completed_side_effects
                        .push("task_proxy_reconciled".to_string());
                    self.update_task_operation(&mut operation, "aria2_proxy_reconciled", context)
                        .await?;
                }
                (task_to_restore.gid.clone(), None)
            } else {
                let (gid, base_dir) = match snapshot.source_type {
                    DownloadTaskSourceType::Url => self
                        .restore_url_task(config, &task_to_restore, &mut operation)
                        .await
                        .map(|gid| (gid, None))?,
                    DownloadTaskSourceType::Torrent => self
                        .restore_bt_task(config, &task_to_restore, &mut operation)
                        .await
                        .map(|gid| (gid, None))?,
                    DownloadTaskSourceType::Magnet
                        if !native
                            && (snapshot.confirmation_required || snapshot.file_path.is_none()) =>
                    {
                        self.restore_magnet_metadata_task(config, &task_to_restore, &mut operation)
                            .await
                            .map(|(gid, dir)| (gid, Some(dir)))?
                    }
                    DownloadTaskSourceType::Magnet => {
                        match read_saved_torrent_metadata(&task_to_restore) {
                            Ok(data) => self
                                .restore_bt_task_with_data(
                                    config,
                                    &task_to_restore,
                                    &data,
                                    &mut operation,
                                )
                                .await
                                .map(|gid| (gid, None))?,
                            Err(_) if !native => self
                                .restore_magnet_metadata_task(
                                    config,
                                    &task_to_restore,
                                    &mut operation,
                                )
                                .await
                                .map(|(gid, dir)| (gid, Some(dir)))?,
                            Err(_) => {
                                return Err(
                                    "task_metadata_missing：完整 metadata 缺失，已保留回收站记录"
                                        .to_string(),
                                )
                            }
                        }
                    }
                };
                created_gid = Some(gid.clone());
                self.record_aria2_task_created(&mut operation, gid.clone())
                    .await?;
                (Some(gid), base_dir)
            };
            replace_task_snapshot(self.download_tasks, task_to_restore.clone())?;
            let restored = if let Some(base_dir) = reparsing_base_dir {
                mark_magnet_task_reparsing(self.download_tasks, task_id, gid.unwrap(), base_dir)?
            } else {
                mark_task_restored(self.download_tasks, task_id, gid)?
            };
            self.persist_task_with_operation(&restored, &mut operation, "task_restored")
                .await?;
            if native && restart_from_zero {
                crate::aria2::save_session(self.aria2_rpc, config, Some(self.debug_logs)).await?;
                self.release_retained_task(config, &snapshot, &mut operation)
                    .await?;
            }
            Ok(restored)
        }
        .await;
        let restored = match result {
            Ok(task) => task,
            Err(error) => {
                if self.has_unknown_aria2_outcome(&operation) {
                    replace_task_snapshot(self.download_tasks, snapshot.clone())?;
                    self.repository
                        .persist_task_state_with_operation(&snapshot, &operation)
                        .await?;
                    return Err(error);
                }
                if let Some(gid) = created_gid {
                    if let Err(cleanup_error) =
                        remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await
                    {
                        replace_task_snapshot(self.download_tasks, snapshot.clone())?;
                        operation.require_manual_review(
                            "restore_cleanup_failed",
                            format!("{error}；{cleanup_error}"),
                        );
                        self.repository
                            .persist_task_state_with_operation(&snapshot, &operation)
                            .await?;
                        return Err(error);
                    }
                }
                return Err(self
                    .rollback_task_operation_state(
                        snapshot,
                        &mut operation,
                        "restore_failed",
                        error,
                    )
                    .await);
            }
        };
        self.complete_task_operation(&mut operation, "completed")
            .await;
        self.debug_logs
            .info("tasks.restore", format!("回收站任务已恢复，ID {task_id}"));
        Ok(restored)
    }

    async fn restore_url_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<String, String> {
        self.prepare_task_destination(task)?;
        let prepared = restored_task_options(task, task.save_dir.clone());
        self.add_uri_for_task_operation(config, operation, &prepared)
            .await
    }

    async fn restore_bt_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<String, String> {
        let torrent_data = read_saved_torrent_metadata(task).map_err(|error| {
            if task.source_type == DownloadTaskSourceType::Torrent {
                format!("种子任务缺少可恢复的源 metadata：{}", error)
            } else {
                error
            }
        })?;
        self.restore_bt_task_with_data(config, task, &torrent_data, operation)
            .await
    }

    async fn restore_bt_task_with_data(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        torrent_data: &[u8],
        operation: &mut TaskOperation,
    ) -> Result<String, String> {
        let task_dir = task_download_dir(task).to_string();
        self.prepare_task_destination(task)?;
        let mut prepared = restored_task_options(task, task_dir);
        if !task.selected_file_indexes.is_empty() {
            prepared.aria2_options.insert(
                "select-file".to_string(),
                serde_json::json!(task
                    .selected_file_indexes
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")),
            );
        }
        self.add_torrent_for_task_operation(config, operation, &prepared, torrent_data)
            .await
    }

    async fn restore_magnet_metadata_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<(String, String), String> {
        let base_save_dir = if task.file_path.is_some() {
            Path::new(task_download_dir(task))
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(&task.save_dir))
                .display()
                .to_string()
        } else {
            task.save_dir.clone()
        };
        let metadata_dir = magnet::magnet_metadata_task_dir(self.app_data_dir, task.id);
        fs::create_dir_all(&metadata_dir).map_err(|error| {
            format!(
                "创建磁链 metadata 临时目录失败：{}（{}）",
                metadata_dir.display(),
                error
            )
        })?;
        let mut prepared = restored_task_options(task, base_save_dir.clone());
        prepared.aria2_save_dir = Some(metadata_dir.display().to_string());
        match self
            .add_uri_for_task_operation(config, operation, &prepared)
            .await
        {
            Ok(gid) => Ok((gid, base_save_dir)),
            Err(error) => {
                if self.has_unknown_aria2_outcome(operation) {
                    return Err(error);
                }
                let _ = fs::remove_dir_all(metadata_dir);
                Err(error)
            }
        }
    }
}

fn restored_task_options(task: &DownloadTask, save_dir: String) -> PreparedDownloadTask {
    let use_proxy = task.source_type == DownloadTaskSourceType::Url && task.use_proxy;
    PreparedDownloadTask {
        url: task.url.clone(),
        file_name: task.file_name.clone(),
        output_file_name: Some(task.file_name.clone()),
        save_dir,
        aria2_save_dir: None,
        category: task.category.clone(),
        source_type: task.source_type,
        start_mode: DownloadTaskStartMode::Paused,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
        use_proxy,
        proxy_binding: if use_proxy {
            task.proxy_binding.clone()
        } else {
            crate::tasks::TaskProxyBinding::profile(None)
        },
    }
}
