use crate::app::AppState;
use crate::tasks::{create_task, list_tasks, CreateDownloadTaskRequest, DownloadTask};
use tauri::State;

#[tauri::command]
pub fn create_download_task(
    state: State<'_, AppState>,
    payload: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    create_task(&state.download_tasks, &state.next_task_id, payload)
}

#[tauri::command]
pub fn list_download_tasks(state: State<'_, AppState>) -> Result<Vec<DownloadTask>, String> {
    list_tasks(&state.download_tasks)
}
