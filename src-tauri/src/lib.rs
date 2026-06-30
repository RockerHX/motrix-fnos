pub mod app;
pub mod aria2;
pub mod commands;
pub mod config;
pub mod database;
pub mod debug_logs;
pub mod tasks;

use crate::config::aria2::Aria2Config;
use std::io;
use std::time::Duration;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let database = tauri::async_runtime::block_on(async {
                let path = database::database_path(&app_handle)?;
                database::connect_database(path).await
            })
            .map_err(io::Error::other)?;
            app.manage(app::AppState::new(database));
            tauri::async_runtime::spawn(async move {
                start_aria2_after_app_launch(app_handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_app_info,
            commands::app::ping_backend,
            commands::debug_logs::list_debug_logs,
            commands::debug_logs::clear_debug_logs,
            commands::aria2::get_aria2_config_status,
            commands::aria2::get_aria2_process_status,
            commands::aria2::start_aria2,
            commands::aria2::stop_aria2,
            commands::aria2::ping_aria2_rpc,
            commands::tasks::create_download_task,
            commands::tasks::list_download_tasks,
            commands::tasks::pause_download_task,
            commands::tasks::resume_download_task,
            commands::tasks::delete_download_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn start_aria2_after_app_launch(app_handle: tauri::AppHandle) {
    const MAX_ATTEMPTS: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 300;

    let config = Aria2Config::from_env();
    {
        let state = app_handle.state::<app::AppState>();
        state
            .debug_logs
            .info("aria2", "应用启动后自动启动 Aria2 Next");
        if let Err(error) =
            aria2::start_process(&app_handle, &state.aria2_process, &config, &state.debug_logs)
        {
            state.debug_logs.error(
                "aria2",
                format!("应用启动时启动 Aria2 Next 失败：{}", error),
            );
            return;
        }
    }

    let mut last_message = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        let status = aria2::ping_rpc(&config, None).await;
        if status.connected {
            let state = app_handle.state::<app::AppState>();
            state.debug_logs.info(
                "aria2.rpc",
                format!("应用启动后 Aria2 RPC ready，第 {} 次检查成功", attempt + 1),
            );
            return;
        }

        last_message = status.message;
        if attempt + 1 < MAX_ATTEMPTS {
            tokio::time::sleep(Duration::from_millis(RETRY_INTERVAL_MS)).await;
        }
    }

    let state = app_handle.state::<app::AppState>();
    state.debug_logs.error(
        "aria2.rpc",
        format!(
            "应用启动后 Aria2 RPC ready timeout：{}",
            normalize_startup_rpc_message(&last_message)
        ),
    );
}

fn normalize_startup_rpc_message(message: &str) -> &str {
    if message.contains("error sending request")
        || message.contains("Connection refused")
        || message.contains("连接失败")
    {
        "无法连接本地 RPC"
    } else if message.is_empty() {
        "未知错误"
    } else {
        message
    }
}
