use super::*;
use crate::aria2::ARIA2_LOG_MAX_BYTES;
use crate::database::task_operations::{begin_task_operation, list_unfinished_task_operations};
use crate::database::tasks::list_download_tasks;
use crate::database::tasks::upsert_download_task;
use crate::settings::service::{load_json_rpc_token, save_json_rpc_token};
use crate::tasks::{
    DownloadTask, DownloadTaskStatus, TaskOperation, TaskOperationContext, TaskOperationType,
};
use std::sync::atomic::Ordering;
use std::sync::OnceLock;

pub(crate) fn replace_fnos_api_client(state: &HttpAppState, client: FnosApiClient) {
    *state
        .fnos_api_client
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = client;
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn runtime_config_uses_explicit_env_values() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-app-config-{}", now_ms()));
    let aria2_path = temp_dir.join("aria2-next");

    std::env::set_var(APP_DATA_DIR_ENV, &temp_dir);
    std::env::set_var(HTTP_ADDR_ENV, "127.0.0.1:18080");
    std::env::set_var(JSONRPC_ADDR_ENV, "127.1.2.3:18081");
    std::env::set_var(LAN_JSONRPC_ADDR_ENV, "127.0.0.1:18082");
    std::env::set_var(ARIA2_PATH_ENV, &aria2_path);
    std::env::remove_var(ACCESSIBLE_PATHS_FILE_ENV);
    std::env::set_var(TRUSTED_PROXY_IPS_ENV, "192.0.2.10, ::1, 192.0.2.10");

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
    assert_eq!(config.jsonrpc_addr.to_string(), "127.1.2.3:18081");
    assert_eq!(config.lan_jsonrpc_addr.to_string(), "127.0.0.1:18082");
    assert_eq!(config.aria2_path.as_deref(), Some(aria2_path.as_path()));
    assert_eq!(
        config.trusted_proxy_ips,
        vec![
            "192.0.2.10"
                .parse::<std::net::IpAddr>()
                .expect("proxy should parse"),
            "::1"
                .parse::<std::net::IpAddr>()
                .expect("proxy should parse"),
        ]
    );

    std::env::remove_var(APP_DATA_DIR_ENV);
    std::env::remove_var(HTTP_ADDR_ENV);
    std::env::remove_var(JSONRPC_ADDR_ENV);
    std::env::remove_var(LAN_JSONRPC_ADDR_ENV);
    std::env::remove_var(ARIA2_PATH_ENV);
    std::env::remove_var(ACCESSIBLE_PATHS_FILE_ENV);
    std::env::remove_var(TRUSTED_PROXY_IPS_ENV);
}

#[test]
fn runtime_config_uses_default_listener_addresses() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-default-config-{}", now_ms()));
    std::env::set_var(APP_DATA_DIR_ENV, &temp_dir);
    std::env::remove_var(HTTP_ADDR_ENV);
    std::env::remove_var(JSONRPC_ADDR_ENV);
    std::env::remove_var(LAN_JSONRPC_ADDR_ENV);
    std::env::remove_var(TRUSTED_PROXY_IPS_ENV);

    let config = ServerRuntimeConfig::from_env().expect("config should load");

    assert_eq!(config.http_addr.to_string(), DEFAULT_HTTP_ADDR);
    assert_eq!(config.jsonrpc_addr.to_string(), DEFAULT_JSONRPC_ADDR);
    assert_eq!(
        config.lan_jsonrpc_addr.to_string(),
        DEFAULT_LAN_JSONRPC_ADDR
    );

    std::env::remove_var(APP_DATA_DIR_ENV);
}

#[test]
fn runtime_config_rejects_invalid_lan_jsonrpc_address() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    std::env::set_var(LAN_JSONRPC_ADDR_ENV, "not-an-address");

    let error = ServerRuntimeConfig::from_env().expect_err("address should be rejected");

    assert!(error.contains("解析局域网 JSON-RPC 监听地址失败"));
    std::env::remove_var(LAN_JSONRPC_ADDR_ENV);
}

