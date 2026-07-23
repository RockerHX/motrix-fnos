use super::*;

impl<'a> TaskService<'a> {
    pub async fn pause_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let gid = task_gid(self.download_tasks, task_id)?;
        pause_task(config, &gid, Some(self.debug_logs)).await?;
        let task = sync_task_progress_after_pause_by_gid(
            self.download_tasks,
            config,
            &gid,
            Some(self.debug_logs),
        )
        .await?;
        query::sync_task_to_database(self, &task).await?;
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
        let task_before_resume = task_snapshot(self.download_tasks, task_id)?;
        if task_before_resume.confirmation_required {
            return Err("请先确认要下载的文件".to_string());
        }
        let gid = task_gid(self.download_tasks, task_id)?;
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
                readd_task_to_aria2(self.download_tasks, config, task_id, Some(self.debug_logs))
                    .await?
            }
            Err(error) => return Err(error),
        };
        query::sync_task_to_database(self, &task).await?;
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
        let task = task_snapshot(self.download_tasks, task_id)?;
        if task.status != DownloadTaskStatus::Complete {
            return Err("只有已完成任务可以重新下载".to_string());
        }

        delete_task_files(&task)?;
        let prepared = crate::tasks::PreparedDownloadTask {
            url: task.url.clone(),
            file_name: task.file_name.clone(),
            output_file_name: Some(task.file_name.clone()),
            save_dir: task_download_dir(&task).to_string(),
            aria2_save_dir: None,
            category: task.category.clone(),
            source_type: task.source_type,
            start_mode: DownloadTaskStartMode::Now,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        };
        let gid = add_uri_to_aria2(config, &prepared, Some(self.debug_logs)).await?;
        let task = mark_task_redownloaded(self.download_tasks, task_id, gid.clone())?;
        query::sync_task_to_database(self, &task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已重新下载，ID {}，GID {}，原本地文件已删除",
                task_id, gid
            ),
        );
        Ok(task)
    }
}
