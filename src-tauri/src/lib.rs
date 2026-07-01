pub mod app;
pub mod aria2;
pub mod commands;
pub mod config;
pub mod database;
pub mod debug_logs;
pub mod runtime;
pub mod tasks;

use crate::config::aria2::Aria2Config;
use crate::database::tasks::{persist_download_task_state, persist_download_task_states};
use serde::Serialize;
use std::io;
use std::sync::atomic::Ordering;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, RunEvent, WindowEvent};

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
        .menu(|app| build_app_menu(app))
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "app-quit" {
                request_application_exit(app, "用户通过 macOS 应用菜单退出应用");
            }
        })
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
            commands::app::quit_app,
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
            commands::tasks::redownload_download_task,
            commands::tasks::delete_download_task
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main_window(app),
            RunEvent::ExitRequested { api, .. } => {
                let state = app.state::<app::AppState>();
                if !state.is_exiting.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    request_application_exit(app, "系统退出请求");
                }
            }
            _ => {}
        });
}

fn build_app_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let quit = MenuItem::with_id(
        app,
        "app-quit",
        "Quit motrix-fnos",
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    let app_menu = SubmenuBuilder::new(app, "motrix-fnos")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&quit)
        .build()?;

    MenuBuilder::new(app).item(&app_menu).build()
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
                request_application_exit(app, "用户通过托盘退出应用");
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeExitingPayload {
    reason: String,
    timestamp: u64,
}

pub(crate) fn request_application_exit(app: &tauri::AppHandle, reason: &str) {
    let state = app.state::<app::AppState>();
    if state.is_exiting.swap(true, Ordering::SeqCst) {
        state
            .debug_logs
            .info("runtime.exit", "应用退出流程已在执行，忽略重复退出请求");
        return;
    }

    state.debug_logs.info("runtime.exit", reason);
    if let Err(error) = app.emit(
        "runtime://exiting",
        RuntimeExitingPayload {
            reason: reason.to_string(),
            timestamp: current_timestamp_ms(),
        },
    ) {
        state
            .debug_logs
            .warn("runtime.exit", format!("发送退出事件失败：{}", error));
    }
    let app_handle = app.clone();
    tauri::async_runtime::block_on(async move {
        run_application_exit(app_handle).await;
    });
}

async fn sync_tasks_before_exit(app: &tauri::AppHandle) {
    let state = app.state::<app::AppState>();
    let config = state.aria2_config();
    match tasks::refresh_tasks_from_aria2(&state.download_tasks, &config, Some(&state.debug_logs))
        .await
    {
        Ok(tasks) => {
            if let Err(error) = persist_download_task_states(&state.database.pool, &tasks).await {
                state.debug_logs.error(
                    "runtime.exit",
                    format!("退出前保存最新任务状态失败：{}", error),
                );
            } else {
                state.debug_logs.info(
                    "runtime.exit",
                    format!("退出前已同步并保存 {} 个任务状态", tasks.len()),
                );
            }
        }
        Err(error) => {
            state.debug_logs.warn(
                "runtime.exit",
                format!("退出前同步 Aria2 状态失败，将保存应用内最后状态：{}", error),
            );
            match tasks::list_tasks(&state.download_tasks) {
                Ok(tasks) => {
                    if let Err(error) =
                        persist_download_task_states(&state.database.pool, &tasks).await
                    {
                        state.debug_logs.error(
                            "runtime.exit",
                            format!("退出前保存最后已知任务状态失败：{}", error),
                        );
                    }
                }
                Err(error) => state
                    .debug_logs
                    .error("runtime.exit", format!("退出前读取任务快照失败：{}", error)),
            }
        }
    }
}

