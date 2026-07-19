use super::*;

pub(super) async fn create_magnet_download_task(
    service: &TaskService<'_>,
    config: &Aria2Config,
    mut prepared: crate::tasks::PreparedDownloadTask,
) -> Result<DownloadTask, String> {
    let task_id = service.next_task_id.fetch_add(1, Ordering::Relaxed);
    let metadata_dir = magnet_metadata_task_dir(service.app_data_dir, task_id);
    fs::create_dir_all(&metadata_dir).map_err(|error| {
        format!(
            "创建磁链 metadata 临时目录失败：{}（{}）",
            metadata_dir.display(),
            error
        )
    })?;
    prepared.aria2_save_dir = Some(metadata_dir.display().to_string());
    let gid = match add_uri_to_aria2(config, &prepared, Some(service.debug_logs)).await {
        Ok(gid) => gid,
        Err(error) => {
            let _ = fs::remove_dir_all(&metadata_dir);
            return Err(error);
        }
    };
    store_created_task_with_id(service.download_tasks, task_id, prepared, gid)
}

impl<'a> TaskService<'a> {
    pub async fn confirm_download_task_files(
        &self,
        config: &Aria2Config,
        task_id: u64,
        selected_file_indexes: Vec<u32>,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
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

        let torrent_data = read_saved_torrent_metadata(&task)?;
        let restore_metadata_path =
            save_restore_torrent_metadata(self.app_data_dir, task_id, &torrent_data)?;
        let mut options = serde_json::Map::new();
        options.insert("select-file".to_string(), serde_json::json!(select_file));
        let prepared = prepare_bt_download_task_with_logs(
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
        )?;
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
        delete::remove_magnet_metadata_dir(self.app_data_dir, &task);
        let mut task = mark_task_files_confirmed(
            self.download_tasks,
            task_id,
            gid.clone(),
            prepared.save_dir.clone(),
            &selected,
            restore_metadata_path.display().to_string(),
        )?;

        match sync_task_progress_from_aria2_by_gid(
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await
        {
            Ok(synced_task) => task = synced_task,
            Err(error) => self.debug_logs.warn(
                "tasks.control",
                format!(
                    "确认文件后同步最新进度失败，使用最后已知进度，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            ),
        }
        query::sync_task_to_database(self, &task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!("任务文件已确认并开始下载，ID {}，GID {}", task_id, gid),
        );
        Ok(task)
    }
}

pub(super) fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}
