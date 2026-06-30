use crate::app::AppState;
use crate::aria2::{ping_rpc, process_status, start_process};
use crate::config::aria2::Aria2Config;
use crate::tasks::{
    add_uri_to_aria2, prepare_task, refresh_tasks_from_aria2, store_created_task,
    CreateDownloadTaskRequest, DownloadTask,
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
    let prepared = prepare_task(payload)?;
    ensure_aria2_ready(&app, &state, &config).await?;
    let gid = add_uri_to_aria2(&config, &prepared).await?;
    store_created_task(&state.download_tasks, &state.next_task_id, prepared, gid)
}

async fn ensure_aria2_ready(
    app: &AppHandle,
    state: &State<'_, AppState>,
    config: &Aria2Config,
) -> Result<(), String> {
    let process = process_status(&state.aria2_process)?;
    if !process.running {
        start_process(app, &state.aria2_process, config)
            .map_err(|error| format!("启动 Aria2 Next 失败：{}", shorten_start_error(error)))?;
    }

    wait_for_rpc_ready(config).await
}

async fn wait_for_rpc_ready(config: &Aria2Config) -> Result<(), String> {
    const MAX_ATTEMPTS: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 300;

    let mut last_message = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        let status = ping_rpc(config).await;
        if status.connected {
            return Ok(());
        }

        last_message = status.message;
        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    if last_message.is_empty() {
        Err("Aria2 Next 已启动但 RPC 未就绪，请稍后重试".to_string())
    } else {
        Err(format!(
            "Aria2 Next 已启动但 RPC 未就绪，请稍后重试（{}）",
            normalize_rpc_error(&last_message)
        ))
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
    refresh_tasks_from_aria2(&state.download_tasks, &Aria2Config::from_env()).await
}
