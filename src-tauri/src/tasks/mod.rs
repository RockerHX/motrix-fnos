use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use serde::Deserialize;
pub use motrix_fnos_server::tasks::{
    CreateDownloadTaskRequest, DownloadTask, DownloadTaskStatus,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs};

pub fn should_pause_task_on_exit(task: &DownloadTask) -> bool {
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

pub fn should_force_pause_task_on_startup(task: &DownloadTask) -> bool {
    should_pause_task_on_exit(task)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDownloadTask {
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
}

#[derive(Debug, Deserialize)]
struct AddUriResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct GidResponse {
    result: Option<String>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TellStatusResponse {
    result: Option<Aria2TaskStatus>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TellManyResponse {
    result: Option<Vec<Aria2TaskStatus>>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2TaskStatus {
    gid: Option<String>,
    status: String,
    total_length: String,
    completed_length: String,
    download_speed: String,
    error_code: Option<String>,
    error_message: Option<String>,
    dir: Option<String>,
    files: Option<Vec<Aria2FileStatus>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2FileStatus {
    path: String,
    #[serde(default)]
    uris: Vec<Aria2UriStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2UriStatus {
    uri: String,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    message: String,
}

pub fn prepare_task(request: CreateDownloadTaskRequest) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, None)
}

pub fn prepare_task_with_logs(
    request: CreateDownloadTaskRequest,
    debug_logs: &DebugLogStore,
) -> Result<PreparedDownloadTask, String> {
    prepare_task_inner(request, Some(debug_logs))
}

fn prepare_task_inner(
    request: CreateDownloadTaskRequest,
    debug_logs: Option<&DebugLogStore>,
) -> Result<PreparedDownloadTask, String> {
    let url = match normalize_required(&request.url, "下载链接不能为空") {
        Ok(url) => url,
        Err(error) => {
            log_error(debug_logs, "tasks.create", &error);
            return Err(error);
        }
    };
    if let Err(error) = validate_http_url(&url) {
        log_error(debug_logs, "tasks.create", &error);
        return Err(error);
    }

    let file_name = normalize_optional(request.file_name).unwrap_or_else(|| infer_file_name(&url));
    let save_dir = resolve_save_dir_with_logs(normalize_optional(request.save_dir), debug_logs)?;
    log_info(
        debug_logs,
        "tasks.create",
        format!(
            "下载任务参数已准备，URL {}，文件名 {}，保存目录 {}",
            redact_url_for_log(&url),
            file_name,
            save_dir
        ),
    );

    Ok(PreparedDownloadTask {
        file_name,
        save_dir,
        url,
    })
}

pub async fn add_uri_to_aria2(
    config: &Aria2Config,
    task: &PreparedDownloadTask,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    log_info(
        debug_logs,
        "aria2.addUri",
        format!(
            "开始创建 Aria2 下载任务，URL {}，保存目录 {}",
            redact_url_for_log(&task.url),
            task.save_dir
        ),
    );
    let request_body = build_add_uri_request(config, task);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "创建下载任务失败：无法连接 Aria2 RPC，请确认引擎已启动".to_string();
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
    };

    let rpc_response = match response.json::<AddUriResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("创建 Aria2 下载任务失败，响应解析失败：{}", error);
            log_error(debug_logs, "aria2.addUri", &error);
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("创建 Aria2 下载任务失败：{}", error.message);
        log_error(debug_logs, "aria2.addUri", &error);
        return Err(error);
    }

    let gid = rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "创建 Aria2 下载任务失败：响应缺少 GID".to_string())?;
    log_info(
        debug_logs,
        "aria2.addUri",
        format!("Aria2 下载任务创建成功，GID {}", gid),
    );
    Ok(gid)
}

pub async fn pause_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        config,
        gid,
        "aria2.pause",
        "motrix-fnos-pause",
        "暂停任务",
        debug_logs,
    )
    .await
}

pub async fn unpause_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        config,
        gid,
        "aria2.unpause",
        "motrix-fnos-unpause",
        "恢复任务",
        debug_logs,
    )
    .await
}

pub async fn remove_task(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    match send_gid_control_request(
        config,
        gid,
        "aria2.remove",
        "motrix-fnos-remove",
        "删除任务",
        debug_logs,
    )
    .await
    {
        Ok(result_gid) => Ok(result_gid),
        Err(error) => {
            log_info(
                debug_logs,
                "aria2.removeDownloadResult",
                format!(
                    "aria2.remove 未完成，尝试清理已停止任务结果，GID {}：{}",
                    gid, error
                ),
            );
            send_gid_control_request(
                config,
                gid,
                "aria2.removeDownloadResult",
                "motrix-fnos-remove-result",
                "删除任务结果",
                debug_logs,
            )
            .await
        }
    }
}

pub fn store_created_task(
    tasks: &Mutex<Vec<DownloadTask>>,
    next_id: &AtomicU64,
    prepared: PreparedDownloadTask,
    gid: String,
) -> Result<DownloadTask, String> {
    let file_path = Path::new(&prepared.save_dir)
        .join(&prepared.file_name)
        .display()
        .to_string();
    let now = current_timestamp_ms();
    let task = DownloadTask {
        id: next_id.fetch_add(1, Ordering::Relaxed),
        file_name: prepared.file_name,
        save_dir: prepared.save_dir,
        url: prepared.url,
        gid: Some(gid),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some(file_path),
        created_at: now,
        updated_at: now,
    };

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    guard.push(task.clone());

    Ok(task)
}

