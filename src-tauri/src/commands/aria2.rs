use crate::app::AppState;
use crate::aria2::{
    generate_rpc_secret, ping_rpc, process_status, runtime_config,
    select_rpc_port_with_saved_runtime, start_process, stop_process, Aria2ConfigStatus,
    Aria2ProcessStatus, Aria2RpcStatus,
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
    let base = Aria2Config::from_env();
    let saved_runtime = state.load_saved_aria2_runtime();
    let port = select_rpc_port_with_saved_runtime(&base, saved_runtime.as_ref(), &state.debug_logs)
        .ok_or_else(crate::aria2::rpc_ports_exhausted_message)?;
    let config =
        state.with_aria2_runtime_paths(runtime_config(&base, port, generate_rpc_secret()))?;
    let status = start_process(&app, &state.aria2_process, &config, &state.debug_logs)?;
    if let (Some(pid), Some(source)) = (status.pid, status.binary_source.clone()) {
        state.set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            source,
            crate::aria2::process_args(&config),
        ))?;
    }
    Ok(status)
}

#[tauri::command]
pub fn stop_aria2(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    let status = stop_process(&state.aria2_process, &state.debug_logs)?;
    state.clear_aria2_runtime();
    Ok(status)
}

#[tauri::command]
pub fn ping_aria2_rpc(state: State<'_, AppState>) -> Aria2RpcStatus {
    tauri::async_runtime::block_on(ping_rpc(&state.aria2_config(), Some(&state.debug_logs)))
}
