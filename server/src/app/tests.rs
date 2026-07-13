use super::*;
use crate::database::tasks::upsert_download_task;
use crate::tasks::{DownloadTask, DownloadTaskStatus};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn runtime_config_uses_explicit_env_values() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-app-config-{}", now_ms()));
    let aria2_path = temp_dir.join("aria2-next");
    let gateway_socket_path = temp_dir.join("motrix-fnos.sock");

    std::env::set_var(APP_DATA_DIR_ENV, &temp_dir);
    std::env::set_var(HTTP_ADDR_ENV, "127.0.0.1:18080");
    std::env::set_var(ARIA2_PATH_ENV, &aria2_path);
    std::env::set_var(GATEWAY_SOCKET_ENV, &gateway_socket_path);
    std::env::remove_var(ACCESSIBLE_PATHS_FILE_ENV);

    let config = ServerRuntimeConfig::from_env().expect("config should load");

    assert_eq!(config.app_data_dir, temp_dir);
    assert_eq!(
        config.database_path,
        config.app_data_dir.join(DATABASE_FILE_NAME)
    );
    assert_eq!(
        config.accessible_paths_path,
        config.app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME)
    );
    assert_eq!(config.http_addr.to_string(), "127.0.0.1:18080");
    assert_eq!(config.aria2_path.as_deref(), Some(aria2_path.as_path()));
    assert_eq!(
        config.gateway_socket_path.as_deref(),
        Some(gateway_socket_path.as_path())
    );

    std::env::remove_var(APP_DATA_DIR_ENV);
    std::env::remove_var(HTTP_ADDR_ENV);
    std::env::remove_var(ARIA2_PATH_ENV);
    std::env::remove_var(GATEWAY_SOCKET_ENV);
    std::env::remove_var(ACCESSIBLE_PATHS_FILE_ENV);
}

#[test]
fn bootstrap_http_app_state_restores_database_state() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async {
            let app_data_dir =
                std::env::temp_dir().join(format!("motrix-fnos-http-app-state-{}", now_ms()));
            let runtime = ServerRuntimeConfig {
                database_path: app_data_dir.join(DATABASE_FILE_NAME),
                accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
                app_data_dir: app_data_dir.clone(),
                http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
                gateway_socket_path: None,
                aria2_path: None,
            };

            let database = connect_database(runtime.database_path.clone())
                .await
                .expect("database should connect");
            let task = sample_task();
            upsert_download_task(&database.pool, &task)
                .await
                .expect("task should persist");
            database.pool.close().await;

            let state = bootstrap_http_app_state(&runtime)
                .await
                .expect("state should bootstrap");

            let tasks = state.core.download_tasks.list().expect("tasks should lock");

            assert_eq!(state.runtime.app_data_dir, app_data_dir);
            assert_eq!(state.runtime.http_addr.to_string(), DEFAULT_HTTP_ADDR);
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].id, task.id);
            assert_eq!(state.core.next_task_id.load(Ordering::SeqCst), task.id + 1);

            state.core.database.pool.close().await;
            let _ = std::fs::remove_file(&runtime.database_path);
            let _ = std::fs::remove_dir_all(&runtime.app_data_dir);
        });
}

#[test]
fn request_shutdown_marks_exiting_and_broadcasts_event() {
    let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-shutdown-{}", now_ms()));
    let runtime = ServerRuntimeConfig {
        database_path: temp_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: temp_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        app_data_dir: temp_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        gateway_socket_path: None,
        aria2_path: None,
    };
    let database = tokio::runtime::Runtime::new()
        .expect("tokio runtime should create")
        .block_on(async { connect_database(runtime.database_path.clone()).await })
        .expect("database should connect");
    let state = HttpAppState::new(ServerState::new(database, Vec::new(), 1), runtime);
    let mut receiver = state.runtime_events.subscribe();

    state.request_shutdown("收到停止信号");

    assert!(state.core.shutdown.is_exiting());
    let event = receiver.try_recv().expect("event should be broadcast");
    match event {
        RuntimeEvent::RuntimeExiting(payload) => {
            assert_eq!(payload.reason, "收到停止信号");
            assert!(payload.timestamp > 0);
        }
        other => panic!("unexpected event: {:?}", other),
    }
}

#[test]
fn reconcile_magnet_metadata_dirs_keeps_pending_magnet_metadata_dir() {
    let app_data_dir =
        std::env::temp_dir().join(format!("motrix-fnos-reconcile-pending-{}", now_ms()));
    let metadata_dir = app_data_dir.join("magnet-metadata").join("task-9");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir should create");
    std::fs::write(metadata_dir.join("pending.torrent"), b"torrent").expect("torrent should write");
    let mut tasks = vec![DownloadTask {
        id: 9,
        url: "magnet:?xt=urn:btih:test".to_string(),
        file_name: "磁力链接任务".to_string(),
        save_dir: "/downloads".to_string(),
        category: "默认".to_string(),
        gid: Some("gid-9".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        metadata_torrent_path: None,
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }];

    reconcile_magnet_metadata_dirs(&app_data_dir, &mut tasks).expect("reconcile should succeed");

    assert!(metadata_dir.exists());
    assert_eq!(tasks[0].status, DownloadTaskStatus::Pending);

    let _ = std::fs::remove_dir_all(&app_data_dir);
}

#[test]
fn reconcile_magnet_metadata_dirs_marks_pending_magnet_task_error_when_dir_missing() {
    let app_data_dir =
        std::env::temp_dir().join(format!("motrix-fnos-reconcile-missing-{}", now_ms()));
    std::fs::create_dir_all(app_data_dir.join("magnet-metadata"))
        .expect("metadata root should create");
    let mut tasks = vec![DownloadTask {
        id: 10,
        url: "magnet:?xt=urn:btih:test".to_string(),
        file_name: "磁力链接任务".to_string(),
        save_dir: "/downloads".to_string(),
        category: "默认".to_string(),
        gid: Some("gid-10".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        metadata_torrent_path: None,
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }];

    reconcile_magnet_metadata_dirs(&app_data_dir, &mut tasks).expect("reconcile should succeed");

    assert_eq!(tasks[0].status, DownloadTaskStatus::Error);
    assert!(tasks[0].gid.is_none());
    assert_eq!(
        tasks[0].error_message.as_deref(),
        Some("磁链 metadata 临时目录丢失，请重新添加磁链")
    );

    let _ = std::fs::remove_dir_all(&app_data_dir);
}

fn sample_task() -> DownloadTask {
    DownloadTask {
        id: 7,
        url: "https://example.com/archive.zip".to_string(),
        file_name: "archive.zip".to_string(),
        save_dir: "/downloads".to_string(),
        category: "默认".to_string(),
        gid: Some("gid-7".to_string()),
        status: DownloadTaskStatus::Paused,
        total_length: 1024,
        completed_length: 512,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        metadata_torrent_path: None,
        confirmation_required: false,
        files: Vec::new(),
        created_at: 100,
        updated_at: 101,
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_millis()
}
