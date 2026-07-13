use super::*;

impl<'a> TaskService<'a> {
    pub async fn delete_download_task(
        &self,
        config: &Aria2Config,
        task_id: u64,
        delete_files: bool,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let task_before_delete = task_snapshot(self.download_tasks, task_id)?;
        let gid = task_before_delete
            .gid
            .clone()
            .filter(|gid| !gid.trim().is_empty());
        if let Some(gid) = gid.as_deref() {
            if let Err(error) = remove_task(config, gid, Some(self.debug_logs)).await {
                if is_stale_aria2_gid_error(&error) {
                    self.debug_logs.warn(
                        "tasks.control",
                        format!(
                            "删除任务时 Aria2 已无此 GID，继续删除本地任务记录，ID {}，GID {}：{}",
                            task_id, gid, error
                        ),
                    );
                } else {
                    return Err(error);
                }
            }
        }
        remove_magnet_metadata_dir(self.app_data_dir, &task_before_delete);
        let task = mark_task_removed(self.download_tasks, task_id, delete_files)?;
        query::sync_task_to_database(self, &task).await?;
        self.debug_logs.info(
            "tasks.control",
            format!(
                "任务已删除，ID {}，GID {}，删除本地文件 {}",
                task_id,
                gid.as_deref().unwrap_or("-"),
                if delete_files { "是" } else { "否" }
            ),
        );
        Ok(task)
    }

    pub async fn permanently_delete_removed_task(&self, task_id: u64) -> Result<(), String> {
        self.ensure_not_exiting()?;
        let task = task_snapshot(self.download_tasks, task_id)?;
        if task.status != DownloadTaskStatus::Removed {
            return Err("只有已删除任务可以永久删除".to_string());
        }

        if !self.repository.delete_task_record(task_id).await? {
            return Err(format!("下载任务不存在：{}", task_id));
        }
        remove_task_record(self.download_tasks, task_id)?;
        self.debug_logs.info(
            "tasks.control",
            format!("已永久删除回收站任务记录，ID {}", task_id),
        );
        Ok(())
    }
}

pub(super) fn remove_magnet_metadata_dir(app_data_dir: &Path, task: &DownloadTask) {
    // 磁链清理只允许删除按任务 ID 分配的应用私有 metadata 目录，记录中的种子路径不匹配时立即放弃。
    let expected_dir = magnet::magnet_metadata_task_dir(app_data_dir, task.id);
    if !expected_dir.exists() {
        return;
    }
    let Ok(expected_dir) = expected_dir.canonicalize() else {
        return;
    };
    if let Some(metadata_torrent_path) = task.metadata_torrent_path.as_deref() {
        if let Some(dir) = Path::new(metadata_torrent_path).parent() {
            if let Ok(dir) = dir.canonicalize() {
                if dir != expected_dir {
                    return;
                }
            }
        }
    }
    let _ = fs::remove_dir_all(expected_dir);
}
