use super::*;
use crate::tasks::PreparedDownloadTask;

impl<'a> TaskService<'a> {
    pub async fn restore_removed_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.status != DownloadTaskStatus::Removed {
            return Err("只有回收站任务可以恢复".to_string());
        }

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

        let (gid, reparsing_base_dir) = match snapshot.source_type {
            DownloadTaskSourceType::Url => match self
                .restore_url_task(config, &snapshot, &mut operation)
                .await
            {
                Ok(gid) => (gid, None),
                Err(error) => {
                    if self.has_unknown_aria2_outcome(&operation) {
                        return Err(error);
                    }
                    self.fail_task_operation(&mut operation, "aria2_restore_failed", &error)
                        .await;
                    return Err(error);
                }
            },
            DownloadTaskSourceType::Torrent => {
                match self
                    .restore_bt_task(config, &snapshot, &mut operation)
                    .await
                {
                    Ok(gid) => (gid, None),
                    Err(error) => {
                        if self.has_unknown_aria2_outcome(&operation) {
                            return Err(error);
                        }
                        self.fail_task_operation(&mut operation, "aria2_restore_failed", &error)
                            .await;
                        return Err(error);
                    }
                }
            }
            DownloadTaskSourceType::Magnet
                if snapshot.confirmation_required || snapshot.file_path.is_none() =>
            {
                match self
                    .restore_magnet_metadata_task(config, &snapshot, &mut operation)
                    .await
                {
                    Ok((gid, base_save_dir)) => (gid, Some(base_save_dir)),
                    Err(error) => {
                        if self.has_unknown_aria2_outcome(&operation) {
                            return Err(error);
                        }
                        self.fail_task_operation(&mut operation, "aria2_restore_failed", &error)
                            .await;
                        return Err(error);
                    }
                }
            }
            DownloadTaskSourceType::Magnet => match read_saved_torrent_metadata(&snapshot) {
                Ok(torrent_data) => match self
                    .restore_bt_task_with_data(config, &snapshot, &torrent_data, &mut operation)
                    .await
                {
                    Ok(gid) => (gid, None),
                    Err(error) => {
                        if self.has_unknown_aria2_outcome(&operation) {
                            return Err(error);
                        }
                        self.fail_task_operation(&mut operation, "aria2_restore_failed", &error)
                            .await;
                        return Err(error);
                    }
                },
                Err(_) => match self
                    .restore_magnet_metadata_task(config, &snapshot, &mut operation)
                    .await
                {
                    Ok((gid, base_save_dir)) => (gid, Some(base_save_dir)),
                    Err(error) => {
                        if self.has_unknown_aria2_outcome(&operation) {
                            return Err(error);
                        }
                        self.fail_task_operation(&mut operation, "aria2_restore_failed", &error)
                            .await;
                        return Err(error);
                    }
                },
            },
        };

        if let Err(error) = self
            .record_aria2_task_created(&mut operation, gid.clone())
            .await
        {
            let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
            self.fail_task_operation(&mut operation, "aria2_record_failed", &error)
                .await;
            return Err(error);
        }

        let restored = if let Some(base_save_dir) = reparsing_base_dir {
            match mark_magnet_task_reparsing(
                self.download_tasks,
                task_id,
                gid.clone(),
                base_save_dir,
            ) {
                Ok(task) => task,
                Err(error) => {
                    let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
                    return Err(self
                        .rollback_task_operation_state(
                            snapshot,
                            &mut operation,
                            "memory_state_failed",
                            error,
                        )
                        .await);
                }
            }
        } else {
            match mark_task_restored(self.download_tasks, task_id, gid.clone()) {
                Ok(task) => task,
                Err(error) => {
                    let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
                    return Err(self
                        .rollback_task_operation_state(
                            snapshot,
                            &mut operation,
                            "memory_state_failed",
                            error,
                        )
                        .await);
                }
            }
        };

        if let Err(error) = self
            .persist_task_with_operation(&restored, &mut operation, "task_restored")
            .await
        {
            let _ = remove_task(self.aria2_rpc, config, &gid, Some(self.debug_logs)).await;
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
            "tasks.restore",
            format!("回收站任务已恢复，ID {}，GID {}", task_id, gid),
        );
        Ok(restored)
    }

    async fn restore_url_task(
        &self,
        config: &Aria2Config,
        task: &DownloadTask,
        operation: &mut TaskOperation,
    ) -> Result<String, String> {
        fs::create_dir_all(&task.save_dir)
            .map_err(|error| format!("重建任务保存目录失败：{}（{}）", task.save_dir, error))?;
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
        fs::create_dir_all(&task_dir)
            .map_err(|error| format!("重建任务保存目录失败：{}（{}）", task_dir, error))?;
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
    }
}
