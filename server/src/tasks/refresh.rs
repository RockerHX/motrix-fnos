use super::*;
use crate::aria2::Aria2RpcClient;

pub async fn refresh_tasks_from_aria2(
    tasks: &TaskMemoryState,
    app_data_dir: &Path,
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let snapshot = list_tasks(tasks)?;
    let candidates: Vec<DownloadTask> = snapshot
        .iter()
        .filter(|task| should_refresh_task(task))
        .filter(|task| {
            task.gid
                .as_deref()
                .map(|gid| !gid.trim().is_empty())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    if candidates.is_empty() {
        return Ok(snapshot);
    }

    // 远端 RPC 可能阻塞，必须基于快照完成查询，不能在 await 期间持有任务写锁。
    // 查询结果先收集为更新指令，最后一次性回写，避免半批任务已更新、半批仍是旧状态。
    let mut updates = Vec::new();
    for candidate in candidates {
        let Some(gid) = candidate.gid.clone() else {
            continue;
        };
        // session 恢复后旧 GID 可能已经失效：普通任务可按原配置重建，等待 metadata 的磁链任务则必须保留确认流程并报告错误。
        match tell_status(client, config, &gid, debug_logs).await {
            Ok(status) if is_stale_aria2_gid_status(&status) => {
                if is_pending_magnet_metadata_task(&candidate) {
                    updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: stale_magnet_metadata_status(app_data_dir, &candidate),
                    });
                    continue;
                }
                match readd_download_task(client, config, &candidate, debug_logs).await {
                    Ok(new_gid) => updates.push(TaskRefreshUpdate::Readded {
                        task_id: candidate.id,
                        old_gid: gid,
                        new_gid,
                    }),
                    Err(error) => updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: task_status_error(error),
                    }),
                }
            }
            Ok(status) => {
                match resolve_followed_metadata(client, config, &gid, &status, debug_logs).await {
                    Some(Ok((followed_status, metadata_torrent_path))) => {
                        updates.push(TaskRefreshUpdate::MagnetMetadataResolved {
                            old_gid: gid,
                            status: followed_status,
                            metadata_torrent_path,
                        });
                    }
                    Some(Err(error)) => updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: task_status_error(error),
                    }),
                    None => updates.push(TaskRefreshUpdate::Status { gid, status }),
                }
            }
            Err(error) if is_stale_aria2_gid_error(&error) => {
                if is_pending_magnet_metadata_task(&candidate) {
                    updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: stale_magnet_metadata_status(app_data_dir, &candidate),
                    });
                    continue;
                }
                match readd_download_task(client, config, &candidate, debug_logs).await {
                    Ok(new_gid) => updates.push(TaskRefreshUpdate::Readded {
                        task_id: candidate.id,
                        old_gid: gid,
                        new_gid,
                    }),
                    Err(error) => updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: task_status_error(error),
                    }),
                }
            }
            Err(error) => updates.push(TaskRefreshUpdate::Status {
                gid,
                status: task_status_error(error),
            }),
        }
    }

    // 回写时再次核对 task id 与旧 GID，防止 RPC 往返期间并发操作产生的新 GID 被旧查询结果覆盖。
    let mut guard = tasks.with_tasks_mut(|tasks| {
        for update in &updates {
            match update {
                TaskRefreshUpdate::Status { gid, status } => {
                    for task in tasks
                        .iter_mut()
                        .filter(|task| task.gid.as_ref() == Some(gid))
                    {
                        apply_aria2_status(task, status);
                    }
                }
                TaskRefreshUpdate::Readded {
                    task_id,
                    old_gid,
                    new_gid,
                } => {
                    if let Some(task) = tasks
                        .iter_mut()
                        .find(|task| task.id == *task_id && task.gid.as_ref() == Some(old_gid))
                    {
                        apply_readded_gid(task, new_gid);
                    }
                }
                TaskRefreshUpdate::MagnetMetadataResolved {
                    old_gid,
                    status,
                    metadata_torrent_path,
                } => {
                    if let Some(task) = tasks
                        .iter_mut()
                        .find(|task| task.gid.as_ref() == Some(old_gid))
                    {
                        apply_magnet_metadata_confirmation(
                            task,
                            status,
                            metadata_torrent_path.clone(),
                        );
                    }
                }
            }
        }

        tasks.clone()
    })?;

    Ok(std::mem::take(&mut guard))
}

