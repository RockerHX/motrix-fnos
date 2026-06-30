use crate::app::AppState;
use crate::debug_logs::DebugLogEntry;
use tauri::State;

#[tauri::command]
pub fn list_debug_logs(state: State<'_, AppState>) -> Vec<DebugLogEntry> {
    state.debug_logs.list()
}

#[tauri::command]
pub fn clear_debug_logs(state: State<'_, AppState>) {
    state.debug_logs.clear();
}