#[test]
fn runtime_config_rejects_invalid_or_non_loopback_jsonrpc_addresses() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    for (value, expected_message) in [
        ("not-an-address", "解析 JSON-RPC 监听地址失败"),
        ("0.0.0.0:17081", "JSON-RPC 监听地址必须使用回环 IP"),
        ("192.168.1.10:17081", "JSON-RPC 监听地址必须使用回环 IP"),
        ("203.0.113.10:17081", "JSON-RPC 监听地址必须使用回环 IP"),
    ] {
        std::env::set_var(JSONRPC_ADDR_ENV, value);
        let error = ServerRuntimeConfig::from_env().expect_err("address should be rejected");
        assert!(error.contains(expected_message), "error: {error}");
    }
    std::env::remove_var(JSONRPC_ADDR_ENV);
}

#[test]
fn runtime_config_accepts_ipv4_and_ipv6_loopback_jsonrpc_addresses() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    for value in ["127.0.0.1:17081", "[::1]:17081"] {
        std::env::set_var(JSONRPC_ADDR_ENV, value);
        let config = ServerRuntimeConfig::from_env().expect("loopback address should load");
        assert!(config.jsonrpc_addr.ip().is_loopback());
    }
    std::env::remove_var(JSONRPC_ADDR_ENV);
}

