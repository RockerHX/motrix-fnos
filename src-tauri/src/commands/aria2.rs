use crate::app::AppState;
use crate::aria2::{
    ping_rpc, process_status, start_process, stop_process, Aria2ConfigStatus, Aria2ProcessStatus,
    Aria2RpcStatus,
};
use crate::config::aria2::Aria2Config;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_aria2_config_status() -> Aria2ConfigStatus {
    Aria2ConfigStatus::from_config(&Aria2Config::from_env())
}

#[tauri::command]
pub fn get_aria2_process_status(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    let status = process_status(&state.aria2_process)?;
    state
        .debug_logs
        .info("aria2", format!("读取 Aria2 进程状态：{}", status.message));
    Ok(status)
}

#[tauri::command]
pub fn start_aria2(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Aria2ProcessStatus, String> {
    start_process(
        &app,
        &state.aria2_process,
        &Aria2Config::from_env(),
        &state.debug_logs,
    )
}

#[tauri::command]
pub fn stop_aria2(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    stop_process(&state.aria2_process, &state.debug_logs)
}

#[tauri::command]
pub fn ping_aria2_rpc(state: State<'_, AppState>) -> Aria2RpcStatus {
    tauri::async_runtime::block_on(ping_rpc(
        &Aria2Config::from_env(),
        Some(&state.debug_logs),
    ))
}
