pub mod app;
pub mod aria2;
pub mod commands;
pub mod config;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(app::AppState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::app::ping_backend,
            commands::aria2::get_aria2_config_status,
            commands::aria2::get_aria2_process_status,
            commands::aria2::start_aria2,
            commands::aria2::stop_aria2,
            commands::aria2::ping_aria2_rpc
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