pub async fn refresh_tasks_from_aria2(
    tasks: &Mutex<Vec<DownloadTask>>,
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

    let client = reqwest::Client::new();
    let mut updates = Vec::new();
    for candidate in candidates {
        let Some(gid) = candidate.gid.clone() else {
            continue;
        };
        match tell_status(&client, config, &gid, debug_logs).await {
            Ok(status) if is_stale_aria2_gid_status(&status) => {
                match readd_download_task(config, &candidate, debug_logs).await {
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
            Ok(status) => updates.push(TaskRefreshUpdate::Status { gid, status }),
            Err(error) if is_stale_aria2_gid_error(&error) => {
                match readd_download_task(config, &candidate, debug_logs).await {
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

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    for update in &updates {
        match update {
            TaskRefreshUpdate::Status { gid, status } => {
                for task in guard
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
                if let Some(task) = guard
                    .iter_mut()
                    .find(|task| task.id == *task_id && task.gid.as_ref() == Some(old_gid))
                {
                    apply_readded_gid(task, new_gid);
                }
            }
        }
    }

    Ok(guard.clone())
}

pub async fn sync_task_progress_from_aria2_by_gid(
    tasks: &Mutex<Vec<DownloadTask>>,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    let client = reqwest::Client::new();
    let status = tell_status(&client, config, gid, debug_logs).await?;
    apply_aria2_status_by_gid(tasks, gid, &status)
}

pub async fn sync_task_progress_after_pause_by_gid(
    tasks: &Mutex<Vec<DownloadTask>>,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    const MAX_ATTEMPTS: usize = 8;
    const RETRY_INTERVAL_MS: u64 = 150;

    let client = reqwest::Client::new();
    let mut previous_completed = None;
    let mut latest_status = None;

    for attempt in 0..MAX_ATTEMPTS {
        let status = tell_status(&client, config, gid, debug_logs).await?;
        let completed = parse_aria2_u64(&status.completed_length);
        let settled = pause_status_is_settled(&status, previous_completed);
        previous_completed = Some(completed);
        latest_status = Some(status);

        if settled {
            break;
        }

        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    let status =
        latest_status.ok_or_else(|| "暂停后同步 Aria2 任务状态失败：未获取到状态".to_string())?;
    if !matches!(status.status.as_str(), "paused" | "complete" | "error") {
        log_info(
            debug_logs,
            "tasks.control",
            format!(
                "暂停后 Aria2 状态尚未稳定，使用最后一次进度，GID {}，状态 {}",
                gid, status.status
            ),
        );
    }
    apply_aria2_status_by_gid(tasks, gid, &status)
}

fn pause_status_is_settled(status: &Aria2TaskStatus, previous_completed: Option<u64>) -> bool {
    matches!(status.status.as_str(), "paused" | "complete" | "error")
        && previous_completed == Some(parse_aria2_u64(&status.completed_length))
}

fn apply_aria2_status_by_gid(
    tasks: &Mutex<Vec<DownloadTask>>,
    gid: &str,
    status: &Aria2TaskStatus,
) -> Result<DownloadTask, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.gid.as_deref() == Some(gid))
        .ok_or_else(|| format!("下载任务不存在，GID {}", gid))?;
    apply_aria2_status(task, status);
    Ok(task.clone())
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
}

fn should_refresh_task(task: &DownloadTask) -> bool {
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

pub async fn sync_session_tasks_from_aria2(
    tasks: &Mutex<Vec<DownloadTask>>,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let session_tasks = list_current_aria2_tasks(config, debug_logs).await?;
    if session_tasks.is_empty() {
        log_info(debug_logs, "tasks.restore", "Aria2 session 未加载任何任务");
        return list_tasks(tasks);
    }

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let mut matched_count = 0;
    let mut unmatched_count = 0;

    for session_task in &session_tasks {
        if let Some(index) = find_matching_sqlite_task(&guard, session_task) {
            let task = &mut guard[index];
            if let Some(gid) = session_task
                .gid
                .as_deref()
                .filter(|gid| !gid.trim().is_empty())
            {
                task.gid = Some(gid.to_string());
            }
            apply_aria2_status(task, session_task);
            if should_force_pause_task_on_startup(task) {
                apply_paused_state(task);
            }
            task.updated_at = current_timestamp_ms();
            matched_count += 1;
        } else {
            unmatched_count += 1;
            log_info(
                debug_logs,
                "tasks.restore",
                format!(
                    "Aria2 session 存在未匹配的任务，GID {}，不自动创建 UI 任务",
                    session_task.gid.as_deref().unwrap_or("-")
                ),
            );
        }
    }

    log_info(
        debug_logs,
        "tasks.restore",
        format!(
            "Aria2 session 任务同步完成：匹配 {} 个，未匹配 {} 个",
            matched_count, unmatched_count
        ),
    );

    Ok(guard.clone())
}

async fn list_current_aria2_tasks(
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<Aria2TaskStatus>, String> {
    let client = reqwest::Client::new();
    let mut tasks = Vec::new();
    for method in ["aria2.tellActive", "aria2.tellWaiting", "aria2.tellStopped"] {
        match tell_many_tasks(&client, config, method).await {
            Ok(mut result) => tasks.append(&mut result),
            Err(error) => {
                log_error(debug_logs, "tasks.restore", &error);
                return Err(error);
            }
        }
    }
    Ok(tasks)
}

async fn tell_many_tasks(
    client: &reqwest::Client,
    config: &Aria2Config,
    method: &str,
) -> Result<Vec<Aria2TaskStatus>, String> {
    let request_body = build_tell_many_request(config, method);
    let response = client
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
        .map_err(|error| format!("读取 Aria2 session 任务失败：无法连接 RPC（{}）", error))?;

    let rpc_response = response
        .json::<TellManyResponse>()
        .await
        .map_err(|error| format!("读取 Aria2 session 任务失败：响应解析失败（{}）", error))?;

    if let Some(error) = rpc_response.error {
        return Err(format!("读取 Aria2 session 任务失败：{}", error.message));
    }

    Ok(rpc_response.result.unwrap_or_default())
}

fn find_matching_sqlite_task(
    tasks: &[DownloadTask],
    session_task: &Aria2TaskStatus,
) -> Option<usize> {
    if let Some(gid) = session_task
        .gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty())
    {
        if let Some(index) = tasks.iter().position(|task| {
            task.status != DownloadTaskStatus::Removed && task.gid.as_deref() == Some(gid)
        }) {
            return Some(index);
        }
    }

    let urls = session_task_urls(session_task);
    if urls.is_empty() {
        return None;
    }

    tasks.iter().position(|task| {
        task.status != DownloadTaskStatus::Removed
            && urls.iter().any(|url| url == &task.url)
            && session_task_location_matches(task, session_task)
    })
}

fn session_task_urls(session_task: &Aria2TaskStatus) -> Vec<String> {
    session_task
        .files
        .as_ref()
        .into_iter()
        .flatten()
        .flat_map(|file| file.uris.iter())
        .map(|uri| uri.uri.trim())
        .filter(|uri| !uri.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn session_task_location_matches(task: &DownloadTask, session_task: &Aria2TaskStatus) -> bool {
    let dir_matches = session_task
        .dir
        .as_deref()
        .filter(|dir| !dir.trim().is_empty())
        .map(|dir| normalize_path_for_match(dir) == normalize_path_for_match(&task.save_dir))
        .unwrap_or(false);

    let file_matches = session_task.files.as_ref().is_some_and(|files| {
        files.iter().any(|file| {
            let normalized_path = normalize_path_for_match(&file.path);
            normalized_path.ends_with(&normalize_path_for_match(&task.file_name))
                || task
                    .file_path
                    .as_deref()
                    .map(|path| normalized_path == normalize_path_for_match(path))
                    .unwrap_or(false)
        })
    });

    dir_matches || file_matches
}

fn normalize_path_for_match(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

pub async fn readd_task_to_aria2(
    tasks: &Mutex<Vec<DownloadTask>>,
    config: &Aria2Config,
    task_id: u64,
    debug_logs: Option<&DebugLogStore>,
) -> Result<DownloadTask, String> {
    let task = {
        let guard = tasks
            .lock()
            .map_err(|_| "无法读取下载任务列表".to_string())?;
        guard
            .iter()
            .find(|task| task.id == task_id)
            .cloned()
            .ok_or_else(|| format!("下载任务不存在：{}", task_id))?
    };

    let new_gid = readd_download_task(config, &task, debug_logs).await?;

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;
    apply_readded_gid(task, &new_gid);
    Ok(task.clone())
}

pub fn move_task_files_to_trash(task: &DownloadTask) -> Result<(), String> {
    delete_task_file(task)
}

pub fn mark_task_redownloaded(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
    new_gid: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if task.status != DownloadTaskStatus::Complete {
            return Err("只有已完成任务可以重新下载".to_string());
        }

        task.gid = Some(new_gid);
        task.status = DownloadTaskStatus::Pending;
        task.total_length = 0;
        task.completed_length = 0;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.file_path = Some(
            Path::new(&task.save_dir)
                .join(&task.file_name)
                .display()
                .to_string(),
        );
        Ok(())
    })
}

async fn readd_download_task(
    config: &Aria2Config,
    task: &DownloadTask,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    log_info(
        debug_logs,
        "tasks.restore",
        format!(
            "Aria2 GID 失效，准备使用原始 URL 重新加入任务，ID {}，旧 GID {}",
            task.id,
            task.gid.as_deref().unwrap_or("-")
        ),
    );
    if let Some(old_gid) = task.gid.as_deref() {
        if let Err(error) = remove_download_result(config, old_gid, debug_logs).await {
            log_info(
                debug_logs,
                "tasks.restore",
                format!(
                    "旧 GID 结果清理未完成，继续重新加入任务，GID {}：{}",
                    old_gid, error
                ),
            );
        }
    }
    let prepared = PreparedDownloadTask {
        url: task.url.clone(),
        file_name: task.file_name.clone(),
        save_dir: task.save_dir.clone(),
    };
    add_uri_to_aria2(config, &prepared, debug_logs).await
}

async fn remove_download_result(
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    send_gid_control_request(
        config,
        gid,
        "aria2.removeDownloadResult",
        "motrix-fnos-remove-result-before-readd",
        "清理任务结果",
        debug_logs,
    )
    .await
}

pub fn list_tasks(tasks: &Mutex<Vec<DownloadTask>>) -> Result<Vec<DownloadTask>, String> {
    tasks
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "无法读取下载任务列表".to_string())
}

pub fn task_gid(tasks: &Mutex<Vec<DownloadTask>>, task_id: u64) -> Result<String, String> {
    let task = task_snapshot(tasks, task_id)?;

    if task.status == DownloadTaskStatus::Removed {
        return Err("已删除任务不能继续操作".to_string());
    }

    task.gid
        .clone()
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "下载任务缺少 Aria2 GID，无法控制".to_string())
}

pub fn task_snapshot(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let guard = tasks
        .lock()
        .map_err(|_| "无法读取下载任务列表".to_string())?;
    guard
        .iter()
        .find(|task| task.id == task_id)
        .cloned()
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))
}

pub fn mark_task_paused(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        apply_paused_state(task);
        Ok(())
    })
}

pub fn mark_task_paused_by_gid(
    tasks: &Mutex<Vec<DownloadTask>>,
    gid: &str,
) -> Result<DownloadTask, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.gid.as_deref() == Some(gid))
        .ok_or_else(|| format!("下载任务不存在，GID {}", gid))?;
    apply_paused_state(task);
    Ok(task.clone())
}