#[test]
fn runtime_config_rejects_invalid_trusted_proxy_addresses() {
    let _guard = env_lock().lock().expect("env lock should succeed");
    std::env::set_var(TRUSTED_PROXY_IPS_ENV, "192.0.2.10,not-an-ip");

    let error = ServerRuntimeConfig::from_env().expect_err("proxy address should be rejected");

    assert!(error.contains("解析可信代理地址失败"));
    std::env::remove_var(TRUSTED_PROXY_IPS_ENV);
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
                jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
                lan_jsonrpc_addr: DEFAULT_LAN_JSONRPC_ADDR.parse().expect("addr should parse"),
                aria2_path: None,
                trusted_proxy_ips: Vec::new(),
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

#[tokio::test]
async fn bootstrap_without_pending_file_cleanup_keeps_state_exclusively_owned() {
    let app_data_dir = std::env::temp_dir().join(format!(
        "motrix-fnos-bootstrap-no-file-cleanup-{}",
        now_ms()
    ));
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        app_data_dir: app_data_dir.clone(),
        http_addr: "127.0.0.1:0".parse().expect("address should parse"),
        jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };

    let mut state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");

    assert!(std::sync::Arc::get_mut(&mut state).is_some());

    state.core.database.pool.close().await;
    drop(state);
    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[tokio::test]
async fn bootstrap_trims_oversized_aria2_log_without_starting_aria2() {
    let app_data_dir = std::env::temp_dir().join(format!(
        "motrix-fnos-bootstrap-log-maintenance-{}",
        now_ms()
    ));
    let log_path = app_data_dir.join("aria2").join("aria2.log");
    std::fs::create_dir_all(log_path.parent().expect("log should have parent"))
        .expect("log directory should create");
    let log = std::fs::File::create(&log_path).expect("log should create");
    log.set_len(ARIA2_LOG_MAX_BYTES + 1)
        .expect("log should size");
    drop(log);
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        app_data_dir: app_data_dir.clone(),
        http_addr: "127.0.0.1:0".parse().expect("address should parse"),
        jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("address should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };

    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");

    assert_eq!(
        std::fs::metadata(&log_path)
            .expect("log should exist")
            .len(),
        ARIA2_LOG_MAX_BYTES
    );
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
    assert!(state.aria2_runtime_snapshot().is_none());

    state.core.database.pool.close().await;
    drop(state);
    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[tokio::test]
async fn bootstrap_reconciles_prepared_operation_without_starting_aria2() {
    let app_data_dir =
        std::env::temp_dir().join(format!("motrix-fnos-bootstrap-prepared-{}", now_ms()));
    let runtime = listener_runtime(
        "127.0.0.1:0".parse().expect("address should parse"),
        "127.0.0.1:0".parse().expect("address should parse"),
    );
    let runtime = ServerRuntimeConfig {
        app_data_dir: app_data_dir.clone(),
        database_path: app_data_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        ..runtime
    };
    let database = connect_database(runtime.database_path.clone())
        .await
        .expect("database should connect");
    let task = sample_task();
    upsert_download_task(&database.pool, &task)
        .await
        .expect("task should persist");
    let operation = TaskOperation::with_id(
        "bootstrap-prepared-operation",
        task.id,
        TaskOperationType::Create,
        "prepared",
        TaskOperationContext::default(),
    );
    begin_task_operation(&database.pool, &operation)
        .await
        .expect("operation should persist");
    database.pool.close().await;

    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("prepared operation should reconcile");

    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(list_unfinished_task_operations(&state.core.database.pool)
        .await
        .expect("unfinished operations should list")
        .is_empty());

    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[tokio::test]
async fn bootstrap_preserves_manual_review_after_restart_without_starting_aria2() {
    let app_data_dir =
        std::env::temp_dir().join(format!("motrix-fnos-bootstrap-manual-{}", now_ms()));
    let runtime = listener_runtime(
        "127.0.0.1:0".parse().expect("address should parse"),
        "127.0.0.1:0".parse().expect("address should parse"),
    );
    let runtime = ServerRuntimeConfig {
        app_data_dir: app_data_dir.clone(),
        database_path: app_data_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        ..runtime
    };
    let database = connect_database(runtime.database_path.clone())
        .await
        .expect("database should connect");
    let task = sample_task();
    upsert_download_task(&database.pool, &task)
        .await
        .expect("task should persist");
    let mut operation = TaskOperation::with_id(
        "bootstrap-manual-review-operation",
        task.id,
        TaskOperationType::Redownload,
        "files_staged",
        TaskOperationContext::default(),
    );
    operation.require_manual_review("startup_manual_review", "用户文件需要人工确认");
    begin_task_operation(&database.pool, &operation)
        .await
        .expect("manual review operation should persist");
    database.pool.close().await;

    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("manual review should survive restart");

    assert!(state.aria2_runtime_snapshot().is_none());
    let unfinished = list_unfinished_task_operations(&state.core.database.pool)
        .await
        .expect("unfinished operations should list");
    assert_eq!(unfinished.len(), 1);
    assert_eq!(
        unfinished[0].status,
        crate::tasks::TaskOperationStatus::ManualReview
    );

    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(runtime.app_data_dir);
}

#[test]
fn request_shutdown_marks_exiting_and_broadcasts_event() {
    let temp_dir = std::env::temp_dir().join(format!("motrix-fnos-shutdown-{}", now_ms()));
    let runtime = ServerRuntimeConfig {
        database_path: temp_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: temp_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        app_data_dir: temp_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: DEFAULT_LAN_JSONRPC_ADDR.parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
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

#[tokio::test]
async fn listener_binding_fails_without_starting_jsonrpc_when_management_port_is_occupied() {
    let occupied_management = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("management port should bind");
    let management_addr = occupied_management
        .local_addr()
        .expect("management address should read");
    let jsonrpc_addr = reserve_local_addr().await;
    let runtime = listener_runtime(management_addr, jsonrpc_addr);

    let error = bind_http_listeners(&runtime)
        .await
        .expect_err("management binding should fail");

    assert!(error.contains("绑定管理监听地址失败"));
    assert!(error.contains(&management_addr.to_string()));
    TcpListener::bind(jsonrpc_addr)
        .await
        .expect("jsonrpc port should remain available");
}

#[tokio::test]
async fn listener_binding_releases_management_when_jsonrpc_port_is_occupied() {
    let management_addr = reserve_local_addr().await;
    let occupied_jsonrpc = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("jsonrpc port should bind");
    let jsonrpc_addr = occupied_jsonrpc
        .local_addr()
        .expect("jsonrpc address should read");
    let runtime = listener_runtime(management_addr, jsonrpc_addr);

    let error = bind_http_listeners(&runtime)
        .await
        .expect_err("jsonrpc binding should fail");

    assert!(error.contains("绑定 JSON-RPC 监听地址失败"));
    assert!(error.contains(&jsonrpc_addr.to_string()));
    TcpListener::bind(management_addr)
        .await
        .expect("management port should be released after failure");
}

#[tokio::test]
async fn listener_binding_releases_other_ports_when_lan_jsonrpc_port_is_occupied() {
    let management_addr = reserve_local_addr().await;
    let jsonrpc_addr = reserve_local_addr().await;
    let occupied_lan_jsonrpc = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("LAN JSON-RPC port should bind");
    let lan_jsonrpc_addr = occupied_lan_jsonrpc
        .local_addr()
        .expect("LAN JSON-RPC address should read");
    let mut runtime = listener_runtime(management_addr, jsonrpc_addr);
    runtime.lan_jsonrpc_addr = lan_jsonrpc_addr;

    let error = bind_http_listeners(&runtime)
        .await
        .expect_err("LAN JSON-RPC binding should fail");

    assert!(error.contains("绑定局域网 JSON-RPC 监听地址失败"));
    assert!(error.contains(&lan_jsonrpc_addr.to_string()));
    TcpListener::bind(management_addr)
        .await
        .expect("management port should be released after failure");
    TcpListener::bind(jsonrpc_addr)
        .await
        .expect("JSON-RPC port should be released after failure");
}

#[tokio::test]
async fn three_listeners_serve_isolated_routes_and_cleanup_once() {
    let runtime = listener_runtime(
        "127.0.0.1:0".parse().expect("address should parse"),
        "127.0.0.1:0".parse().expect("address should parse"),
    );
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    let listeners = bind_http_listeners(&runtime)
        .await
        .expect("listeners should bind");
    state.mark_listeners_ready();
    let management_addr = listeners
        .management
        .local_addr()
        .expect("management address should read");
    let jsonrpc_addr = listeners
        .jsonrpc
        .local_addr()
        .expect("jsonrpc address should read");
    let lan_jsonrpc_addr = listeners
        .lan_jsonrpc
        .local_addr()
        .expect("LAN JSON-RPC address should read");
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel::<String>();
    let serving_state = state.clone();
    let server = tokio::spawn(async move {
        serve_http_listeners(serving_state, listeners, async move {
            shutdown_receiver
                .await
                .map_err(|error| format!("测试停止信号丢失：{}", error))
        })
        .await
    });
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{management_addr}/api/app/ping"))
        .send()
        .await
        .expect("management API should respond");
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    let response = client
        .get(format!("http://{management_addr}/api/auth/status"))
        .send()
        .await
        .expect("management auth status should respond");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response = client
        .get(format!("http://{management_addr}/api/app/ready"))
        .send()
        .await
        .expect("management readiness should respond");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let response = client
        .get(format!("http://{jsonrpc_addr}/api/app/ping"))
        .send()
        .await
        .expect("jsonrpc management path should respond");
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    let response = client
        .post(format!("http://{jsonrpc_addr}/jsonrpc"))
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "rpc-version",
            "method": "system.unsupported",
            "params": []
        }))
        .send()
        .await
        .expect("jsonrpc request should respond");
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let payload = response
        .json::<serde_json::Value>()
        .await
        .expect("jsonrpc response should deserialize");
    assert_eq!(payload["id"], "rpc-version");
    assert_eq!(payload["error"]["code"], -32601);

    let response = client
        .post(format!("http://{lan_jsonrpc_addr}/jsonrpc"))
        .body("not-json")
        .send()
        .await
        .expect("disabled LAN JSON-RPC should respond");
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    *state.lan_json_rpc_config.write().await = crate::settings::service::LanJsonRpcConfig {
        enabled: true,
        token: "lan-secret".to_string(),
    };
    let response = client
        .post(format!("http://{lan_jsonrpc_addr}/jsonrpc"))
        .header("x-forwarded-for", "192.168.1.12")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "lan-loopback",
            "method": "aria2.getVersion",
            "params": []
        }))
        .send()
        .await
        .expect("loopback LAN JSON-RPC should respond");
    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);

    shutdown_sender
        .send("测试三监听器停止".to_string())
        .expect("shutdown signal should send");
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .expect("server should stop before timeout")
        .expect("server task should join");
    assert_eq!(result, Ok(()));

    drop(client);
    assert_listener_closed(management_addr).await;
    assert_listener_closed(jsonrpc_addr).await;
    assert_listener_closed(lan_jsonrpc_addr).await;
    let cleanup_count = state
        .core
        .debug_logs
        .list()
        .iter()
        .filter(|entry| entry.module == "runtime.exit" && entry.message == "开始执行统一退出流程")
        .count();
    assert_eq!(cleanup_count, 1);
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
        source_type: crate::tasks::DownloadTaskSourceType::Magnet,
        file_name: "磁力链接任务".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-9".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
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
        source_type: crate::tasks::DownloadTaskSourceType::Magnet,
        file_name: "磁力链接任务".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-10".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
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

