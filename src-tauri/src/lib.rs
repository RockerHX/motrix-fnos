pub mod app;
pub mod aria2;
pub mod commands;
pub mod config;
pub mod database;
pub mod debug_logs;
pub mod runtime;
pub mod tasks;

use crate::config::aria2::Aria2Config;
use std::io;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_handle = app.handle().clone();
            let (database, restored_tasks, next_task_id) = tauri::async_runtime::block_on(async {
                let path = database::database_path(&app_handle)?;
                let database = database::connect_database(path).await?;
                let restored_tasks = database::tasks::list_download_tasks(&database.pool).await?;
                let max_task_id = database::tasks::max_download_task_id(&database.pool).await?;
                Ok::<_, String>((database, restored_tasks, max_task_id.saturating_add(1)))
            })
            .map_err(io::Error::other)?;
            app.manage(app::AppState::new(database, restored_tasks, next_task_id));
            tauri::async_runtime::spawn(async move {
                start_aria2_after_app_launch(app_handle).await;
            });
            runtime::spawn_task_monitor(app.handle().clone());
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<app::AppState>();
                if state.is_exiting.load(Ordering::SeqCst) {
                    state
                        .debug_logs
                        .info("runtime.window", "应用正在退出，允许关闭主窗口");
                    return;
                }

                api.prevent_close();
                if let Err(error) = window.hide() {
                    state
                        .debug_logs
                        .error("runtime.window", format!("隐藏主窗口失败：{}", error));
                    return;
                }

                state
                    .debug_logs
                    .info("runtime.window", "主窗口已隐藏到后台，下载任务将继续运行");
            }
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
            commands::settings::get_app_config,
            commands::settings::save_app_config,
            commands::settings::get_ui_preferences,
            commands::settings::save_ui_preferences,
            commands::tasks::create_download_task,
            commands::tasks::list_download_tasks,
            commands::tasks::pause_download_task,
            commands::tasks::resume_download_task,
            commands::tasks::delete_download_task
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Reopen { .. } = event {
                show_main_window(app);
            }
        });
}

fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let open = MenuItem::with_id(app, "open-main-window", "打开主界面", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide-main-window", "隐藏主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit-app", "退出 Motrix FNOS", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &hide, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| io::Error::other("应用默认图标未配置"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Motrix FNOS")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open-main-window" => {
                show_main_window(app);
            }
            "hide-main-window" => {
                hide_main_window(app);
            }
            "quit-app" => {
                let state = app.state::<app::AppState>();
                state
                    .debug_logs
                    .info("runtime.tray", "用户通过托盘退出应用");
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.show() {
            let state = app.state::<app::AppState>();
            state
                .debug_logs
                .error("runtime.tray", format!("显示主窗口失败：{}", error));
            return;
        }
        let _ = window.unminimize();
        let _ = window.set_focus();

        let state = app.state::<app::AppState>();
        state
            .debug_logs
            .info("runtime.tray", "已通过托盘打开主界面");
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            let state = app.state::<app::AppState>();
            state
                .debug_logs
                .error("runtime.tray", format!("隐藏主窗口失败：{}", error));
            return;
        }

        let state = app.state::<app::AppState>();
        state
            .debug_logs
            .info("runtime.tray", "已通过托盘隐藏主界面");
    }
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
        if let Err(error) = aria2::start_process(
            &app_handle,
            &state.aria2_process,
            &config,
            &state.debug_logs,
        ) {
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
            drop(state);
            if let Err(error) = refresh_persisted_tasks_after_rpc_ready(&app_handle, &config).await
            {
                let state = app_handle.state::<app::AppState>();
                state
                    .debug_logs
                    .error("tasks.restore", format!("恢复任务状态同步失败：{}", error));
            }
            if let Err(error) = apply_saved_download_config_after_rpc_ready(&app_handle, &config).await
            {
                let state = app_handle.state::<app::AppState>();
                state
                    .debug_logs
                    .warn("settings", format!("应用启动后应用下载配置失败：{}", error));
            }
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

async fn refresh_persisted_tasks_after_rpc_ready(
    app_handle: &tauri::AppHandle,
    config: &Aria2Config,
) -> Result<(), String> {
    let state = app_handle.state::<app::AppState>();
    let tasks =
        tasks::refresh_tasks_from_aria2(&state.download_tasks, config, Some(&state.debug_logs))
            .await?;

    for task in &tasks {
        database::tasks::persist_download_task_state(&state.database.pool, task).await?;
    }

    state.debug_logs.info(
        "tasks.restore",
        format!("应用启动后已同步 {} 个恢复任务状态", tasks.len()),
    );
    Ok(())
}

async fn apply_saved_download_config_after_rpc_ready(
    app_handle: &tauri::AppHandle,
    config: &Aria2Config,
) -> Result<(), String> {
    let state = app_handle.state::<app::AppState>();
    let app_config = commands::settings::load_app_config_from_pool(&state.database.pool).await?;
    let options = aria2::global_options_from_values(
        app_config.max_concurrent_downloads,
        app_config.download_limit,
        app_config.upload_limit,
    );
    aria2::apply_global_options(config, &options, Some(&state.debug_logs)).await
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