pub fn mark_unfinished_tasks_paused(
    tasks: &Mutex<Vec<DownloadTask>>,
) -> Result<Vec<DownloadTask>, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let mut updated = Vec::new();
    for task in guard
        .iter_mut()
        .filter(|task| should_pause_task_on_exit(task))
    {
        apply_paused_state(task);
        task.updated_at = current_timestamp_ms();
        updated.push(task.clone());
    }
    Ok(updated)
}

fn apply_paused_state(task: &mut DownloadTask) {
    task.status = DownloadTaskStatus::Paused;
    task.download_speed = 0;
    task.error_code = None;
    task.error_message = None;
}

pub fn mark_task_resumed(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.status = DownloadTaskStatus::Active;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

pub fn mark_task_removed(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if delete_files {
            delete_task_file(task)?;
        }
        task.status = DownloadTaskStatus::Removed;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

async fn tell_status(
    client: &reqwest::Client,
    config: &Aria2Config,
    gid: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Aria2TaskStatus, String> {
    let request_body = build_tell_status_request(config, gid);
    let response = match client
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = "同步任务状态失败：无法连接 Aria2 RPC".to_string();
            log_error(
                debug_logs,
                "aria2.tellStatus",
                format!("GID {} {}", gid, error),
            );
            return Err(error);
        }
    };

    let rpc_response = match response.json::<TellStatusResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("同步 Aria2 任务状态解析失败：{}", error);
            log_error(
                debug_logs,
                "aria2.tellStatus",
                format!("GID {} {}", gid, error),
            );
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("同步 Aria2 任务状态失败：{}", error.message);
        log_error(
            debug_logs,
            "aria2.tellStatus",
            format!("GID {} {}", gid, error),
        );
        return Err(error);
    }

    let status = rpc_response
        .result
        .ok_or_else(|| "同步 Aria2 任务状态失败：响应缺少任务状态".to_string())?;
    if is_aria2_status_error(&status) {
        log_error(
            debug_logs,
            "aria2.tellStatus",
            format!(
                "GID {} 返回错误状态，错误码 {}，原因 {}",
                gid,
                status.error_code.as_deref().unwrap_or("-"),
                status.error_message.as_deref().unwrap_or("未知错误")
            ),
        );
    }
    Ok(status)
}

