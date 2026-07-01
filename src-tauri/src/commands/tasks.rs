use crate::app::AppState;
use crate::aria2::{
    generate_rpc_secret, ping_rpc, process_status, runtime_config,
    select_rpc_port_with_saved_runtime, start_process,
};
use crate::commands::settings::load_app_config_from_pool;
use crate::config::aria2::Aria2Config;
use crate::database::tasks::{
    persist_download_task_state, persist_download_task_states, upsert_download_task,
};
use crate::tasks::{
    add_uri_to_aria2, is_stale_aria2_gid_error, mark_task_paused, mark_task_redownloaded,
    mark_task_removed, mark_task_resumed, move_task_files_to_trash, pause_task,
    prepare_task_with_logs, readd_task_to_aria2, refresh_tasks_from_aria2, remove_task,
    should_readd_task_after_resume_error, store_created_task, task_gid, task_snapshot,
    unpause_task, CreateDownloadTaskRequest, DownloadTask, DownloadTaskStatus,
};
use std::time::Duration;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn create_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    let mut payload = payload;
    if payload
        .save_dir
        .as_deref()
        .map(|save_dir| save_dir.trim().is_empty())
        .unwrap_or(true)
    {
        let app_config = load_app_config_from_pool(&state.database.pool).await?;
        payload.save_dir = Some(app_config.default_download_dir);
    }
    let prepared = prepare_task_with_logs(payload, &state.debug_logs)?;
    let config = ensure_aria2_ready(&app, &state).await?;
    let gid = add_uri_to_aria2(&config, &prepared, Some(&state.debug_logs)).await?;
    let task = store_created_task(&state.download_tasks, &state.next_task_id, prepared, gid)?;
    upsert_download_task(&state.database.pool, &task).await?;
    state.debug_logs.info(
        "tasks.create",
        format!(
            "下载任务已写入内存列表和 SQLite，ID {}，GID {}",
            task.id,
            task.gid.as_deref().unwrap_or("-")
        ),
    );
    Ok(task)
}

async fn ensure_aria2_ready(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Aria2Config, String> {
    let process = process_status(&state.aria2_process)?;
    if !process.running {
        state
            .debug_logs
            .info("aria2", "Aria2 进程未运行，准备自动启动");
        let base = Aria2Config::from_env();
        let saved_runtime = state.load_saved_aria2_runtime();
        let port =
            select_rpc_port_with_saved_runtime(&base, saved_runtime.as_ref(), &state.debug_logs)
                .ok_or_else(crate::aria2::rpc_ports_exhausted_message)?;
        let config = runtime_config(&base, port, generate_rpc_secret());
        let status = start_process(app, &state.aria2_process, &config, &state.debug_logs)
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
    wait_for_rpc_ready(&config, &state.debug_logs).await?;
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
    let tasks = refresh_tasks_from_aria2(
        &state.download_tasks,
        &state.aria2_config(),
        Some(&state.debug_logs),
    )
    .await?;
    sync_tasks_to_database(&state, &tasks).await?;

    Ok(tasks
        .into_iter()
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .collect())
}

#[tauri::command]
pub async fn pause_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let config = ensure_aria2_ready(&app, &state).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    pause_task(&config, &gid, Some(&state.debug_logs)).await?;
    let task = mark_task_paused(&state.download_tasks, task_id)?;
    sync_task_to_database(&state, &task).await?;
    state.debug_logs.info(
        "tasks.control",
        format!("任务已暂停，ID {}，GID {}", task_id, gid),
    );
    Ok(task)
}

#[tauri::command]
pub async fn resume_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let config = ensure_aria2_ready(&app, &state).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    let task_before_resume = task_snapshot(&state.download_tasks, task_id)?;
    let task = match unpause_task(&config, &gid, Some(&state.debug_logs)).await {
        Ok(_) => mark_task_resumed(&state.download_tasks, task_id)?,
        Err(error) if should_readd_task_after_resume_error(&task_before_resume, &error) => {
            state.debug_logs.warn(
                "tasks.restore",
                format!("恢复任务时发现旧 GID 已失效，准备重新加入任务：{}", error),
            );
            readd_task_to_aria2(
                &state.download_tasks,
                &config,
                task_id,
                Some(&state.debug_logs),
            )
            .await?
        }
        Err(error) => return Err(error),
    };
    sync_task_to_database(&state, &task).await?;
    state.debug_logs.info(
        "tasks.control",
        format!(
            "任务已恢复，ID {}，旧 GID {}，当前 GID {}",
            task_id,
            gid,
            task.gid.as_deref().unwrap_or("-")
        ),
    );
    Ok(task)
}

#[tauri::command]
pub async fn redownload_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let config = ensure_aria2_ready(&app, &state).await?;
    let task = task_snapshot(&state.download_tasks, task_id)?;
    if task.status != DownloadTaskStatus::Complete {
        return Err("只有已完成任务可以重新下载".to_string());
    }

    move_task_files_to_trash(&task)?;
    let prepared = crate::tasks::PreparedDownloadTask {
        url: task.url.clone(),
        file_name: task.file_name.clone(),
        save_dir: task.save_dir.clone(),
    };
    let gid = add_uri_to_aria2(&config, &prepared, Some(&state.debug_logs)).await?;
    let task = mark_task_redownloaded(&state.download_tasks, task_id, gid.clone())?;
    sync_task_to_database(&state, &task).await?;
    state.debug_logs.info(
        "tasks.control",
        format!(
            "任务已重新下载，ID {}，GID {}，原本地文件已移入回收站",
            task_id, gid
        ),
    );
    Ok(task)
}

#[tauri::command]
pub async fn delete_download_task(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    let config = ensure_aria2_ready(&app, &state).await?;
    let gid = task_gid(&state.download_tasks, task_id)?;
    if let Err(error) = remove_task(&config, &gid, Some(&state.debug_logs)).await {
        if is_stale_aria2_gid_error(&error) {
            state.debug_logs.warn(
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
    let task = mark_task_removed(&state.download_tasks, task_id, delete_files)?;
    sync_task_to_database(&state, &task).await?;
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

async fn sync_tasks_to_database(
    state: &State<'_, AppState>,
    tasks: &[DownloadTask],
) -> Result<(), String> {
    persist_download_task_states(&state.database.pool, tasks).await
}

async fn sync_task_to_database(
    state: &State<'_, AppState>,
    task: &DownloadTask,
) -> Result<(), String> {
    persist_download_task_state(&state.database.pool, task).await
}
