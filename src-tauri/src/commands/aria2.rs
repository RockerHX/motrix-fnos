use crate::aria2::Aria2ConfigStatus;
use crate::config::aria2::Aria2Config;

#[tauri::command]
pub fn get_aria2_config_status() -> Aria2ConfigStatus {
    Aria2ConfigStatus::from_config(&Aria2Config::from_env())
}