fn apply_aria2_status(task: &mut DownloadTask, status: &Aria2TaskStatus) {
    let next_total_length = parse_aria2_u64(&status.total_length);
    let next_completed_length = parse_aria2_u64(&status.completed_length);
    let should_preserve_progress = should_preserve_existing_progress(
        &status.status,
        next_total_length,
        next_completed_length,
        task.total_length,
    );

    task.status = map_aria2_status(&status.status);
    if !should_preserve_progress {
        task.total_length = next_total_length;
        task.completed_length = next_completed_length;
    }
    task.download_speed = parse_aria2_u64(&status.download_speed);
    task.error_code = normalize_aria2_error_code(status.error_code.as_deref());
    task.error_message = status
        .error_message
        .clone()
        .filter(|message| !message.trim().is_empty());
    if let Some(dir) = status.dir.clone().filter(|dir| !dir.is_empty()) {
        task.save_dir = dir;
    }
    task.file_path = status
        .files
        .as_ref()
        .and_then(|files| files.first())
        .map(|file| file.path.clone())
        .filter(|path| !path.is_empty())
        .or_else(|| {
            Some(
                Path::new(&task.save_dir)
                    .join(&task.file_name)
                    .display()
                    .to_string(),
            )
        });
    task.updated_at = current_timestamp_ms();
}

fn should_preserve_existing_progress(
    status: &str,
    next_total_length: u64,
    next_completed_length: u64,
    current_total_length: u64,
) -> bool {
    next_total_length == 0
        && next_completed_length == 0
        && current_total_length > 0
        && matches!(status, "active" | "waiting" | "paused" | "error")
}

fn apply_readded_gid(task: &mut DownloadTask, new_gid: &str) {
    task.gid = Some(new_gid.to_string());
    task.status = DownloadTaskStatus::Active;
    task.download_speed = 0;
    task.error_code = None;
    task.error_message = None;
    task.file_path = Some(
        Path::new(&task.save_dir)
            .join(&task.file_name)
            .display()
            .to_string(),
    );
    task.updated_at = current_timestamp_ms();
}

fn update_task(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
    update: impl FnOnce(&mut DownloadTask) -> Result<(), String>,
) -> Result<DownloadTask, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;

    update(task)?;
    task.updated_at = current_timestamp_ms();
    Ok(task.clone())
}

fn delete_task_file(task: &DownloadTask) -> Result<(), String> {
    let Some(file_path) = task
        .file_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(());
    };

    let save_dir = Path::new(&task.save_dir)
        .canonicalize()
        .map_err(|error| format!("校验保存目录失败：{}（{}）", task.save_dir, error))?;
    let candidates = delete_file_candidates(Path::new(file_path));

    for path in candidates {
        if !path.exists() {
            continue;
        }
        if !path.is_file() {
            return Err(format!("当前仅支持删除单文件：{}", path.display()));
        }

        let file = path
            .canonicalize()
            .map_err(|error| format!("校验本地文件失败：{}（{}）", path.display(), error))?;
        if !file.starts_with(&save_dir) {
            return Err("拒绝删除保存目录外的文件".to_string());
        }

        delete_local_file(&file)?;
    }

    Ok(())
}

#[cfg(not(test))]
fn delete_local_file(file: &Path) -> Result<(), String> {
    trash::delete(file).map_err(|error| format!("移入回收站失败：{}（{}）", file.display(), error))
}

#[cfg(test)]
fn delete_local_file(file: &Path) -> Result<(), String> {
    fs::remove_file(file).map_err(|error| format!("删除测试文件失败：{}（{}）", file.display(), error))
}

fn delete_file_candidates(path: &Path) -> Vec<PathBuf> {
    vec![
        path.to_path_buf(),
        PathBuf::from(format!("{}.aria2", path.display())),
    ]
}

fn task_status_error(message: String) -> Aria2TaskStatus {
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
    }
}

fn is_stale_aria2_gid_status(status: &Aria2TaskStatus) -> bool {
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
    let normalized = message.to_ascii_lowercase();
    is_stale_aria2_gid_error(&normalized)
        || (normalized.contains("cannot be unpaused now")
            && task.status == DownloadTaskStatus::Error)
}

fn is_aria2_status_error(status: &Aria2TaskStatus) -> bool {
    status.status == "error"
        || normalize_aria2_error_code(status.error_code.as_deref()).is_some()
        || status
            .error_message
            .as_deref()
            .map(|message| !message.trim().is_empty())
            .unwrap_or(false)
}

fn normalize_aria2_error_code(error_code: Option<&str>) -> Option<String> {
    error_code
        .map(str::trim)
        .filter(|code| !code.is_empty() && *code != "0")
        .map(ToOwned::to_owned)
}

fn map_aria2_status(status: &str) -> DownloadTaskStatus {
    match status {
        "active" | "waiting" => DownloadTaskStatus::Active,
        "paused" => DownloadTaskStatus::Paused,
        "complete" => DownloadTaskStatus::Complete,
        "error" => DownloadTaskStatus::Error,
        "removed" => DownloadTaskStatus::Removed,
        _ => DownloadTaskStatus::Pending,
    }
}

fn parse_aria2_u64(value: &str) -> u64 {
    value.parse::<u64>().unwrap_or_default()
}

fn build_tell_status_request(config: &Aria2Config, gid: &str) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!(gid));
    params.push(serde_json::json!([
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "errorCode",
        "errorMessage",
        "dir",
        "files"
    ]));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-tell-status",
        "method": "aria2.tellStatus",
        "params": params,
    })
}

fn build_tell_many_request(config: &Aria2Config, method: &str) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    if method != "aria2.tellActive" {
        params.push(serde_json::json!(0));
        params.push(serde_json::json!(1000));
    }
    params.push(serde_json::json!([
        "gid",
        "status",
        "totalLength",
        "completedLength",
        "downloadSpeed",
        "errorCode",
        "errorMessage",
        "dir",
        "files"
    ]));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": format!("motrix-fnos-{}", method.replace('.', "-")),
        "method": method,
        "params": params,
    })
}

async fn send_gid_control_request(
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
    action_label: &str,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let module = method;
    log_info(
        debug_logs,
        module,
        format!("开始{}，GID {}", action_label, gid),
    );
    let request_body = build_gid_control_request(config, gid, method, request_id);
    let response = match reqwest::Client::new()
        .post(config.rpc_url())
        .json(&request_body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            let error = format!("{}失败：无法连接 Aria2 RPC", action_label);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    let rpc_response = match response.json::<GidResponse>().await {
        Ok(response) => response,
        Err(error) => {
            let error = format!("{}失败，响应解析失败：{}", action_label, error);
            log_error(debug_logs, module, format!("GID {} {}", gid, error));
            return Err(error);
        }
    };

    if let Some(error) = rpc_response.error {
        let error = format!("{}失败：{}", action_label, error.message);
        log_error(debug_logs, module, format!("GID {} {}", gid, error));
        return Err(error);
    }

    let result_gid = rpc_response
        .result
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| format!("{}失败：响应缺少 GID", action_label))?;
    log_info(
        debug_logs,
        module,
        format!("{}成功，GID {}", action_label, result_gid),
    );
    Ok(result_gid)
}