pub async fn sync_task_progress_from_aria2_by_gid(
    client: &Aria2RpcClient,
    tasks: &TaskMemoryState,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    let status = tell_status(client, config, gid, debug_logs).await?;
    apply_aria2_status_by_gid(tasks, gid, &status)
}

pub async fn sync_task_progress_after_pause_by_gid(
    client: &Aria2RpcClient,
    tasks: &TaskMemoryState,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    const MAX_ATTEMPTS: usize = 61;
    const RETRY_INTERVAL_MS: u64 = 500;

    let mut previous_completed = None;
    let mut latest_status = None;
    let mut settled = false;

    // Aria2 接受 pause 后仍可能短暂写入缓存；需要状态已暂停且连续两次进度不变，才能把最终进度持久化。
    for attempt in 0..MAX_ATTEMPTS {
        let status = tell_status(client, config, gid, debug_logs).await?;
        let completed = parse_aria2_u64(&status.completed_length);
        let is_settled = pause_status_is_settled(&status, previous_completed);
        previous_completed = Some(completed);
        latest_status = Some(status);

        if is_settled {
            settled = true;
            break;
        }

        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    let status =
        latest_status.ok_or_else(|| "暂停后同步 Aria2 任务状态失败：未获取到状态".to_string())?;
    if let Err(error) = ensure_pause_status_settled(gid, &status, settled) {
        log_info(debug_logs, "tasks.control", &error);
        return Err(error);
    }
    apply_aria2_status_by_gid(tasks, gid, &status)
}

pub(super) fn ensure_pause_status_settled(
    gid: &str,
    status: &Aria2TaskStatus,
    settled: bool,
) -> Result<(), String> {
    if settled {
        return Ok(());
    }

    Err(format!(
        "暂停后 Aria2 状态在等待期限内仍未稳定，GID {}，当前状态 {}",
        gid, status.status
    ))
}

pub(super) fn pause_status_is_settled(
    status: &Aria2TaskStatus,
    previous_completed: Option<u64>,
) -> bool {
    matches!(status.status.as_str(), "paused" | "complete" | "error")
        && previous_completed == Some(parse_aria2_u64(&status.completed_length))
}

enum TaskRefreshUpdate {
    Status {
        gid: String,
        status: Aria2TaskStatus,
    },
    Readded {
        task_id: u64,
        old_gid: String,
        new_gid: String,
    },
    MagnetMetadataResolved {
        old_gid: String,
        status: Aria2TaskStatus,
        metadata_torrent_path: String,
    },
}

pub(super) fn task_status_error(message: String) -> Aria2TaskStatus {
    Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: None,
        error_message: Some(message),
        dir: None,
        files: None,
        followed_by: None,
        bittorrent: None,
    }
}

pub(super) fn is_stale_aria2_gid_status(status: &Aria2TaskStatus) -> bool {
    status.status == "error"
        && status
            .error_message
            .as_deref()
            .map(is_stale_aria2_gid_error)
            .unwrap_or(false)
}

pub fn is_stale_aria2_gid_error(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("no uri available") || normalized.contains("is not found")
}

pub fn should_readd_task_after_resume_error(task: &DownloadTask, message: &str) -> bool {
    if is_pending_magnet_metadata_task(task) {
        return false;
    }
    let normalized = message.to_ascii_lowercase();
    is_stale_aria2_gid_error(&normalized)
        || (normalized.contains("cannot be unpaused now")
            && task.status == DownloadTaskStatus::Error)
}