async fn pause_unfinished_tasks_before_exit(app: &tauri::AppHandle) {
    let state = app.state::<app::AppState>();
    let config = state.aria2_config();
    let candidates = match tasks::list_tasks(&state.download_tasks) {
        Ok(tasks) => tasks
            .into_iter()
            .filter(tasks::should_pause_task_on_exit)
            .filter_map(|task| task.gid.map(|gid| (task.id, gid)))
            .collect::<Vec<_>>(),
        Err(error) => {
            state.debug_logs.error(
                "runtime.exit",
                format!("退出前读取待暂停任务失败：{}", error),
            );
            return;
        }
    };

    if candidates.is_empty() {
        state
            .debug_logs
            .info("runtime.exit", "退出前没有可通过 RPC 暂停的未完成任务");
    }

    let mut rpc_paused_count = 0;
    for (task_id, gid) in candidates {
        match tasks::pause_task(&config, &gid, Some(&state.debug_logs)).await {
            Ok(_) => rpc_paused_count += 1,
            Err(error) => state.debug_logs.warn(
                "runtime.exit",
                format!(
                    "退出前 RPC 暂停任务失败，仍会把任务保存为暂停态，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            ),
        }
    }

    let paused_tasks = match tasks::mark_unfinished_tasks_paused(&state.download_tasks) {
        Ok(tasks) => tasks,
        Err(error) => {
            state.debug_logs.error(
                "runtime.exit",
                format!("退出前标记未完成任务暂停失败：{}", error),
            );
            return;
        }
    };

    let tasks = match tasks::list_tasks(&state.download_tasks) {
        Ok(tasks) => tasks,
        Err(error) => {
            state.debug_logs.error(
                "runtime.exit",
                format!("退出前读取暂停后任务状态失败：{}", error),
            );
            return;
        }
    };

    if let Err(error) = persist_download_task_states(&state.database.pool, &tasks).await {
        state.debug_logs.error(
            "runtime.exit",
            format!("退出前保存暂停任务状态失败：{}", error),
        );
    } else {
        state.debug_logs.info(
            "runtime.exit",
            format!(
                "退出前已保存 {} 个未完成任务为暂停态，RPC 成功暂停 {} 个",
                paused_tasks.len(),
                rpc_paused_count
            ),
        );
    }
}

async fn save_aria2_session_before_exit(app: &tauri::AppHandle) {
    let state = app.state::<app::AppState>();
    let config = state.aria2_config();
    match aria2::save_session(&config, Some(&state.debug_logs)).await {
        Ok(()) => state
            .debug_logs
            .info("runtime.exit", "退出前已请求 Aria2 保存 session"),
        Err(error) => state.debug_logs.warn(
            "runtime.exit",
            format!("退出前保存 Aria2 session 失败，继续退出：{}", error),
        ),
    }
}

async fn run_application_exit(app: tauri::AppHandle) {
    {
        let state = app.state::<app::AppState>();
        state
            .debug_logs
            .info("runtime.exit", "开始执行统一退出流程");
    }

    sync_tasks_before_exit(&app).await;
    pause_unfinished_tasks_before_exit(&app).await;
    save_aria2_session_before_exit(&app).await;

    let should_clear_runtime = {
        let state = app.state::<app::AppState>();
        match aria2::stop_process(&state.aria2_process, &state.debug_logs) {
            Ok(status) => {
                state.debug_logs.info(
                    "runtime.exit",
                    format!("退出流程已停止 Aria2：{}", status.message),
                );
                true
            }
            Err(error) => {
                state.debug_logs.warn(
                    "runtime.exit",
                    format!(
                        "退出流程停止 Aria2 失败，将保留运行态记录供下次启动清理：{}",
                        error
                    ),
                );
                false
            }
        }
    };

    if should_clear_runtime {
        let state = app.state::<app::AppState>();
        state.clear_aria2_runtime();
    }

    app.exit(0);
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

fn runtime_aria2_config(app: &tauri::AppHandle) -> Result<Aria2Config, String> {
    let base = Aria2Config::from_env();
    let state = app.state::<app::AppState>();
    let saved_runtime = state.load_saved_aria2_runtime();
    let port =
        aria2::select_rpc_port_with_saved_runtime(&base, saved_runtime.as_ref(), &state.debug_logs)
            .ok_or_else(aria2::rpc_ports_exhausted_message)?;
    state.with_aria2_runtime_paths(aria2::runtime_config(
        &base,
        port,
        aria2::generate_rpc_secret(),
    ))
}

async fn start_aria2_after_app_launch(app_handle: tauri::AppHandle) {
    const MAX_ATTEMPTS: usize = 10;
    const RETRY_INTERVAL_MS: u64 = 300;

    force_pause_unfinished_tasks_on_startup(&app_handle).await;

    let config = match runtime_aria2_config(&app_handle) {
        Ok(config) => config,
        Err(error) => {
            let state = app_handle.state::<app::AppState>();
            state.debug_logs.error("aria2", &error);
            return;
        }
    };
    {
        let state = app_handle.state::<app::AppState>();
        state
            .debug_logs
            .info("aria2", "应用启动后自动启动 Aria2 Next");
        match aria2::start_process(
            &app_handle,
            &state.aria2_process,
            &config,
            &state.debug_logs,
        ) {
            Ok(status) => {
                if let Some(pid) = status.pid {
                    if let Some(source) = status.binary_source.clone() {
                        if let Err(error) = state.set_aria2_runtime(state.build_aria2_runtime_info(
                            pid,
                            &config,
                            source,
                            crate::aria2::process_args(&config),
                        )) {
                            state.debug_logs.warn("aria2", error);
                        }
                    }
                }
            }
            Err(error) => {
                state.debug_logs.error(
                    "aria2",
                    format!("应用启动时启动 Aria2 Next 失败：{}", error),
                );
                return;
            }
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
            if let Err(error) = sync_session_tasks_after_rpc_ready(&app_handle, &config).await {
                let state = app_handle.state::<app::AppState>();
                state.debug_logs.warn(
                    "tasks.restore",
                    format!(
                        "Aria2 session 任务同步失败，保留 SQLite 恢复路径：{}",
                        error
                    ),
                );
            }
            if let Err(error) = refresh_persisted_tasks_after_rpc_ready(&app_handle, &config).await
            {
                let state = app_handle.state::<app::AppState>();
                state
                    .debug_logs
                    .error("tasks.restore", format!("恢复任务状态同步失败：{}", error));
            }
            if let Err(error) =
                apply_saved_download_config_after_rpc_ready(&app_handle, &config).await
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

async fn force_pause_unfinished_tasks_on_startup(app_handle: &tauri::AppHandle) {
    let state = app_handle.state::<app::AppState>();
    let paused_tasks = match tasks::mark_unfinished_tasks_paused(&state.download_tasks) {
        Ok(tasks) => tasks,
        Err(error) => {
            state.debug_logs.error(
                "tasks.restore",
                format!("启动时兜底暂停未完成任务失败：{}", error),
            );
            return;
        }
    };

    if paused_tasks.is_empty() {
        return;
    }

    for task in &paused_tasks {
        if let Err(error) = persist_download_task_state(&state.database.pool, task).await {
            state.debug_logs.error(
                "tasks.restore",
                format!("启动时保存兜底暂停任务失败，ID {}：{}", task.id, error),
            );
        }
    }

    state.debug_logs.warn(
        "tasks.restore",
        format!(
            "启动时已将 {} 个上次未完成任务兜底恢复为暂停态，避免自动继续下载",
            paused_tasks.len()
        ),
    );
}

async fn sync_session_tasks_after_rpc_ready(
    app_handle: &tauri::AppHandle,
    config: &Aria2Config,
) -> Result<(), String> {
    let state = app_handle.state::<app::AppState>();
    let tasks = tasks::sync_session_tasks_from_aria2(
        &state.download_tasks,
        config,
        Some(&state.debug_logs),
    )
    .await?;
    persist_download_task_states(&state.database.pool, &tasks).await?;
    Ok(())
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

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
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