fn build_gid_control_request(
    config: &Aria2Config,
    gid: &str,
    method: &str,
    request_id: &str,
) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params.push(serde_json::json!(gid));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
        "params": params,
    })
}

fn build_add_uri_request(config: &Aria2Config, task: &PreparedDownloadTask) -> serde_json::Value {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }

    params.push(serde_json::json!([task.url.clone()]));

    let mut options = serde_json::Map::new();
    options.insert("dir".to_string(), serde_json::json!(task.save_dir));
    if !task.file_name.trim().is_empty() {
        options.insert("out".to_string(), serde_json::json!(task.file_name));
    }
    params.push(serde_json::Value::Object(options));

    serde_json::json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-add-uri",
        "method": "aria2.addUri",
        "params": params,
    })
}

fn normalize_required(value: &str, empty_message: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(empty_message.to_string());
    }

    Ok(trimmed.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn resolve_save_dir_with_logs(
    input: Option<String>,
    debug_logs: Option<&DebugLogStore>,
) -> Result<String, String> {
    let source = if input.is_some() {
        "自定义"
    } else {
        "默认"
    };
    let path = match input {
        Some(path) => expand_home_dir(&path)?,
        None => default_download_dir()?,
    };
    log_info(
        debug_logs,
        "tasks.path",
        format!("解析{}下载目录：{}", source, path.display()),
    );

    if let Err(error) = fs::create_dir_all(&path) {
        let error = format!("创建下载目录失败：{}（{}）", path.display(), error);
        log_error(debug_logs, "tasks.path", &error);
        return Err(error);
    }

    if !path.is_dir() {
        let error = format!("下载目录不是有效文件夹：{}", path.display());
        log_error(debug_logs, "tasks.path", &error);
        return Err(error);
    }
    log_info(
        debug_logs,
        "tasks.path",
        format!("下载目录可用：{}", path.display()),
    );

    Ok(path.display().to_string())
}

fn default_download_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join("Downloads"))
}

pub fn default_download_dir_string() -> Result<String, String> {
    Ok(default_download_dir()?.display().to_string())
}

fn expand_home_dir(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        return home_dir();
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }

    Ok(PathBuf::from(path))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "无法读取当前用户目录，不能确定默认下载目录".to_string())
}

fn validate_http_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Ok(());
    }

    Err("阶段 1 当前仅支持 HTTP / HTTPS 下载链接".to_string())
}

