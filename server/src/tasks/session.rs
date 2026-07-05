use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use crate::tasks::{
    add_uri_to_aria2, should_force_pause_task_on_startup, DownloadTask, DownloadTaskStatus,
    PreparedDownloadTask,
};
use std::sync::Mutex;

use super::aria2_rpc::{build_tell_many_request, send_gid_control_request, TellManyResponse};
use super::{
    apply_aria2_status, apply_paused_state, apply_readded_gid, current_timestamp_ms, log_error,
    log_info, Aria2TaskStatus,
};

pub async fn sync_session_tasks_from_aria2(
    tasks: &Mutex<Vec<DownloadTask>>,
    config: &Aria2Config,
    debug_logs: Option<&DebugLogStore>,
) -> Result<Vec<DownloadTask>, String> {
    let session_tasks = list_current_aria2_tasks(config, debug_logs).await?;
    if session_tasks.is_empty() {
        log_info(debug_logs, "tasks.restore", "Aria2 session 未加载任何任务");
        return crate::tasks::list_tasks(tasks);
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

pub(crate) fn find_matching_sqlite_task(
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

pub(crate) async fn readd_download_task(
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
        aria2_options: serde_json::Map::new(),
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
