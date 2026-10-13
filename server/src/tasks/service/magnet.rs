use super::*;

pub(super) async fn create_magnet_download_task(
    service: &TaskService<'_>,
    config: &Aria2Config,
    task_id: u64,
    mut prepared: crate::tasks::PreparedDownloadTask,
    operation: &mut TaskOperation,
) -> Result<DownloadTask, String> {
    let metadata_dir = magnet_metadata_task_dir(service.app_data_dir, task_id);
    if let Err(error) = fs::create_dir_all(&metadata_dir) {
        let error = format!(
            "创建磁链 metadata 临时目录失败：{}（{}）",
            metadata_dir.display(),
            error
        );
        service
            .fail_task_operation(operation, "metadata_dir_create_failed", &error)
            .await;
        return Err(error);
    }
    prepared.aria2_save_dir = Some(metadata_dir.display().to_string());
    let mut context = operation.context.clone();
    context
        .completed_side_effects
        .push("magnet_metadata_dir_created".to_string());
    if let Err(error) = service
        .update_task_operation(operation, "metadata_dir_created", context)
        .await
    {
        let _ = fs::remove_dir_all(&metadata_dir);
        service
            .fail_task_operation(operation, "metadata_record_failed", &error)
            .await;
        return Err(error);
    }
    let gid = match service
        .add_uri_for_task_operation(config, operation, &prepared)
        .await
    {
        Ok(gid) => gid,
        Err(error) => {
            if service.has_unknown_aria2_outcome(operation) {
                return Err(error);
            }
            let _ = fs::remove_dir_all(&metadata_dir);
            service
                .fail_task_operation(operation, "aria2_failed", &error)
                .await;
            return Err(error);
        }
    };
    if let Err(error) = service
        .record_aria2_task_created(operation, gid.clone())
        .await
    {
        let _ = remove_task(service.aria2_rpc, config, &gid, Some(service.debug_logs)).await;
        let _ = fs::remove_dir_all(&metadata_dir);
        service
            .fail_task_operation(operation, "aria2_record_failed", &error)
            .await;
        return Err(error);
    }
    match store_created_task_with_id(
        service.download_tasks,
        task_id,
        prepared,
        gid,
        config.engine_generation(),
    ) {
        Ok(task) => Ok(task),
        Err(error) => {
            let gid = operation.context.new_gid.as_deref().unwrap_or_default();
            let _ = remove_task(service.aria2_rpc, config, gid, Some(service.debug_logs)).await;
            let _ = fs::remove_dir_all(&metadata_dir);
            service
                .fail_task_operation(operation, "memory_state_failed", &error)
                .await;
            Err(error)
        }
    }
}

impl<'a> TaskService<'a> {
    pub async fn confirm_download_task_files(
        &self,
        config: &Aria2Config,
        task_id: u64,
        selected_file_indexes: Vec<u32>,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let _proxy_update_guard = self.proxy_update_lock.lock().await;
        let _operation = self.download_tasks.begin_operation(task_id)?;
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
            if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
                && task.source_type == DownloadTaskSourceType::Magnet
                && task.selected_file_indexes == selected
            {
                return Ok(task);
            }
            if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
                && task.source_type == DownloadTaskSourceType::Magnet
            {
                return Err("task_already_confirmed：磁链任务已确认，不能改变文件选择".to_string());
            }
            return Err("当前任务不需要确认文件".to_string());
        }
        self.ensure_bt_proxy_ready(config).await?;
        self.validate_task_authorization(&task)?;
        if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
            && task.source_type == DownloadTaskSourceType::Magnet
        {
            return self
                .confirm_native_magnet_download_task(config, task, selected)
                .await;
        }

