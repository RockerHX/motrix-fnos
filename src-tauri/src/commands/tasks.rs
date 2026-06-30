use crate::app::AppState;
use crate::aria2::{ping_rpc, process_status, start_process};
use crate::config::aria2::Aria2Config;
use crate::tasks::{
    add_uri_to_aria2, mark_task_paused, mark_task_removed, mark_task_resumed, pause_task,
    prepare_task_with_logs, refresh_tasks_from_aria2, remove_task, store_created_task, task_gid,
    unpause_task, CreateDownloadTaskRequest, DownloadTask,
};
use std::time::Duration;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    let config = Aria2Config::from_env();
    let prepared = prepare_task_with_logs(payload, &state.debug_logs)?;
    ensure_aria2_ready(&app, &state, &config).await?;
    let gid = add_uri_to_aria2(&config, &prepared, Some(&state.debug_logs)).await?;
    let task = store_created_task(&state.download_tasks, &state.next_task_id, prepared, gid)?;
    state.debug_logs.info(
        "tasks.create",
        format!(
            "下载任务已写入内存列表，ID {}，GID {}",
            task.id,
            task.gid.as_deref().unwrap_or("-")
        ),
    );
    Ok(task)
}

async fn ensure_aria2_ready(
    app: &AppHandle,
    state: &State<'_, AppState>,
    config: &Aria2Config,
) -> Result<(), String> {
    let process = process_status(&state.aria2_process)?;
    if !process.running {
        state
            .debug_logs
            .info("aria2", "Aria2 进程未运行，准备自动启动");
        start_process(app, &state.aria2_process, config, &state.debug_logs)
            .map_err(|error| format!("启动 Aria2 Next 失败：{}", shorten_start_error(error)))?;
    }

    wait_for_rpc_ready(config, &state.debug_logs).await
}

async fn wait_for_rpc_ready(
    config: &Aria2Config,
    debug_logs: &crate::debug_logs::DebugLogStore,
) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 300;

    let mut last_message = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        let status = ping_rpc(config, None).await;
        if status.connected {
            debug_logs.info(
                "aria2.rpc",
                format!("Aria2 RPC ready，第 {} 次检查成功", attempt + 1),
            );
            return Ok(());
        }

        last_message = status.message;
        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    if last_message.is_empty() {
        let error = "Aria2 Next 已启动但 RPC 未就绪，请稍后重试".to_string();
        debug_logs.error("aria2.rpc", &error);
        Err(error)
    } else {
        let error = format!(
            "Aria2 Next 已启动但 RPC 未就绪，请稍后重试（{}）",
            normalize_rpc_error(&last_message)
        );
        debug_logs.error("aria2.rpc", format!("RPC ready timeout：{}", error));
        Err(error)
    }
}

fn normalize_rpc_error(message: &str) -> String {
    if message.contains("error sending request")
        || message.contains("Connection refused")
        || message.contains("连接失败")
    {
        return "无法连接本地 RPC".to_string();
    }

    message.to_string()
}

fn shorten_start_error(message: String) -> String {
    if message.contains("permission") || message.contains("Permission") {
        return "内置 Aria2 Next 没有执行权限".to_string();
    }

    message
}

#[tauri::command]
pub async fn list_download_tasks(state: State<'_, AppState>) -> Result<Vec<DownloadTask>, String> {
    refresh_tasks_from_aria2(
        &state.download_tasks,
        &Aria2Config::from_env(),
        Some(&state.debug_logs),
    )
    .await
}

#[tauri::command]
pub async fn pause_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let config = Aria2Config::from_env();
    ensure_aria2_ready(&app, &state, &config).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    pause_task(&config, &gid, Some(&state.debug_logs)).await?;
    let task = mark_task_paused(&state.download_tasks, task_id)?;
    state
        .debug_logs
        .info("tasks.control", format!("任务已暂停，ID {}，GID {}", task_id, gid));
    Ok(task)
}

#[tauri::command]
pub async fn resume_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let config = Aria2Config::from_env();
    ensure_aria2_ready(&app, &state, &config).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    unpause_task(&config, &gid, Some(&state.debug_logs)).await?;
    let task = mark_task_resumed(&state.download_tasks, task_id)?;
    state
        .debug_logs
        .info("tasks.control", format!("任务已恢复，ID {}，GID {}", task_id, gid));
    Ok(task)
}

#[tauri::command]
pub async fn delete_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    let config = Aria2Config::from_env();
    ensure_aria2_ready(&app, &state, &config).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    remove_task(&config, &gid, Some(&state.debug_logs)).await?;
    let task = mark_task_removed(&state.download_tasks, task_id, delete_files)?;
    state.debug_logs.info(
        "tasks.control",
        format!(
            "任务已删除，ID {}，GID {}，删除本地文件 {}",
            task_id,
            gid,
            if delete_files { "是" } else { "否" }
        ),
    );
    Ok(task)
}
