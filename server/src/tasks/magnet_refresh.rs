use super::*;
use crate::aria2::Aria2RpcClient;

pub(super) async fn resolve_followed_metadata(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    metadata_gid: &str,
    metadata_status: &Aria2TaskStatus,
    debug_logs: Option<&DebugLogStore>,
) -> Option<Result<(Aria2TaskStatus, String), String>> {
    // followedBy 指向 metadata 完成后生成的真实任务；先暂停并读取其状态，保存种子路径后再清理两个临时 GID。
    let followed_gid = followed_gid(metadata_status)?;
    if let Err(error) = pause_task(client, config, &followed_gid, debug_logs).await {
        log_info(
            debug_logs,
            "tasks.magnet",
            format!(
                "磁链 metadata 完成后暂停真实任务失败，继续同步状态，GID {}：{}",
                followed_gid, error
            ),
        );
    }

    let followed_status = match tell_status(client, config, &followed_gid, debug_logs).await {
        Ok(status) => status,
        Err(error) => return Some(Err(error)),
    };
    let metadata_dir = match metadata_status
        .dir
        .as_deref()
        .filter(|dir| !dir.trim().is_empty())
    {
        Some(dir) => dir,
        None => return Some(Err("磁链 metadata 解析完成但缺少 metadata 目录".to_string())),
    };

    // 该目录由磁链创建流程分配在应用私有数据区，只定位其中唯一的种子元数据，不能改为拼接用户输入路径。
    let metadata_torrent_path = match find_single_torrent_file(Path::new(metadata_dir)) {
        Ok(path) => path.display().to_string(),
        Err(error) => return Some(Err(error)),
    };

    remove_temporary_magnet_gid(client, config, &followed_gid, debug_logs).await;
    remove_temporary_magnet_gid(client, config, metadata_gid, debug_logs).await;
    log_info(
        debug_logs,
        "tasks.magnet",
        format!(
            "磁链 metadata 已解析完成，临时 GID {}，真实 GID {}，等待用户确认文件",
            metadata_gid, followed_gid
        ),
    );
    Some(Ok((followed_status, metadata_torrent_path)))
}

pub(super) fn stale_magnet_metadata_status(
    app_data_dir: &Path,
    task: &DownloadTask,
) -> Aria2TaskStatus {
    let metadata_dir = magnet_metadata_task_dir(app_data_dir, task.id);
    let message = match find_single_torrent_file(&metadata_dir) {
        Ok(path) => format!(
            "磁链 metadata 解析任务已失效，请重新添加磁链：{}",
            path.display()
        ),
        Err(_) => "磁链 metadata 解析任务已失效，请重新添加磁链".to_string(),
    };
    task_status_error(message)
}

async fn remove_temporary_magnet_gid(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) {
    if let Err(error) = remove_task(client, config, gid, debug_logs).await {
        if is_stale_aria2_gid_error(&error) {
            log_info(
                debug_logs,
                "tasks.magnet",
                format!("临时磁链 GID 已不存在，跳过清理，GID {}：{}", gid, error),
            );
            return;
        }
        log_info(
            debug_logs,
            "tasks.magnet",
            format!("清理临时磁链 GID 失败，GID {}：{}", gid, error),
        );
    }
}

fn followed_gid(status: &Aria2TaskStatus) -> Option<String> {
    status
        .followed_by
        .as_ref()
        .and_then(|gids| gids.first())
        .map(|gid| gid.trim())
        .filter(|gid| !gid.is_empty())
        .map(ToOwned::to_owned)
}

fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}