        let proxy_binding = self.resolve_existing_task_proxy(&task).await?;
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

        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Confirm,
                "prepared",
                task_operation_context(
                    Some(task.clone()),
                    vec![
                        task.save_dir.clone(),
                        magnet_metadata_task_dir(self.app_data_dir, task_id)
                            .display()
                            .to_string(),
                    ],
                ),
            )
            .await?;
        let torrent_data = match read_saved_torrent_metadata(&task) {
            Ok(data) => data,
            Err(error) => {
                self.fail_task_operation(&mut operation, "metadata_read_failed", &error)
                    .await;
                return Err(error);
            }
        };
        let restore_metadata_path =
            match save_restore_torrent_metadata(self.app_data_dir, task_id, &torrent_data) {
                Ok(path) => path,
                Err(error) => {
                    self.fail_task_operation(&mut operation, "metadata_save_failed", &error)
                        .await;
                    return Err(error);
                }
            };
        let mut metadata_context = operation.context.clone();
        metadata_context
            .critical_paths
            .push(restore_metadata_path.display().to_string());
        metadata_context
            .completed_side_effects
            .push("restore_metadata_saved".to_string());
        if let Err(error) = self
            .update_task_operation(&mut operation, "metadata_saved", metadata_context)
            .await
        {
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "metadata_record_failed", &error)
                .await;
            return Err(error);
        }
        if let Err(error) = self.validate_task_authorization(&task) {
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "authorization_revoked", &error)
                .await;
            return Err(error);
        }
        let mut options = serde_json::Map::new();
        options.insert("select-file".to_string(), serde_json::json!(select_file));
        if let Err(error) = self.prepare_task_destination(&task) {
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "authorization_revoked", &error)
                .await;
            return Err(error);
        }
        let mut prepared = match prepare_bt_download_task_with_logs(
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
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.fail_task_operation(&mut operation, "prepare_failed", &error)
                    .await;
                return Err(error);
            }
        };
        prepared.use_proxy = false;
        prepared.proxy_binding = proxy_binding;
        let gid = match self
            .add_torrent_for_task_operation(config, &mut operation, &prepared, &torrent_data)
            .await
        {
            Ok(gid) => gid,
            Err(error) => {
                if self.has_unknown_aria2_outcome(&operation) {
                    return Err(error);
                }
                cleanup_empty_torrent_task_dir(&prepared);
                remove_restore_metadata(self.app_data_dir, task_id);
                self.fail_task_operation(&mut operation, "aria2_failed", &error)
                    .await;
                return Err(error);
            }
        };
        if let Err(error) = self
            .record_aria2_task_created(&mut operation, gid.clone())
            .await
        {
            let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
            cleanup_empty_torrent_task_dir(&prepared);
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "aria2_record_failed", &error)
                .await;
            return Err(error);
        }
        let mut confirmed_task = match mark_task_files_confirmed(
            self.download_tasks,
            task_id,
            gid.clone(),
            prepared.save_dir.clone(),
            &selected,
            restore_metadata_path.display().to_string(),
            prepared.proxy_binding.clone(),
        ) {
            Ok(task) => task,
            Err(error) => {
                let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
                cleanup_empty_torrent_task_dir(&prepared);
                remove_restore_metadata(self.app_data_dir, task_id);
                self.fail_task_operation(&mut operation, "memory_state_failed", &error)
                    .await;
                return Err(error);
            }
        };

        match sync_task_progress_from_aria2_by_gid(
            self.aria2_rpc,
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await
        {
            Ok(synced_task) => confirmed_task = synced_task,
            Err(error) => self.debug_logs.warn(
                "tasks.control",
                format!(
                    "确认文件后同步最新进度失败，使用最后已知进度，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            ),
        }
        if let Err(error) = self
            .persist_task_with_operation(&confirmed_task, &mut operation, "task_confirmed")
            .await
        {
            let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
            cleanup_empty_torrent_task_dir(&prepared);
            replace_task_snapshot(self.download_tasks, task.clone())?;
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "task_persist_failed", &error)
                .await;
            return Err(error);
        }
        delete::remove_magnet_metadata_dir(self.app_data_dir, &task);
        operation
            .context
            .completed_side_effects
            .push("magnet_metadata_removed".to_string());
        self.complete_task_operation(&mut operation, "completed")
            .await;
        self.debug_logs.info(
            "tasks.control",
            format!("任务文件已确认并开始下载，ID {}，GID {}", task_id, gid),
        );
        Ok(confirmed_task)
    }

    async fn confirm_native_magnet_download_task(
        &self,
        config: &Aria2Config,
        task: DownloadTask,
        selected: Vec<u32>,
    ) -> Result<DownloadTask, String> {
        let gid = task
            .gid
            .as_deref()
            .filter(|gid| !gid.trim().is_empty())
            .ok_or_else(|| "task_resume_state_invalid：磁链确认缺少原 GID".to_string())?;
        let status = crate::tasks::tell_status(self.aria2_rpc, config, gid, Some(self.debug_logs))
            .await
            .map_err(|error| {
                if crate::tasks::is_stale_aria2_gid_error(&error) {
                    format!("task_resume_state_invalid：磁链原 GID 已丢失：{error}")
                } else {
                    format!("读取磁链 metadata 状态失败：{error}")
                }
            })?;
        let selection_state = status
            .bittorrent
            .as_ref()
            .and_then(|bittorrent| bittorrent.file_selection_state.as_deref());
        if status.gid.as_deref() != Some(gid)
            || status.status != "paused"
            || selection_state != Some("awaiting")
        {
            return Err("task_resume_state_invalid：磁链 metadata 尚未处于待确认状态".to_string());
        }
        let available = status
            .files
            .as_ref()
            .into_iter()
            .flatten()
            .map(|file| file.index)
            .collect::<std::collections::BTreeSet<_>>();
        if !selected
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .is_subset(&available)
        {
            return Err("选择的文件索引不在 Aria2 实际文件列表中".to_string());
        }
        let display_name = status
            .bittorrent
            .as_ref()
            .and_then(|bittorrent| bittorrent.info.as_ref())
            .and_then(|info| info.name.as_deref())
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&task.file_name);
        let mut operation = self
            .begin_task_operation(
                task.id,
                TaskOperationType::Confirm,
                "prepared",
                task_operation_context(
                    Some(task.clone()),
                    vec![magnet_metadata_task_dir(self.app_data_dir, task.id)
                        .display()
                        .to_string()],
                ),
            )
            .await?;
        let final_dir = match crate::tasks::prepare::create_bt_task_dir(
            &task.save_dir,
            display_name,
            "磁链",
            Some(self.debug_logs),
        ) {
            Ok(path) => path,
            Err(error) => {
                self.fail_task_operation(&mut operation, "destination_prepare_failed", &error)
                    .await;
                return Err(error);
            }
        };
        operation.context.critical_paths.push(final_dir.clone());
        let operation_context = operation.context.clone();
        if let Err(error) = self
            .update_task_operation(&mut operation, "destination_prepared", operation_context)
            .await
        {
            let _ = fs::remove_dir_all(&final_dir);
            self.fail_task_operation(&mut operation, "destination_record_failed", &error)
                .await;
            return Err(error);
        }
        let select_file = selected
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let options = serde_json::Map::from_iter([
            ("dir".to_string(), serde_json::json!(final_dir)),
            ("select-file".to_string(), serde_json::json!(select_file)),
        ]);
        if let Err(error) = crate::tasks::change_task_options_with_request_id(
            self.aria2_rpc,
            config,
            gid,
            options,
            Some(&operation.id),
            Some(self.debug_logs),
        )
        .await
        {
            let error = error.to_string();
            if error.contains("结果未知") {
                self.record_unknown_aria2_outcome(&mut operation, error.clone())
                    .await?;
            } else {
                let _ = fs::remove_dir_all(&final_dir);
                self.fail_task_operation(&mut operation, "change_option_failed", &error)
                    .await;
            }
            return Err(error);
        }
        let status =
            match crate::tasks::tell_status(self.aria2_rpc, config, gid, Some(self.debug_logs))
                .await
            {
                Ok(status) => status,
                Err(error) => {
                    let error = format!("确认磁链文件选择后读取状态失败：{error}");
                    self.record_unknown_aria2_outcome(&mut operation, error.clone())
                        .await?;
                    return Err(error);
                }
            };
        let selection_state = status
            .bittorrent
            .as_ref()
            .and_then(|bittorrent| bittorrent.file_selection_state.as_deref());
        let selected_actual = status
            .files
            .as_ref()
            .into_iter()
            .flatten()
            .filter(|file| file.selected == "true")
            .map(|file| file.index)
            .collect::<std::collections::BTreeSet<_>>();
        if status.gid.as_deref() != Some(gid)
            || status.status != "paused"
            || status.dir.as_deref() != Some(final_dir.as_str())
            || selection_state != Some("ready")
            || selected_actual != selected.iter().copied().collect()
        {
            self.record_unknown_aria2_outcome(
                &mut operation,
                "确认磁链文件选择后原生状态核对失败，任务保持暂停".to_string(),
            )
            .await?;
            return Err("确认磁链文件选择后原生状态核对失败，任务保持暂停".to_string());
        }
        let proxy_binding = crate::tasks::TaskProxyBinding::profile(None);
        let mut confirmed_task = match mark_task_files_confirmed(
            self.download_tasks,
            task.id,
            gid.to_string(),
            final_dir,
            &selected,
            String::new(),
            proxy_binding,
        ) {
            Ok(task) => task,
            Err(error) => {
                self.record_unknown_aria2_outcome(&mut operation, error.clone())
                    .await?;
                return Err(error);
            }
        };
        crate::tasks::progress::apply_aria2_status(&mut confirmed_task, &status);
        confirmed_task.confirmation_required = false;
        confirmed_task.metadata_torrent_path = None;
        confirmed_task.status = DownloadTaskStatus::Paused;
        confirmed_task.selected_file_indexes = selected.clone();
        replace_task_snapshot(self.download_tasks, confirmed_task.clone())?;
        if let Err(error) = self
            .persist_task_with_operation(&confirmed_task, &mut operation, "task_confirmed")
            .await
        {
            return Err(self
                .rollback_task_operation_state(
                    task.clone(),
                    &mut operation,
                    "task_persist_failed",
                    error,
                )
                .await);
        }
        if let Err(error) = crate::tasks::unpause_task_with_request_id(
            self.aria2_rpc,
            config,
            gid,
            Some(&operation.id),
            Some(self.debug_logs),
        )
        .await
        {
            if error.contains("结果未知") {
                self.record_unknown_aria2_outcome(&mut operation, error.clone())
                    .await?;
            } else {
                self.fail_task_operation(&mut operation, "unpause_failed", &error)
                    .await;
            }
            return Err(error);
        }
        let final_status =
            crate::tasks::tell_status(self.aria2_rpc, config, gid, Some(self.debug_logs)).await;
        if let Ok(final_status) = final_status {
            let mut running_task = confirmed_task.clone();
            crate::tasks::progress::apply_aria2_status(&mut running_task, &final_status);
            running_task.confirmation_required = false;
            running_task.metadata_torrent_path = None;
            confirmed_task = running_task;
            replace_task_snapshot(self.download_tasks, confirmed_task.clone())?;
            if let Err(error) = self
                .persist_task_with_operation(&confirmed_task, &mut operation, "task_started")
                .await
            {
                self.fail_task_operation(&mut operation, "task_persist_failed", &error)
                    .await;
                return Err(error);
            }
        } else if let Err(error) = final_status {
            self.record_unknown_aria2_outcome(&mut operation, error.clone())
                .await?;
            return Err(error);
        }
        delete::remove_magnet_metadata_dir(self.app_data_dir, &task);
        self.complete_task_operation(&mut operation, "completed")
            .await;
        Ok(confirmed_task)
    }
}

pub(super) fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}
