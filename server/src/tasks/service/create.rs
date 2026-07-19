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
        let task = if prepared.source_type == DownloadTaskSourceType::Magnet {
            magnet::create_magnet_download_task(self, config, prepared).await?
        } else {
            let gid = match add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await {
                Ok(gid) => gid,
                Err(error) => {
                    cleanup_empty_torrent_task_dir(&prepared);
                    return Err(error);
                }
            };
            store_created_task(self.download_tasks, self.next_task_id, prepared, gid)?
        };
        self.repository.upsert_task(&task).await?;
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
        let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let metadata_path =
            save_restore_torrent_metadata(self.app_data_dir, task_id, &torrent_data)?;
        let gid =
            match add_torrent_to_aria2(config, &prepared, &torrent_data, Some(self.debug_logs))
                .await
            {
                Ok(gid) => gid,
                Err(error) => {
                    cleanup_empty_torrent_task_dir(&prepared);
                    remove_restore_metadata(self.app_data_dir, task_id);
                    return Err(error);
                }
            };
        store_created_task_with_id(self.download_tasks, task_id, prepared, gid)?;
        let task = set_task_metadata_torrent_path(
            self.download_tasks,
            task_id,
            metadata_path.display().to_string(),
        )?;
        self.repository.upsert_task(&task).await?;
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