fn infer_file_name(url: &str) -> String {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/');

    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("未命名下载任务")
        .to_string()
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn log_info(debug_logs: Option<&DebugLogStore>, module: &str, message: impl Into<String>) {
    if let Some(debug_logs) = debug_logs {
        debug_logs.info(module, message);
    }
}

fn log_error(debug_logs: Option<&DebugLogStore>, module: &str, message: impl Into<String>) {
    if let Some(debug_logs) = debug_logs {
        debug_logs.error(module, message);
    }
}

fn redact_url_for_log(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Aria2Config {
        Aria2Config {
            aria2_path: None,
            binary_source: crate::config::aria2::Aria2BinarySource::Sidecar,
            sidecar_name: "aria2-next".to_string(),
            target_triple: "test-target".to_string(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: String::new(),
            session_path: None,
            log_path: None,
        }
    }

    fn temp_download_dir(name: &str) -> String {
        let dir = env::temp_dir().join(format!(
            "motrix-fnos-test-{}-{}",
            name,
            current_timestamp_ms()
        ));
        dir.display().to_string()
    }

    fn sample_task(file_path: Option<String>, save_dir: String) -> DownloadTask {
        DownloadTask {
            id: 1,
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir,
            gid: Some("abc123".to_string()),
            status: DownloadTaskStatus::Active,
            total_length: 100,
            completed_length: 40,
            download_speed: 20,
            error_code: Some("old".to_string()),
            error_message: Some("old".to_string()),
            file_path,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn prepare_task_accepts_https_url() {
        let task = prepare_task(CreateDownloadTaskRequest {
            url: " https://example.com/file.zip?token=1 ".to_string(),
            file_name: None,
            save_dir: Some(format!(" {} ", temp_download_dir("prepare"))),
        })
        .expect("https task should be prepared");

        assert_eq!(task.url, "https://example.com/file.zip?token=1");
        assert_eq!(task.file_name, "file.zip");
        assert!(Path::new(&task.save_dir).is_dir());
    }

    #[test]
    fn prepare_task_rejects_non_http_url() {
        let error = prepare_task(CreateDownloadTaskRequest {
            url: "magnet:?xt=urn:btih:test".to_string(),
            file_name: None,
            save_dir: None,
        })
        .expect_err("non-http url should fail");

        assert!(error.contains("HTTP / HTTPS"));
    }

    #[test]
    fn store_created_task_persists_gid() {
        let tasks = Mutex::new(Vec::new());
        let next_id = AtomicU64::new(1);
        let task = store_created_task(
            &tasks,
            &next_id,
            PreparedDownloadTask {
                url: "https://example.com/file.zip".to_string(),
                file_name: "file.zip".to_string(),
                save_dir: "/downloads".to_string(),
            },
            "abc123".to_string(),
        )
        .expect("task should be stored");

        assert_eq!(task.id, 1);
        assert_eq!(task.gid.as_deref(), Some("abc123"));
        assert_eq!(
            list_tasks(&tasks).expect("tasks should be readable").len(),
            1
        );
    }

    #[test]
    fn task_gid_rejects_removed_task() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Removed;
        let tasks = Mutex::new(vec![task]);

        let error = task_gid(&tasks, 1).expect_err("removed task should be rejected");

        assert!(error.contains("已删除"));
    }

    #[test]
    fn startup_force_pause_scope_matches_exit_pause_scope() {
        let mut task = sample_task(None, "/downloads".to_string());

        task.status = DownloadTaskStatus::Pending;
        assert!(should_force_pause_task_on_startup(&task));

        task.status = DownloadTaskStatus::Active;
        assert!(should_force_pause_task_on_startup(&task));

        task.status = DownloadTaskStatus::Paused;
        assert!(!should_force_pause_task_on_startup(&task));

        task.status = DownloadTaskStatus::Complete;
        assert!(!should_force_pause_task_on_startup(&task));
    }

    #[test]
    fn exit_pause_scope_only_includes_unfinished_tasks() {
        let mut task = sample_task(None, "/downloads".to_string());

        task.status = DownloadTaskStatus::Pending;
        assert!(should_pause_task_on_exit(&task));

        task.status = DownloadTaskStatus::Active;
        assert!(should_pause_task_on_exit(&task));

        task.status = DownloadTaskStatus::Paused;
        assert!(!should_pause_task_on_exit(&task));

        task.status = DownloadTaskStatus::Complete;
        assert!(!should_pause_task_on_exit(&task));

        task.status = DownloadTaskStatus::Error;
        assert!(!should_pause_task_on_exit(&task));

        task.status = DownloadTaskStatus::Removed;
        assert!(!should_pause_task_on_exit(&task));
    }

    #[test]
    fn mark_task_paused_updates_status_and_speed() {
        let tasks = Mutex::new(vec![sample_task(None, "/downloads".to_string())]);

        let task = mark_task_paused(&tasks, 1).expect("task should be paused");

        assert_eq!(task.status, DownloadTaskStatus::Paused);
        assert_eq!(task.download_speed, 0);
        assert_eq!(task.error_message, None);
    }

    #[test]
    fn mark_task_paused_by_gid_updates_matching_task() {
        let tasks = Mutex::new(vec![sample_task(None, "/downloads".to_string())]);

        let task = mark_task_paused_by_gid(&tasks, "abc123").expect("task should be paused");

        assert_eq!(task.status, DownloadTaskStatus::Paused);
        assert_eq!(task.download_speed, 0);
        assert_eq!(task.error_message, None);
    }

    #[test]
    fn mark_task_resumed_updates_status() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Paused;
        let tasks = Mutex::new(vec![task]);

        let task = mark_task_resumed(&tasks, 1).expect("task should be resumed");

        assert_eq!(task.status, DownloadTaskStatus::Active);
    }

    #[test]
    fn mark_task_redownloaded_resets_completed_task_progress() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Complete;
        task.total_length = 100;
        task.completed_length = 100;
        task.download_speed = 0;
        let tasks = Mutex::new(vec![task]);

        let task = mark_task_redownloaded(&tasks, 1, "new-gid".to_string())
            .expect("completed task should be redownloaded");

        assert_eq!(task.gid.as_deref(), Some("new-gid"));
        assert_eq!(task.status, DownloadTaskStatus::Pending);
        assert_eq!(task.total_length, 0);
        assert_eq!(task.completed_length, 0);
        assert_eq!(task.download_speed, 0);
        assert!(task.error_code.is_none());
        assert!(task.error_message.is_none());
        assert_eq!(task.file_path.as_deref(), Some("/downloads/file.zip"));
    }

    #[test]
    fn mark_task_redownloaded_rejects_unfinished_task() {
        let task = sample_task(None, "/downloads".to_string());
        let tasks = Mutex::new(vec![task]);

        let error = mark_task_redownloaded(&tasks, 1, "new-gid".to_string())
            .expect_err("unfinished task should be rejected");

        assert!(error.contains("已完成任务"));
    }

    #[test]
    fn move_task_files_to_trash_removes_completed_file_before_redownload() {
        let save_dir = PathBuf::from(temp_download_dir("redownload-trash"));
        fs::create_dir_all(&save_dir).expect("save dir should be created");
        let file_path = save_dir.join("file.zip");
        fs::write(&file_path, b"completed").expect("file should be written");
        let aria2_path = save_dir.join("file.zip.aria2");
        fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
        let mut task = sample_task(
            Some(file_path.display().to_string()),
            save_dir.display().to_string(),
        );
        task.status = DownloadTaskStatus::Complete;

        move_task_files_to_trash(&task).expect("files should move to trash");

        assert!(!file_path.exists());
        assert!(!aria2_path.exists());
    }

    fn session_status(gid: &str, url: &str, dir: &str, path: &str) -> Aria2TaskStatus {
        Aria2TaskStatus {
            gid: Some(gid.to_string()),
            status: "paused".to_string(),
            total_length: "100".to_string(),
            completed_length: "40".to_string(),
            download_speed: "0".to_string(),
            error_code: None,
            error_message: None,
            dir: Some(dir.to_string()),
            files: Some(vec![Aria2FileStatus {
                path: path.to_string(),
                uris: vec![Aria2UriStatus {
                    uri: url.to_string(),
                }],
            }]),
        }
    }

    #[test]
    fn session_task_matches_by_url_dir_and_file() {
        let task = sample_task(
            Some("/downloads/file.zip".to_string()),
            "/downloads".to_string(),
        );
        let session_task = session_status(
            "newgid",
            "https://example.com/file.zip",
            "/downloads",
            "/downloads/file.zip",
        );

        assert_eq!(find_matching_sqlite_task(&[task], &session_task), Some(0));
    }

    #[test]
    fn session_task_does_not_match_unknown_url() {
        let task = sample_task(None, "/downloads".to_string());
        let session_task = session_status(
            "newgid",
            "https://example.com/other.zip",
            "/downloads",
            "/downloads/file.zip",
        );

        assert_eq!(find_matching_sqlite_task(&[task], &session_task), None);
    }

    #[test]
    fn tell_many_request_uses_offsets_for_waiting_tasks() {
        let config = test_config();
        let request = build_tell_many_request(&config, "aria2.tellWaiting");

        assert_eq!(request["method"], "aria2.tellWaiting");
        assert_eq!(request["params"][0], 0);
        assert_eq!(request["params"][1], 1000);
    }

    #[test]
    fn is_stale_aria2_gid_error_detects_unrecoverable_resume_errors() {
        assert!(is_stale_aria2_gid_error("No URI available"));
        assert!(is_stale_aria2_gid_error(
            "GID 6c4e6a308ea8d57e is not found"
        ));
        assert!(!is_stale_aria2_gid_error("GID#123 cannot be unpaused now"));
        assert!(!is_stale_aria2_gid_error("download failed"));
    }

    #[test]
    fn resume_error_readds_when_gid_is_not_found() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Error;

        assert!(should_readd_task_after_resume_error(
            &task,
            "恢复任务失败：GID 6c4e6a308ea8d57e is not found"
        ));
    }

    #[test]
    fn resume_error_readds_only_when_task_already_has_stale_gid_error() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Error;
        task.error_message = Some("No URI available.".to_string());

        assert!(should_readd_task_after_resume_error(
            &task,
            "GID#abc cannot be unpaused now"
        ));

        task.error_message = Some("download failed".to_string());
        assert!(should_readd_task_after_resume_error(
            &task,
            "GID#abc cannot be unpaused now"
        ));

        task.status = DownloadTaskStatus::Active;
        assert!(!should_readd_task_after_resume_error(
            &task,
            "GID#abc cannot be unpaused now"
        ));
    }

    #[test]
    fn mark_task_removed_moves_file_under_save_dir_to_trash() {
        let save_dir = PathBuf::from(temp_download_dir("delete-file"));
        fs::create_dir_all(&save_dir).expect("save dir should be created");
        let file_path = save_dir.join("file.zip");
        fs::write(&file_path, b"test").expect("file should be written");
        let aria2_path = save_dir.join("file.zip.aria2");
        fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
        let tasks = Mutex::new(vec![sample_task(
            Some(file_path.display().to_string()),
            save_dir.display().to_string(),
        )]);

        let task = mark_task_removed(&tasks, 1, true).expect("task should be removed");

        assert_eq!(task.status, DownloadTaskStatus::Removed);
        assert!(!file_path.exists());
        assert!(!aria2_path.exists());
    }

    #[test]
    fn mark_task_removed_moves_orphan_aria2_control_file_to_trash() {
        let save_dir = PathBuf::from(temp_download_dir("delete-orphan-aria2"));
        fs::create_dir_all(&save_dir).expect("save dir should be created");
        let file_path = save_dir.join("file.zip");
        let aria2_path = save_dir.join("file.zip.aria2");
        fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
        let tasks = Mutex::new(vec![sample_task(
            Some(file_path.display().to_string()),
            save_dir.display().to_string(),
        )]);

        let task = mark_task_removed(&tasks, 1, true).expect("task should be removed");

        assert_eq!(task.status, DownloadTaskStatus::Removed);
        assert!(!aria2_path.exists());
    }

    #[test]
    fn delete_file_candidates_include_aria2_control_file() {
        let candidates = delete_file_candidates(Path::new("/downloads/file.iso"));

        assert_eq!(candidates[0], PathBuf::from("/downloads/file.iso"));
        assert_eq!(candidates[1], PathBuf::from("/downloads/file.iso.aria2"));
    }

    #[test]
    fn mark_task_removed_refuses_file_outside_save_dir() {
        let save_dir = PathBuf::from(temp_download_dir("safe-delete-save"));
        let outside_dir = PathBuf::from(temp_download_dir("safe-delete-outside"));
        fs::create_dir_all(&save_dir).expect("save dir should be created");
        fs::create_dir_all(&outside_dir).expect("outside dir should be created");
        let file_path = outside_dir.join("file.zip");
        fs::write(&file_path, b"test").expect("file should be written");
        let tasks = Mutex::new(vec![sample_task(
            Some(file_path.display().to_string()),
            save_dir.display().to_string(),
        )]);

        let error =
            mark_task_removed(&tasks, 1, true).expect_err("outside file should be rejected");

        assert!(error.contains("保存目录外"));
        assert!(file_path.exists());
    }

    #[test]
    fn tell_status_request_contains_gid_and_fields() {
        let request = build_tell_status_request(&test_config(), "abc123");

        assert_eq!(request["method"], "aria2.tellStatus");
        assert_eq!(request["params"][0], "abc123");
        assert!(request["params"][1]
            .as_array()
            .expect("fields should be array")
            .contains(&serde_json::json!("downloadSpeed")));
    }

    #[test]
    fn apply_aria2_status_updates_progress_fields() {
        let mut task = DownloadTask {
            id: 1,
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            gid: Some("abc123".to_string()),
            status: DownloadTaskStatus::Pending,
            total_length: 0,
            completed_length: 0,
            download_speed: 0,
            error_code: None,
            error_message: None,
            file_path: None,
            created_at: 1,
            updated_at: 1,
        };

        apply_aria2_status(
            &mut task,
            &Aria2TaskStatus {
                gid: None,
                status: "active".to_string(),
                total_length: "100".to_string(),
                completed_length: "40".to_string(),
                download_speed: "20".to_string(),
                error_code: None,
                error_message: None,
                dir: Some("/downloads".to_string()),
                files: Some(vec![Aria2FileStatus {
                    path: "/downloads/file.zip".to_string(),
                    uris: Vec::new(),
                }]),
            },
        );

        assert_eq!(task.status, DownloadTaskStatus::Active);
        assert_eq!(task.total_length, 100);
        assert_eq!(task.completed_length, 40);
        assert_eq!(task.download_speed, 20);
        assert_eq!(task.file_path.as_deref(), Some("/downloads/file.zip"));
    }

    #[test]
    fn pause_status_settles_only_after_paused_progress_is_stable() {
        let active = Aria2TaskStatus {
            gid: Some("abc123".to_string()),
            status: "active".to_string(),
            total_length: "100".to_string(),
            completed_length: "80".to_string(),
            download_speed: "50".to_string(),
            error_code: None,
            error_message: None,
            dir: None,
            files: None,
        };
        let mut paused = active.clone();
        paused.status = "paused".to_string();
        paused.download_speed = "0".to_string();

        assert!(!pause_status_is_settled(&active, Some(80)));
        assert!(!pause_status_is_settled(&paused, None));
        assert!(!pause_status_is_settled(&paused, Some(79)));
        assert!(pause_status_is_settled(&paused, Some(80)));
    }

    #[test]
    fn apply_aria2_status_by_gid_updates_progress_before_pause_state() {
        let tasks = Mutex::new(vec![sample_task(None, "/downloads".to_string())]);
        let status = Aria2TaskStatus {
            gid: Some("abc123".to_string()),
            status: "active".to_string(),
            total_length: "100".to_string(),
            completed_length: "80".to_string(),
            download_speed: "50".to_string(),
            error_code: None,
            error_message: None,
            dir: Some("/downloads".to_string()),
            files: None,
        };

        let synced = apply_aria2_status_by_gid(&tasks, "abc123", &status)
            .expect("task progress should sync");
        assert_eq!(synced.completed_length, 80);

        let paused = mark_task_paused(&tasks, 1).expect("task should pause");
        assert_eq!(paused.status, DownloadTaskStatus::Paused);
        assert_eq!(paused.completed_length, 80);
        assert_eq!(paused.total_length, 100);
        assert_eq!(paused.download_speed, 0);
    }

    #[test]
    fn apply_aria2_status_ignores_empty_error_code_zero() {
        let mut task = DownloadTask {
            id: 1,
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            gid: Some("abc123".to_string()),
            status: DownloadTaskStatus::Pending,
            total_length: 0,
            completed_length: 0,
            download_speed: 0,
            error_code: Some("old".to_string()),
            error_message: Some("old".to_string()),
            file_path: None,
            created_at: 1,
            updated_at: 1,
        };

        let status = Aria2TaskStatus {
            gid: None,
            status: "complete".to_string(),
            total_length: "100".to_string(),
            completed_length: "100".to_string(),
            download_speed: "0".to_string(),
            error_code: Some("0".to_string()),
            error_message: Some("".to_string()),
            dir: None,
            files: None,
        };

        assert!(!is_aria2_status_error(&status));
        apply_aria2_status(&mut task, &status);

        assert_eq!(task.status, DownloadTaskStatus::Complete);
        assert_eq!(task.error_code, None);
        assert_eq!(task.error_message, None);
    }

    #[test]
    fn non_zero_aria2_error_code_is_error() {
        let status = Aria2TaskStatus {
            gid: None,
            status: "error".to_string(),
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            error_code: Some("3".to_string()),
            error_message: Some("Resource not found".to_string()),
            dir: None,
            files: None,
        };

        assert!(is_aria2_status_error(&status));
        assert_eq!(
            normalize_aria2_error_code(status.error_code.as_deref()).as_deref(),
            Some("3")
        );
    }

    #[test]
    fn apply_aria2_status_preserves_progress_when_active_status_is_temporarily_empty() {
        let mut task = sample_task(None, "/downloads".to_string());
        task.status = DownloadTaskStatus::Paused;
        let status = Aria2TaskStatus {
            gid: None,
            status: "active".to_string(),
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            error_code: None,
            error_message: None,
            dir: None,
            files: None,
        };

        apply_aria2_status(&mut task, &status);

        assert_eq!(task.status, DownloadTaskStatus::Active);
        assert_eq!(task.total_length, 100);
        assert_eq!(task.completed_length, 40);
        assert_eq!(task.download_speed, 0);
        assert!(task.error_code.is_none());
        assert!(task.error_message.is_none());
    }

    #[test]
    fn apply_aria2_status_preserves_progress_when_error_has_no_lengths() {
        let mut task = sample_task(None, "/downloads".to_string());
        let status = Aria2TaskStatus {
            gid: None,
            status: "error".to_string(),
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            error_code: Some("1".to_string()),
            error_message: Some(
                "SSL/TLS handshake failure: unable to get local issuer certificate".to_string(),
            ),
            dir: None,
            files: None,
        };

        apply_aria2_status(&mut task, &status);

        assert_eq!(task.status, DownloadTaskStatus::Error);
        assert_eq!(task.total_length, 100);
        assert_eq!(task.completed_length, 40);
        assert_eq!(task.download_speed, 0);
        assert_eq!(task.error_code.as_deref(), Some("1"));
        assert_eq!(
            task.error_message.as_deref(),
            Some("SSL/TLS handshake failure: unable to get local issuer certificate")
        );
    }

    #[test]
    fn task_status_error_keeps_readable_message() {
        let status = task_status_error("同步任务状态失败：无法连接 Aria2 RPC".to_string());

        assert_eq!(status.status, "error");
        assert_eq!(
            status.error_message.as_deref(),
            Some("同步任务状态失败：无法连接 Aria2 RPC")
        );
    }

    #[test]
    fn tell_status_request_contains_error_and_file_fields() {
        let request = build_tell_status_request(&test_config(), "abc123");
        let fields = request["params"][1]
            .as_array()
            .expect("fields should be array");

        assert!(fields.contains(&serde_json::json!("errorCode")));
        assert!(fields.contains(&serde_json::json!("errorMessage")));
        assert!(fields.contains(&serde_json::json!("dir")));
        assert!(fields.contains(&serde_json::json!("files")));
    }

    #[test]
    fn expand_home_dir_supports_tilde_paths() {
        let expanded = expand_home_dir("~/Downloads").expect("home path should expand");

        assert!(expanded.ends_with("Downloads"));
        assert!(expanded.is_absolute());
    }

    #[test]
    fn resolve_save_dir_creates_missing_directory() {
        let dir = temp_download_dir("missing-dir");
        let resolved = resolve_save_dir_with_logs(Some(dir.clone()), None)
            .expect("directory should be created");

        assert_eq!(resolved, dir);
        assert!(Path::new(&resolved).is_dir());
    }

    #[test]
    fn default_download_dir_uses_downloads_under_home() {
        let dir = default_download_dir().expect("default download dir should resolve");

        assert!(dir.ends_with("Downloads"));
    }

    #[test]
    fn add_uri_request_contains_url_and_options() {
        let request = build_add_uri_request(
            &test_config(),
            &PreparedDownloadTask {
                url: "https://example.com/file.zip".to_string(),
                file_name: "custom.zip".to_string(),
                save_dir: "/downloads".to_string(),
            },
        );

        assert_eq!(request["method"], "aria2.addUri");
        assert_eq!(request["params"][0][0], "https://example.com/file.zip");
        assert_eq!(request["params"][1]["dir"], "/downloads");
        assert_eq!(request["params"][1]["out"], "custom.zip");
    }

    #[test]
    fn gid_control_request_contains_method_and_gid() {
        let request =
            build_gid_control_request(&test_config(), "abc123", "aria2.pause", "pause-test");

        assert_eq!(request["method"], "aria2.pause");
        assert_eq!(request["id"], "pause-test");
        assert_eq!(request["params"][0], "abc123");
    }

    #[test]
    fn gid_control_request_includes_token_when_configured() {
        let mut config = test_config();
        config.rpc_secret = "secret".to_string();

        let request = build_gid_control_request(&config, "abc123", "aria2.unpause", "unpause-test");

        assert_eq!(request["params"][0], "token:secret");
        assert_eq!(request["params"][1], "abc123");
    }

    #[test]
    fn stale_aria2_gid_error_is_detected() {
        assert!(is_stale_aria2_gid_error(
            "同步 Aria2 任务状态失败：No URI available."
        ));
        assert!(is_stale_aria2_gid_status(&Aria2TaskStatus {
            gid: None,
            status: "error".to_string(),
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            error_code: Some("1".to_string()),
            error_message: Some("No URI available.".to_string()),
            dir: None,
            files: None,
        }));
        assert!(!is_stale_aria2_gid_error("连接失败"));
    }

    #[test]
    fn readded_gid_updates_task_without_clearing_progress() {
        let save_dir = temp_download_dir("readded-gid");
        let mut task = sample_task(None, save_dir.clone());
        task.status = DownloadTaskStatus::Error;
        task.gid = Some("old-gid".to_string());
        task.error_code = Some("1".to_string());
        task.error_message = Some("No URI available.".to_string());

        apply_readded_gid(&mut task, "new-gid");

        assert_eq!(task.gid.as_deref(), Some("new-gid"));
        assert_eq!(task.status, DownloadTaskStatus::Active);
        assert_eq!(task.completed_length, 40);
        assert_eq!(task.total_length, 100);
        assert!(task.error_code.is_none());
        assert!(task.error_message.is_none());
        let expected_file_path = Path::new(&save_dir).join("file.zip").display().to_string();
        assert_eq!(task.file_path.as_deref(), Some(expected_file_path.as_str()));
    }
}
