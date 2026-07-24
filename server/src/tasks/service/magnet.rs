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
    match store_created_task_with_id(service.download_tasks, task_id, prepared, gid) {
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
        let mut options = serde_json::Map::new();
        options.insert("select-file".to_string(), serde_json::json!(select_file));
        let prepared = match prepare_bt_download_task_with_logs(
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
}

pub(super) fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}
