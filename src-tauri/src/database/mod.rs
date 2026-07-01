use std::path::PathBuf;
use tauri::Manager;

pub use motrix_fnos_server::database::{connect_database, AppDatabase, DATABASE_FILE_NAME};
pub mod settings {
    pub use motrix_fnos_server::database::settings::*;
}
pub mod tasks {
    pub use motrix_fnos_server::database::tasks::*;
}

pub fn database_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("读取应用数据目录失败：{}", error))?;

    Ok(app_data_dir.join(DATABASE_FILE_NAME))
}
