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
        let mut prepared = prepare_task_with_logs(payload, self.debug_logs)?;
        let task = if prepared.source_type == DownloadTaskSourceType::Magnet {
            let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed);
            let metadata_dir = magnet_metadata_task_dir(self.app_data_dir, task_id);
            fs::create_dir_all(&metadata_dir).map_err(|error| {
                format!(
                    "创建磁链 metadata 临时目录失败：{}（{}）",
                    metadata_dir.display(),
                    error
                )
            })?;
            prepared.aria2_save_dir = Some(metadata_dir.display().to_string());
            let gid = match add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await {
                Ok(gid) => gid,
                Err(error) => {
                    let _ = fs::remove_dir_all(&metadata_dir);
                    return Err(error);
                }
            };
            store_created_task_with_id(self.download_tasks, task_id, prepared, gid)?
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
        let gid =
            match add_torrent_to_aria2(config, &prepared, &torrent_data, Some(self.debug_logs))
                .await
            {
                Ok(gid) => gid,
                Err(error) => {
                    cleanup_empty_torrent_task_dir(&prepared);
                    return Err(error);
                }
            };
        let task = store_created_task(self.download_tasks, self.next_task_id, prepared, gid)?;
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
