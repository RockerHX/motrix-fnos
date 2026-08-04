use super::*;
use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::database::connect_database;
use crate::database::settings::get_download_proxy_config;
use crate::database::tasks::upsert_download_task;
use crate::runtime::{Aria2LifecycleCoordinator, Aria2LifecyclePhase};
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus, TaskProxyBinding};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn proxy_url_validation_accepts_supported_schemes_and_rejects_unsafe_inputs() {
    for value in [
        "http://proxy.example:7890",
        "https://proxy.example",
        "socks4://127.0.0.1:1080",
        "socks5://user:password@proxy.example:1080",
    ] {
        assert!(
            normalize_proxy_url(value).is_ok(),
            "{value} should be valid"
        );
    }

    for value in [
        "",
        "ftp://proxy.example:21",
        "http:///missing-host",
        "http://proxy.example:0",
        "http://proxy.example:65536",
        "http://proxy.example:7890?token=secret",
        "http://proxy.example:7890#secret",
        "http://proxy.example:7890\nnext",
        "http://proxy.example:7890\n",
    ] {
        assert!(normalize_proxy_url(value).is_err(), "{value:?} should fail");
    }
    assert!(normalize_proxy_url(&format!("http://{}", "a".repeat(MAX_PROXY_URL_BYTES))).is_err());
}

#[test]
fn proxy_mask_hides_all_credentials_without_changing_host() {
    let normalized =
        normalize_proxy_url("socks5://SensitiveUser:SensitivePassword@Proxy.Example:1080")
            .expect("proxy should normalize");
    let masked = mask_proxy_url(&normalized).expect("proxy should mask");

    assert!(
        masked.contains("***:***@proxy.example:1080"),
        "unexpected masked proxy: {masked}"
    );
    assert!(!masked.contains("SensitiveUser"));
    assert!(!masked.contains("SensitivePassword"));
}

#[tokio::test]
async fn proxy_profile_save_is_idempotent_and_updates_deferred_task_bindings() {
    let (database, path) = test_database("save-idempotent").await;
    let tasks = TaskMemoryState::new(vec![sample_task(1, TaskProxyBinding::profile(None))]);
    let lifecycle = Arc::new(Aria2LifecycleCoordinator::default());
    let rpc = Aria2RpcClient::new();
    let debug_logs = DebugLogStore::default();
    let update_lock = Mutex::new(());
    let raw_proxy = "http://ProfileUser:ProfilePassword@Proxy.Example:7890";

    let first = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: None,
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        raw_proxy,
    )
    .await
    .expect("proxy should save");
    assert_eq!(first.status.revision, 1);
    assert_eq!(first.deferred_task_ids, [1]);
    assert!(first.applied_task_ids.is_empty());
    let public_json = serde_json::to_string(&first).expect("response should serialize");
    assert!(!public_json.contains("ProfileUser"));
    assert!(!public_json.contains("ProfilePassword"));
    assert!(debug_logs
        .list()
        .iter()
        .all(|entry| !entry.message.contains("ProfilePassword")));
    let loaded_task = tasks.list().expect("task should be readable").remove(0);
    assert_eq!(
        loaded_task.proxy_binding.effective_proxy_url(),
        Some("http://ProfileUser:ProfilePassword@proxy.example:7890/")
    );

    let second = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: None,
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "  http://ProfileUser:ProfilePassword@proxy.example:7890/  ",
    )
    .await
    .expect("same normalized proxy should be a no-op");
    assert_eq!(second.status.revision, 1);
    assert!(second.deferred_task_ids.is_empty());

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn concurrent_proxy_saves_have_monotonic_revisions() {
    let (database, path) = test_database("concurrent-save").await;
    let tasks = TaskMemoryState::new(Vec::new());
    let lifecycle = Arc::new(Aria2LifecycleCoordinator::default());
    let rpc = Aria2RpcClient::new();
    let debug_logs = DebugLogStore::default();
    let update_lock = Mutex::new(());

    let first = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: None,
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "http://proxy-one.example:7890",
    );
    let second = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: None,
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "http://proxy-two.example:7890",
    );
    let (first, second) = tokio::join!(first, second);
    let mut revisions = [
        first.expect("first save should succeed").status.revision,
        second.expect("second save should succeed").status.revision,
    ];
    revisions.sort_unstable();
    assert_eq!(revisions, [1, 2]);
    assert_eq!(
        load_download_proxy_status(&database.pool)
            .await
            .expect("status should load")
            .revision,
        2
    );

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn proxy_clear_checks_profile_references_and_ignores_private_overrides() {
    let (database, path) = test_database("clear-reference").await;
    let profile_task = sample_task(1, TaskProxyBinding::profile(None));
    upsert_download_task(&database.pool, &profile_task)
        .await
        .expect("profile task should persist");
    let tasks = TaskMemoryState::new(vec![profile_task.clone()]);
    let lifecycle = Arc::new(Aria2LifecycleCoordinator::default());
    let rpc = Aria2RpcClient::new();
    let debug_logs = DebugLogStore::default();
    let update_lock = Mutex::new(());
    update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: None,
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "http://proxy.example:7890",
    )
    .await
    .expect("proxy should save");

    assert!(matches!(
        delete_download_proxy(&database.pool, &tasks, &update_lock, &debug_logs).await,
        Err(DownloadProxyServiceError::InUse)
    ));

    let mut override_task = sample_task(
        2,
        TaskProxyBinding::override_url("socks5://private.example:1080".to_string()),
    );
    override_task.status = DownloadTaskStatus::Removed;
    upsert_download_task(&database.pool, &override_task)
        .await
        .expect("override task should persist");
    let mut disabled_profile = profile_task;
    disabled_profile.use_proxy = false;
    upsert_download_task(&database.pool, &disabled_profile)
        .await
        .expect("disabled profile task should persist");
    tasks
        .with_tasks_mut(|tasks| tasks[0] = disabled_profile)
        .expect("memory task should update");

    delete_download_proxy(&database.pool, &tasks, &update_lock, &debug_logs)
        .await
        .expect("private override should not block clear");
    assert!(get_download_proxy_config(&database.pool)
        .await
        .expect("config should load")
        .is_none());
    assert_eq!(
        tasks.list().expect("task should be readable")[0]
            .proxy_binding
            .effective_proxy_url(),
        None
    );
    let override_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_proxy_overrides WHERE task_id = 2")
            .fetch_one(&database.pool)
            .await
            .expect("override count should load");
    assert_eq!(override_count, 1);

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn proxy_replacement_reports_partial_runtime_application_without_leaking_credentials() {
    let (database, path) = test_database("partial-apply").await;
    let tasks = TaskMemoryState::new(vec![
        sample_task(1, TaskProxyBinding::profile(None)),
        sample_task(2, TaskProxyBinding::profile(None)),
    ]);
    tasks
        .with_tasks_mut(|tasks| {
            tasks[0].gid = Some("gid-ok".to_string());
            tasks[1].gid = Some("gid-fail".to_string());
        })
        .expect("task gids should update");
    let lifecycle = Arc::new(Aria2LifecycleCoordinator::default());
    lifecycle
        .set_phase(Aria2LifecyclePhase::Ready)
        .expect("lifecycle should become ready");
    let rpc = Aria2RpcClient::with_lifecycle(Arc::clone(&lifecycle));
    let mock = PartialApplyAria2Server::spawn().await;
    let mut config = Aria2Config::from_env();
    config.rpc_host = mock.addr.ip().to_string();
    config.rpc_port = mock.addr.port();
    let debug_logs = DebugLogStore::default();
    let update_lock = Mutex::new(());

    let response = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: Some(config),
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "http://RuntimeUser:RuntimePassword@proxy.example:7890",
    )
    .await
    .expect("profile replacement should persist despite one runtime failure");

    assert_eq!(response.applied_task_ids, [1]);
    assert!(response.deferred_task_ids.is_empty());
    assert_eq!(response.failed.len(), 1);
    assert_eq!(response.failed[0].task_id, 2);
    assert_eq!(response.failed[0].code, "proxy_apply_failed");
    let response_json = serde_json::to_string(&response).expect("response should serialize");
    assert!(!response_json.contains("RuntimeUser"));
    assert!(!response_json.contains("RuntimePassword"));
    assert!(debug_logs.list().iter().all(|entry| {
        !entry.message.contains("RuntimeUser") && !entry.message.contains("RuntimePassword")
    }));
    assert_eq!(mock.request_count.load(Ordering::Relaxed), 2);

    let no_change = update_download_proxy(
        DownloadProxyServiceContext {
            pool: &database.pool,
            tasks: &tasks,
            aria2_lifecycle: &lifecycle,
            aria2_rpc: &rpc,
            aria2_config: Some({
                let mut config = Aria2Config::from_env();
                config.rpc_host = mock.addr.ip().to_string();
                config.rpc_port = mock.addr.port();
                config
            }),
            debug_logs: &debug_logs,
            update_lock: &update_lock,
        },
        "http://RuntimeUser:RuntimePassword@PROXY.EXAMPLE:7890/",
    )
    .await
    .expect("same normalized profile should be a no-op");
    assert_eq!(no_change.status.revision, 1);
    assert!(no_change.applied_task_ids.is_empty());
    assert!(no_change.deferred_task_ids.is_empty());
    assert!(no_change.failed.is_empty());
    assert_eq!(mock.request_count.load(Ordering::Relaxed), 2);

    mock.abort();
    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

