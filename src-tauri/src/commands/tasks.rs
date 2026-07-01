use crate::app::AppState;
use crate::aria2::{
    generate_rpc_secret, ping_rpc, process_status, runtime_config,
    select_rpc_port_with_saved_runtime, start_process,
};
use crate::config::aria2::Aria2Config;
use crate::tasks::{CreateDownloadTaskRequest, DownloadTask};
use motrix_fnos_server::tasks::service::TaskService;
use std::time::Duration;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    let service = task_service(&state);
    service.ensure_not_exiting()?;
    let config = ensure_aria2_ready(&app, &state).await?;
    service.create_download_task(&config, payload).await
}

async fn ensure_aria2_ready(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Aria2Config, String> {
    let process = process_status(&state.aria2_process)?;
    if !process.running {
        state
            .core
            .debug_logs
            .info("aria2", "Aria2 进程未运行，准备自动启动");
        let base = Aria2Config::from_env();
        let saved_runtime = state.load_saved_aria2_runtime();
        let port = select_rpc_port_with_saved_runtime(
            &base,
            saved_runtime.as_ref(),
            &state.core.debug_logs,
        )
        .ok_or_else(crate::aria2::rpc_ports_exhausted_message)?;
        let config =
            state.with_aria2_runtime_paths(runtime_config(&base, port, generate_rpc_secret()))?;
        let status = start_process(app, &state.aria2_process, &config, &state.core.debug_logs)
            .map_err(|error| format!("启动 Aria2 Next 失败：{}", shorten_start_error(error)))?;
        if let (Some(pid), Some(source)) = (status.pid, status.binary_source.clone()) {
            state.set_aria2_runtime(state.build_aria2_runtime_info(
                pid,
                &config,
                source,
                crate::aria2::process_args(&config),
            ))?;
        }
    }

    let config = state.aria2_config();
    if let Err(error) = wait_for_rpc_ready(&config, &state.core.debug_logs).await {
        let status = process_status(&state.aria2_process)?;
        if !status.running {
            state.clear_aria2_runtime();
            state.core.debug_logs.error(
                "aria2",
                format!("Aria2 进程已退出，RPC 无法就绪：{}", status.message),
            );
            return Err(format!(
                "Aria2 Next 启动后已退出，RPC 未就绪，请查看 Aria2 日志（{}）",
                normalize_rpc_error(&error)
            ));
        }
        return Err(error);
    }
    Ok(config)
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
    task_service(&state)
        .list_download_tasks(&state.aria2_config())
        .await
}

#[tauri::command]
pub async fn pause_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let service = task_service(&state);
    service.ensure_not_exiting()?;
    let config = ensure_aria2_ready(&app, &state).await?;
    service.pause_download_task(&config, task_id).await
}

#[tauri::command]
pub async fn resume_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let service = task_service(&state);
    service.ensure_not_exiting()?;
    let config = ensure_aria2_ready(&app, &state).await?;
    service.resume_download_task(&config, task_id).await
}

#[tauri::command]
pub async fn redownload_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let service = task_service(&state);
    service.ensure_not_exiting()?;
    let config = ensure_aria2_ready(&app, &state).await?;
    service.redownload_download_task(&config, task_id).await
}

#[tauri::command]
pub async fn delete_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    let service = task_service(&state);
    service.ensure_not_exiting()?;
    let config = ensure_aria2_ready(&app, &state).await?;
    service
        .delete_download_task(&config, task_id, delete_files)
        .await
}

fn task_service<'a>(state: &'a State<'_, AppState>) -> TaskService<'a> {
    TaskService::new(
        &state.core.database.pool,
        &state.core.download_tasks,
        &state.core.next_task_id,
        &state.core.debug_logs,
        &state.core.is_exiting,
    )
}