#[test]
fn reconcile_missing_magnet_metadata_preserves_user_download_files() {
    let app_data_dir =
        std::env::temp_dir().join(format!("motrix-fnos-reconcile-file-{}", now_ms()));
    let download_dir = app_data_dir.join("downloads");
    let download_file = download_dir.join("user-file.bin");
    std::fs::create_dir_all(&download_dir).expect("download dir should create");
    std::fs::write(&download_file, b"user data").expect("user file should write");
    std::fs::create_dir_all(app_data_dir.join("magnet-metadata"))
        .expect("metadata root should create");
    let mut tasks = vec![DownloadTask {
        id: 11,
        url: "magnet:?xt=urn:btih:test".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Magnet,
        file_name: "磁力链接任务".to_string(),
        save_dir: download_dir.display().to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-11".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }];

    reconcile_magnet_metadata_dirs(&app_data_dir, &mut tasks)
        .expect("reconcile should preserve the user file");

    assert_eq!(tasks[0].status, DownloadTaskStatus::Error);
    assert!(download_file.is_file());
    assert_eq!(
        std::fs::read(&download_file).expect("user file should remain readable"),
        b"user data"
    );

    let _ = std::fs::remove_dir_all(&app_data_dir);
}

#[tokio::test]
async fn reset_web_auth_requires_stopped_server_and_preserves_application_data() {
    let runtime = listener_runtime(
        "127.0.0.1:0".parse().expect("addr should parse"),
        "127.0.0.1:0".parse().expect("addr should parse"),
    );
    std::fs::create_dir_all(&runtime.app_data_dir).expect("app data should create");
    let database = connect_database(runtime.database_path.clone())
        .await
        .expect("database should connect");
    let auth = AuthService::new(database.pool.clone());
    auth.setup("test management password")
        .await
        .expect("auth should initialize");
    upsert_download_task(&database.pool, &sample_task())
        .await
        .expect("task should save");
    save_json_rpc_token(&database.pool, "preserved-rpc-token")
        .await
        .expect("token should save");
    database.pool.close().await;

    let accessible_contents = br#"{"paths":["/downloads"]}"#;
    std::fs::write(&runtime.accessible_paths_path, accessible_contents)
        .expect("accessible paths should write");
    let aria2_session = runtime.app_data_dir.join("aria2").join("aria2.session");
    std::fs::create_dir_all(aria2_session.parent().expect("parent should exist"))
        .expect("aria2 dir should create");
    std::fs::write(&aria2_session, b"session-record").expect("session should write");

    let running_lock =
        ServerProcessLock::acquire(&runtime.app_data_dir).expect("server lock should acquire");
    let error = reset_web_auth_with_runtime(&runtime)
        .await
        .expect_err("running server should block reset");
    assert!(error.contains("正在运行"));
    drop(running_lock);

    reset_web_auth_with_runtime(&runtime)
        .await
        .expect("stopped server should reset auth");
    let database = connect_database(runtime.database_path.clone())
        .await
        .expect("database should reconnect");
    let reset_state = AuthService::new(database.pool.clone())
        .state()
        .await
        .expect("auth state should load");
    assert!(reset_state.setup_required);
    assert_eq!(reset_state.auth_version, 2);
    assert_eq!(
        load_json_rpc_token(&database.pool)
            .await
            .expect("token should load"),
        "preserved-rpc-token"
    );
    assert_eq!(
        list_download_tasks(&database.pool)
            .await
            .expect("tasks should load")
            .len(),
        1
    );
    assert_eq!(
        std::fs::read(&runtime.accessible_paths_path).expect("paths should read"),
        accessible_contents
    );
    assert_eq!(
        std::fs::read(&aria2_session).expect("session should read"),
        b"session-record"
    );
    database.pool.close().await;
    let _ = std::fs::remove_dir_all(&runtime.app_data_dir);
}

#[tokio::test]
async fn run_cli_rejects_unknown_commands() {
    let error = run_cli(&["unknown".to_string()])
        .await
        .expect_err("unknown command should fail");
    assert_eq!(
        error,
        "用法：motrix-fnos-server [reset-web-auth|database-check|database-backup <output>|database-cleanup-history <before_timestamp_ms> [--apply]]"
    );
}

#[test]
fn migrate_legacy_owned_task_dirs_uses_authorized_parent_without_trusting_root() {
    let base_dir = std::env::temp_dir().join(format!("motrix-fnos-owned-dir-migrate-{}", now_ms()));
    let task_dir = base_dir.join("torrent-file-name");
    std::fs::create_dir_all(&task_dir).expect("legacy task dir should create");

    let mut task = sample_task();
    task.source_type = crate::tasks::DownloadTaskSourceType::Torrent;
    task.url = "torrent:source.torrent".to_string();
    task.file_name = "torrent-inner-root".to_string();
    task.save_dir = task_dir.display().to_string();
    task.file_path = Some(
        task_dir
            .join("torrent-inner-root/file.bin")
            .display()
            .to_string(),
    );
    task.owned_task_dir = None;

    let mut pending_magnet = task.clone();
    pending_magnet.id = 8;
    pending_magnet.source_type = crate::tasks::DownloadTaskSourceType::Magnet;
    pending_magnet.url = "magnet:?xt=urn:btih:test".to_string();
    pending_magnet.save_dir = base_dir.display().to_string();
    pending_magnet.file_path = None;
    pending_magnet.metadata_torrent_path = None;
    pending_magnet.confirmation_required = false;

    let mut tasks = vec![task, pending_magnet];
    migrate_legacy_owned_task_dirs(&mut tasks, &[base_dir.display().to_string()]);

    assert_eq!(
        tasks[0].owned_task_dir.as_deref(),
        Some(
            task_dir
                .canonicalize()
                .expect("task dir should canonicalize")
                .to_str()
                .expect("task dir should be utf-8"),
        )
    );
    assert!(tasks[1].owned_task_dir.is_none());

    let _ = std::fs::remove_dir_all(base_dir);
}

fn sample_task() -> DownloadTask {
    DownloadTask {
        id: 7,
        url: "https://example.com/archive.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-7".to_string()),
        status: DownloadTaskStatus::Paused,
        total_length: 1024,
        completed_length: 512,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 100,
        updated_at: 101,
    }
}

async fn reserve_local_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("temporary port should bind");
    listener.local_addr().expect("local address should read")
}

async fn assert_listener_closed(addr: SocketAddr) {
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(250),
        tokio::net::TcpStream::connect(addr),
    )
    .await;
    assert!(
        matches!(result, Ok(Err(_))),
        "listener should reject new connections: {addr}"
    );
}

fn listener_runtime(http_addr: SocketAddr, jsonrpc_addr: SocketAddr) -> ServerRuntimeConfig {
    let app_data_dir = std::env::temp_dir().join(format!("motrix-fnos-listeners-{}", now_ms()));
    ServerRuntimeConfig {
        database_path: app_data_dir.join(DATABASE_FILE_NAME),
        accessible_paths_path: app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME),
        app_data_dir,
        http_addr,
        jsonrpc_addr,
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be valid")
        .as_millis()
}