struct PartialApplyAria2Server {
    addr: std::net::SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    request_count: Arc<AtomicUsize>,
}

impl PartialApplyAria2Server {
    async fn spawn() -> Self {
        let request_count = Arc::new(AtomicUsize::new(0));
        let handler_request_count = Arc::clone(&request_count);
        let app = Router::new().route(
            "/jsonrpc",
            post(move |Json(payload): Json<Value>| {
                let request_count = Arc::clone(&handler_request_count);
                async move {
                    request_count.fetch_add(1, Ordering::Relaxed);
                    let gid = payload
                        .get("params")
                        .and_then(Value::as_array)
                        .and_then(|params| params.iter().find_map(Value::as_str))
                        .unwrap_or_default();
                    if gid == "gid-fail" {
                        Json(json!({ "error": { "message": "deliberate changeOption failure" } }))
                    } else {
                        Json(json!({ "result": gid }))
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock listener should bind");
        let addr = listener.local_addr().expect("mock address should exist");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock aria2 server should run");
        });
        Self {
            addr,
            handle,
            request_count,
        }
    }

    fn abort(self) {
        self.handle.abort();
    }
}

async fn test_database(label: &str) -> (crate::database::AppDatabase, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-proxy-settings-{label}-{}-{}.sqlite",
        std::process::id(),
        current_timestamp_ms()
    ));
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    (database, path)
}

fn sample_task(id: u64, proxy_binding: TaskProxyBinding) -> DownloadTask {
    DownloadTask {
        id,
        url: format!("https://example.com/{id}.zip"),
        source_type: DownloadTaskSourceType::Url,
        file_name: format!("{id}.zip"),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some(format!("gid-{id}")),
        status: DownloadTaskStatus::Active,
        total_length: 100,
        completed_length: 10,
        download_speed: 1,
        error_code: None,
        error_message: None,
        file_path: Some(format!("/downloads/{id}.zip")),
        use_proxy: true,
        proxy_binding,
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}
