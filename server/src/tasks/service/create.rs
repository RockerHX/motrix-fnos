use super::*;

impl<'a> TaskService<'a> {
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
        let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let mut critical_paths = vec![prepared.save_dir.clone()];
        if prepared.source_type == DownloadTaskSourceType::Magnet {
            critical_paths.push(
                magnet::magnet_metadata_task_dir(self.app_data_dir, task_id)
                    .display()
                    .to_string(),
            );
        }
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Create,
                "prepared",
                task_operation_context(None, critical_paths),
            )
            .await?;
        let task = if prepared.source_type == DownloadTaskSourceType::Magnet {
            magnet::create_magnet_download_task(self, config, task_id, prepared, &mut operation)
                .await?
        } else {
            let source_type = prepared.source_type;
            let gid = match add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await {
                Ok(gid) => gid,
                Err(error) => {
                    if source_type == DownloadTaskSourceType::Torrent {
                        cleanup_empty_torrent_task_dir(&prepared);
                    }
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
                if source_type == DownloadTaskSourceType::Torrent {
                    cleanup_empty_torrent_task_dir(&prepared);
                }
                self.fail_task_operation(&mut operation, "aria2_record_failed", &error)
                    .await;
                return Err(error);
            }
            match store_created_task_with_id(self.download_tasks, task_id, prepared, gid) {
                Ok(task) => task,
                Err(error) => {
                    let gid = operation.context.new_gid.as_deref().unwrap_or_default();
                    let _ = remove_task(config, gid, Some(self.debug_logs)).await;
                    self.fail_task_operation(&mut operation, "memory_state_failed", &error)
                        .await;
                    return Err(error);
                }
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_persisted")
            .await
        {
            if let Some(gid) = task.gid.as_deref() {
                let _ = remove_task(config, gid, Some(self.debug_logs)).await;
            }
            let _ = remove_task_record(self.download_tasks, task_id);
            delete::remove_magnet_metadata_dir(self.app_data_dir, &task);
            self.fail_task_operation(&mut operation, "task_persist_failed", &error)
                .await;
            return Err(error);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
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

    pub async fn create_torrent_download_task(
        &self,
        config: &Aria2Config,
        payload: CreateTorrentDownloadTaskRequest,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        if payload.save_dir.trim().is_empty() {
            return Err("请选择已授权的保存目录".to_string());
        }
        let torrent_data = payload.torrent_data.clone();
        let prepared = prepare_torrent_task_with_logs(payload, self.debug_logs)?;
        let prepared_for_cleanup = prepared.clone();
        let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let _operation = self.download_tasks.begin_operation(task_id)?;
        let mut operation = self
            .begin_task_operation(
                task_id,
                TaskOperationType::Create,
                "prepared",
                task_operation_context(None, vec![prepared.save_dir.clone()]),
            )
            .await?;
        let metadata_path =
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
            .push(metadata_path.display().to_string());
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
        let gid =
            match add_torrent_to_aria2(config, &prepared, &torrent_data, Some(self.debug_logs))
                .await
            {
                Ok(gid) => gid,
                Err(error) => {
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
            let _ = remove_task(config, &gid, Some(self.debug_logs)).await;
            cleanup_empty_torrent_task_dir(&prepared);
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "aria2_record_failed", &error)
                .await;
            return Err(error);
        }
        if let Err(error) = store_created_task_with_id(self.download_tasks, task_id, prepared, gid)
        {
            let gid = operation.context.new_gid.as_deref().unwrap_or_default();
            let _ = remove_task(config, gid, Some(self.debug_logs)).await;
            cleanup_empty_torrent_task_dir(&prepared_for_cleanup);
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "memory_state_failed", &error)
                .await;
            return Err(error);
        }
        let task = match set_task_metadata_torrent_path(
            self.download_tasks,
            task_id,
            metadata_path.display().to_string(),
        ) {
            Ok(task) => task,
            Err(error) => {
                let gid = operation.context.new_gid.as_deref().unwrap_or_default();
                let _ = remove_task(config, gid, Some(self.debug_logs)).await;
                let _ = remove_task_record(self.download_tasks, task_id);
                cleanup_empty_torrent_task_dir(&prepared_for_cleanup);
                remove_restore_metadata(self.app_data_dir, task_id);
                self.fail_task_operation(&mut operation, "memory_state_failed", &error)
                    .await;
                return Err(error);
            }
        };
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "task_persisted")
            .await
        {
            if let Some(gid) = task.gid.as_deref() {
                let _ = remove_task(config, gid, Some(self.debug_logs)).await;
            }
            let _ = remove_task_record(self.download_tasks, task_id);
            cleanup_empty_torrent_task_dir(&prepared_for_cleanup);
            remove_restore_metadata(self.app_data_dir, task_id);
            self.fail_task_operation(&mut operation, "task_persist_failed", &error)
                .await;
            return Err(error);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
        self.debug_logs.info(
            "tasks.create",
            format!(
                "种子任务已写入内存列表和 SQLite，ID {}，GID {}",
                task.id,
                task.gid.as_deref().unwrap_or("-")
            ),
        );
        Ok(task)
    }
}
