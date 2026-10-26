use super::*;
use crate::aria2::Aria2RpcClient;

pub async fn refresh_tasks_from_aria2(
    tasks: &TaskMemoryState,
    pool: &sqlx::SqlitePool,
    app_data_dir: &Path,
    accessible_paths_path: &Path,
    client: &Aria2RpcClient,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let snapshot = list_tasks(tasks)?;
    let candidates: Vec<DownloadTask> = snapshot
        .iter()
        .filter(|task| should_refresh_task(task))
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .filter(|task| task.engine_generation == config.engine_generation())
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
                if candidate.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
                    updates.push(TaskRefreshUpdate::Status {
                        gid: gid.clone(),
                        status: task_status_error(
                            "task_resume_state_invalid：原生任务 GID 已丢失，未自动重建"
                                .to_string(),
                        ),
                    });
                    continue;
                } else if is_pending_magnet_metadata_task(&candidate) {
                    updates.push(TaskRefreshUpdate::Status {
                        gid: gid.clone(),
                        status: stale_magnet_metadata_status(app_data_dir, &candidate),
                    });
                    continue;
                }
                match readd_download_task(
                    client,
                    config,
                    &candidate,
                    accessible_paths_path,
                    debug_logs,
                )
                .await
                {
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
                if candidate.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
                    updates.push(TaskRefreshUpdate::Status { gid, status });
                } else {
                    match resolve_followed_metadata(
                        client, config, &candidate, &gid, &status, debug_logs,
                    )
                    .await
                    {
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
            }
            Err(error) if is_stale_aria2_gid_error(&error) => {
                if candidate.engine_generation == crate::config::aria2::EngineGeneration::NativeV2 {
                    updates.push(TaskRefreshUpdate::Status {
                        gid: gid.clone(),
                        status: task_status_error(
                            "task_resume_state_invalid：原生任务 GID 已丢失，未自动重建"
                                .to_string(),
                        ),
                    });
                    continue;
                } else if is_pending_magnet_metadata_task(&candidate) {
                    updates.push(TaskRefreshUpdate::Status {
                        gid,
                        status: stale_magnet_metadata_status(app_data_dir, &candidate),
                    });
                    continue;
                }
                match readd_download_task(
                    client,
                    config,
                    &candidate,
                    accessible_paths_path,
                    debug_logs,
                )
                .await
                {
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

    let mut operations = Vec::new();
    let mut changed = Vec::new();
    for update in updates {
        let original = snapshot.iter().find(|task| match &update {
            TaskRefreshUpdate::Status { gid, .. } => task.gid.as_ref() == Some(gid),
            TaskRefreshUpdate::Readded {
                task_id, old_gid, ..
            } => task.id == *task_id && task.gid.as_ref() == Some(old_gid),
            TaskRefreshUpdate::MagnetMetadataResolved { old_gid, .. } => {
                task.gid.as_ref() == Some(old_gid)
            }
        });
        let Some(original) = original else { continue };
        let Ok(operation) = tasks.begin_operation(original.id) else {
            continue;
        };
        if task_snapshot(tasks, original.id).ok().as_ref() != Some(original) {
            continue;
        }
        let mut next = original.clone();
        match update {
            TaskRefreshUpdate::Status { status, .. } => {
                super::progress::update_aria2_status(&mut next, &status);
            }
            TaskRefreshUpdate::Readded { new_gid, .. } => apply_readded_gid(&mut next, &new_gid),
            TaskRefreshUpdate::MagnetMetadataResolved {
                status,
                metadata_torrent_path,
                ..
            } => {
                apply_magnet_metadata_confirmation(&mut next, &status, metadata_torrent_path);
            }
        }
        // 轮询时间不是业务变化；结构相等比较同时覆盖不会出现在公开 JSON 中的代理绑定。
        next.updated_at = original.updated_at;
        if next.persisted_snapshot() == original.persisted_snapshot() {
            continue;
        }
        next.updated_at = current_timestamp_ms();
        operations.push(operation);
        changed.push((original.clone(), next));
    }
    // 先提交后发布；失败时内存仍是可再次查询的原快照，完成状态也不会漏存。
    let saved = crate::database::tasks::persist_task_refreshes(pool, &changed).await?;
    tasks.with_tasks_mut(|stored| {
        for (original, next) in saved {
            if let Some(task) = stored.iter_mut().find(|task| **task == original) {
                *task = next;
                if task.status == DownloadTaskStatus::Complete {
                    super::files::cleanup_aria2_control_file(task);
                }
            }
        }
        stored.clone()
    })
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
    let status = read_task_status_after_pause(client, config, gid, debug_logs).await?;
    apply_aria2_status_by_gid(tasks, gid, &status)
}

pub(crate) async fn read_task_status_after_pause(
    client: &Aria2RpcClient,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2TaskStatus, String> {
    const MAX_ATTEMPTS: usize = 21;
    const RETRY_INTERVAL_MS: u64 = 500;
    // 留出管理入口 30 秒总预算中的操作持久化时间，RPC 等待也计入确认期限。
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    let mut previous_completed = None;
    let mut latest_status = None;
    let mut settled = false;

    // Aria2 接受 pause 后仍可能短暂写入缓存；需要状态已暂停且连续两次进度不变，才能把最终进度持久化。
    for attempt in 0..MAX_ATTEMPTS {
        let status =
            match tokio::time::timeout_at(deadline, tell_status(client, config, gid, debug_logs))
                .await
            {
                Ok(result) => result.map_err(|error| {
                    if config.state_dir.is_some() {
                        format!("task_pause_unconfirmed：无法确认暂停状态：{error}；已保留原任务和文件，请停止并重启应用后核对")
                    } else {
                        error
                    }
                })?,
                Err(_) => break,
            };
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

    let status = latest_status.ok_or_else(|| {
        "task_pause_unconfirmed：暂停确认超时，未获取到状态；已保留原任务和文件，请停止并重启应用后核对"
            .to_string()
    })?;
    if status.gid.as_deref() != Some(gid) && config.state_dir.is_some() {
        return Err("task_resume_state_invalid：暂停状态的 GID 不一致".to_string());
    }
    if let Err(error) = ensure_pause_status_settled(gid, &status, settled) {
        log_info(debug_logs, "tasks.control", &error);
        return Err(error);
    }
    Ok(status)
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
        "task_pause_unconfirmed：暂停后 Aria2 状态在等待期限内仍未稳定，GID {}，当前状态 {}；已保留原任务和文件，请停止并重启应用后核对",
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
    if task.engine_generation == crate::config::aria2::EngineGeneration::NativeV2
        || is_pending_magnet_metadata_task(task)
    {
        return false;
    }
    let normalized = message.to_ascii_lowercase();
    is_stale_aria2_gid_error(&normalized)
        || (normalized.contains("cannot be unpaused now")
            && task.status == DownloadTaskStatus::Error)
}

#[cfg(test)]
mod tests;
