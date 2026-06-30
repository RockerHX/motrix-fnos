use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadTaskStatus {
    Pending,
    Active,
    Paused,
    Complete,
    Error,
    Removed,
}

impl DownloadTaskStatus {
    pub fn as_storage_value(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Error => "error",
            Self::Removed => "removed",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "active" => Self::Active,
            "paused" => Self::Paused,
            "complete" => Self::Complete,
            "error" => Self::Error,
            "removed" => Self::Removed,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: u64,
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub gid: Option<String>,
    pub status: DownloadTaskStatus,
    pub total_length: u64,
    pub completed_length: u64,
    pub download_speed: u64,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub file_path: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDownloadTaskRequest {
    pub url: String,
    pub file_name: Option<String>,
    pub save_dir: Option<String>,
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
struct Aria2TaskStatus {
    status: String,
    total_length: String,
    completed_length: String,
    download_speed: String,
    error_code: Option<String>,
    error_message: Option<String>,
    dir: Option<String>,
    files: Option<Vec<Aria2FileStatus>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Aria2FileStatus {
    path: String,
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
    let prepared = PreparedDownloadTask {
        url: task.url.clone(),
        file_name: task.file_name.clone(),
        save_dir: task.save_dir.clone(),
    };
    add_uri_to_aria2(config, &prepared, debug_logs).await
}

pub fn list_tasks(tasks: &Mutex<Vec<DownloadTask>>) -> Result<Vec<DownloadTask>, String> {
    tasks
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "无法读取下载任务列表".to_string())
}

pub fn task_gid(tasks: &Mutex<Vec<DownloadTask>>, task_id: u64) -> Result<String, String> {
    let guard = tasks
        .lock()
        .map_err(|_| "无法读取下载任务列表".to_string())?;
    let task = guard
        .iter()
        .find(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;

    if task.status == DownloadTaskStatus::Removed {
        return Err("已删除任务不能继续操作".to_string());
    }

    task.gid
        .clone()
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "下载任务缺少 Aria2 GID，无法控制".to_string())
}

pub fn mark_task_paused(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.status = DownloadTaskStatus::Paused;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
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
    task.status = map_aria2_status(&status.status);
    task.total_length = parse_aria2_u64(&status.total_length);
    task.completed_length = parse_aria2_u64(&status.completed_length);
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
    let path = Path::new(file_path);
    if !path.exists() {
        return Ok(());
    }
    if !path.is_file() {
        return Err(format!("当前仅支持删除单文件：{}", path.display()));
    }

    let save_dir = Path::new(&task.save_dir)
        .canonicalize()
        .map_err(|error| format!("校验保存目录失败：{}（{}）", task.save_dir, error))?;
    let file = path
        .canonicalize()
        .map_err(|error| format!("校验本地文件失败：{}（{}）", path.display(), error))?;

    if !file.starts_with(&save_dir) {
        return Err("拒绝删除保存目录外的文件".to_string());
    }

    fs::remove_file(&file)
        .map_err(|error| format!("删除本地文件失败：{}（{}）", file.display(), error))
}

fn task_status_error(message: String) -> Aria2TaskStatus {
    Aria2TaskStatus {
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
    message.to_ascii_lowercase().contains("no uri available")
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
    fn mark_task_paused_updates_status_and_speed() {
        let tasks = Mutex::new(vec![sample_task(None, "/downloads".to_string())]);

        let task = mark_task_paused(&tasks, 1).expect("task should be paused");

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
    fn mark_task_removed_deletes_file_under_save_dir() {
        let save_dir = PathBuf::from(temp_download_dir("delete-file"));
        fs::create_dir_all(&save_dir).expect("save dir should be created");
        let file_path = save_dir.join("file.zip");
        fs::write(&file_path, b"test").expect("file should be written");
        let tasks = Mutex::new(vec![sample_task(
            Some(file_path.display().to_string()),
            save_dir.display().to_string(),
        )]);

        let task = mark_task_removed(&tasks, 1, true).expect("task should be removed");

        assert_eq!(task.status, DownloadTaskStatus::Removed);
        assert!(!file_path.exists());
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
                status: "active".to_string(),
                total_length: "100".to_string(),
                completed_length: "40".to_string(),
                download_speed: "20".to_string(),
                error_code: None,
                error_message: None,
                dir: Some("/downloads".to_string()),
                files: Some(vec![Aria2FileStatus {
                    path: "/downloads/file.zip".to_string(),
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
