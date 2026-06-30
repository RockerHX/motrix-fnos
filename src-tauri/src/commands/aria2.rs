use crate::app::AppState;
use crate::aria2::{
    ping_rpc, process_status, start_process, stop_process, Aria2ConfigStatus, Aria2ProcessStatus,
    Aria2RpcStatus,
};
use crate::config::aria2::Aria2Config;
use tauri::State;

#[tauri::command]
pub fn get_aria2_config_status() -> Aria2ConfigStatus {
    Aria2ConfigStatus::from_config(&Aria2Config::from_env())
}

#[tauri::command]
pub fn get_aria2_process_status(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    process_status(&state.aria2_process)
}

#[tauri::command]
pub fn start_aria2(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    start_process(&state.aria2_process, &Aria2Config::from_env())
}

#[tauri::command]
pub fn stop_aria2(state: State<'_, AppState>) -> Result<Aria2ProcessStatus, String> {
    stop_process(&state.aria2_process)
}

#[tauri::command]
pub async fn ping_aria2_rpc() -> Aria2RpcStatus {
    ping_rpc(&Aria2Config::from_env()).await
}
