use crate::app::AppState;
use crate::config::aria2::Aria2Config;
use crate::tasks::{
    add_uri_to_aria2, list_tasks, prepare_task, store_created_task, CreateDownloadTaskRequest,
    DownloadTask,
};
use tauri::State;

#[tauri::command]
pub async fn create_download_task(
    state: State<'_, AppState>,
    payload: CreateDownloadTaskRequest,
) -> Result<DownloadTask, String> {
    let prepared = prepare_task(payload)?;
    let gid = add_uri_to_aria2(&Aria2Config::from_env(), &prepared).await?;
    store_created_task(&state.download_tasks, &state.next_task_id, prepared, gid)
}

#[tauri::command]
pub fn list_download_tasks(state: State<'_, AppState>) -> Result<Vec<DownloadTask>, String> {
    list_tasks(&state.download_tasks)
}
