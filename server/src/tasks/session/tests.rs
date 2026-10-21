use super::*;
use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn proxy_reconcile_changes_only_mismatched_option() {
    let mock = MockAria2Server::spawn("").await;
    let task = proxy_task(Some("http://127.0.0.1:7890/"));

    let applied = reconcile_task_proxy_option(
        &Aria2RpcClient::new(),
        &test_config(mock.addr.port()),
        &task,
        None,
        None,
    )
    .await
    .expect("mismatched proxy should reconcile");

    assert!(applied);
    let requests = mock.requests();
    assert_eq!(requests[0]["method"], "aria2.getOption");
    assert_eq!(requests[1]["method"], "aria2.changeOption");
    assert_eq!(
        requests[1]["params"]
            .as_array()
            .and_then(|params| params.last())
            .and_then(Value::as_object)
            .and_then(|options| options.get("all-proxy")),
        Some(&json!("http://127.0.0.1:7890/"))
    );
    mock.abort();
}

#[tokio::test]
async fn proxy_reconcile_skips_matching_option() {
    let mock = MockAria2Server::spawn("http://127.0.0.1:7890/").await;
    let task = proxy_task(Some("http://127.0.0.1:7890/"));

    let applied = reconcile_task_proxy_option(
        &Aria2RpcClient::new(),
        &test_config(mock.addr.port()),
        &task,
        None,
        None,
    )
    .await
    .expect("matching proxy should remain unchanged");

    assert!(!applied);
    assert_eq!(mock.methods(), vec!["aria2.getOption"]);
    mock.abort();
}

#[tokio::test]
async fn proxy_reconcile_rejects_enabled_task_without_binding() {
    let mock = MockAria2Server::spawn("").await;
    let task = proxy_task(None);

    let error = reconcile_task_proxy_option(
        &Aria2RpcClient::new(),
        &test_config(mock.addr.port()),
        &task,
        None,
        None,
    )
    .await
    .expect_err("missing proxy binding must fail closed");

    assert!(error.to_string().contains("没有可用的代理配置"));
    assert_eq!(mock.methods(), vec!["aria2.getOption"]);
    mock.abort();
}

#[tokio::test]
async fn stale_gid_readd_includes_persisted_proxy_binding() {
    let mock = MockAria2Server::spawn("").await;
    let mut task = proxy_task(Some("socks5://127.0.0.1:1080"));
    let save_dir = std::env::temp_dir().join(format!(
        "motrix-session-readd-{}-{}",
        std::process::id(),
        current_timestamp_ms()
    ));
    std::fs::create_dir_all(&save_dir).expect("save directory should exist");
    task.save_dir = save_dir.display().to_string();
    task.file_path = Some(save_dir.join("archive.zip").display().to_string());
    let accessible_paths_path = std::env::temp_dir().join(format!(
        "motrix-accessible-paths-{}-{}.json",
        std::process::id(),
        current_timestamp_ms()
    ));
    std::fs::write(
        &accessible_paths_path,
        serde_json::json!({ "paths": [save_dir.display().to_string()] }).to_string(),
    )
    .expect("authorization snapshot should write");

    let gid = readd_download_task(
        &Aria2RpcClient::new(),
        &test_config(mock.addr.port()),
        &task,
        &accessible_paths_path,
        None,
    )
    .await
    .expect("stale task should readd");

    assert_eq!(gid, "gid-new");
    let requests = mock.requests();
    let add_request = requests
        .iter()
        .find(|request| request["method"] == "aria2.addUri")
        .expect("addUri should be requested");
    let options = add_request["params"]
        .as_array()
        .and_then(|params| params.last())
        .and_then(Value::as_object)
        .expect("addUri options should exist");
    assert_eq!(options["all-proxy"], "socks5://127.0.0.1:1080");
    let _ = std::fs::remove_file(accessible_paths_path);
    let _ = std::fs::remove_dir_all(save_dir);
    mock.abort();
}

#[tokio::test]
async fn session_restore_updates_gid_before_proxy_reconcile() {
    let mock = MockAria2Server::spawn("").await;
    let mut task = proxy_task(Some("http://127.0.0.1:7890/"));
    task.status = DownloadTaskStatus::Active;
    let tasks = TaskMemoryState::new(vec![task]);
    let client = Aria2RpcClient::new();
    let config = test_config(mock.addr.port());

    let restored =
        sync_session_tasks_from_aria2(&tasks, &client, &config, Path::new("/private"), None)
            .await
            .expect("session tasks should synchronize");
    reconcile_session_task_proxies(&tasks, &client, &config, None)
        .await
        .expect("restored task proxy should reconcile");

    assert_eq!(restored[0].gid.as_deref(), Some("gid-session"));
    assert_eq!(restored[0].status, DownloadTaskStatus::Paused);
    assert_eq!(
        mock.methods(),
        vec![
            "aria2.tellActive",
            "aria2.tellWaiting",
            "aria2.tellStopped",
            "aria2.getOption",
            "aria2.changeOption",
        ]
    );
    let requests = mock.requests();
    let get_option = requests
        .iter()
        .find(|request| request["method"] == "aria2.getOption")
        .expect("getOption should be requested");
    assert!(get_option["params"]
        .as_array()
        .is_some_and(|params| params.iter().any(|value| value == "gid-session")));
    mock.abort();
}

fn proxy_task(proxy_url: Option<&str>) -> DownloadTask {
    DownloadTask {
        id: 1,
        engine_generation: crate::config::aria2::EngineGeneration::LegacyV1,
        removed_from_status: None,
        url: "https://example.com/archive.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-old".to_string()),
        status: DownloadTaskStatus::Paused,
        total_length: 1024,
        completed_length: 512,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        use_proxy: true,
        proxy_binding: crate::tasks::TaskProxyBinding::profile(proxy_url.map(str::to_string)),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}

fn native_config(port: u16) -> Aria2Config {
    let mut config = test_config(port);
    config.state_dir = Some("/private/native-state".to_string());
    config
}

#[tokio::test]
async fn native_restore_keeps_original_gid_and_recycle_bin_fact() {
    let mock = MockAria2Server::spawn("").await;
    let mut task = proxy_task(None);
    task.engine_generation = crate::config::aria2::EngineGeneration::NativeV2;
    task.gid = Some("gid-session".to_string());
    task.status = DownloadTaskStatus::Removed;
    task.completed_length = 900;
    task.use_proxy = false;
    let tasks = TaskMemoryState::new(vec![task]);
    let restored = sync_session_tasks_from_aria2(
        &tasks,
        &Aria2RpcClient::new(),
        &native_config(mock.addr.port()),
        Path::new("/private"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(restored[0].status, DownloadTaskStatus::Removed);
    assert_eq!(restored[0].gid.as_deref(), Some("gid-session"));
    assert_eq!(restored[0].completed_length, 512);
    assert_eq!(restored[0].files.len(), 1);
    assert!(mock.methods().iter().all(|method| !method.contains("add")));
    mock.abort();
}

#[tokio::test]
async fn native_restore_rejects_unknown_gid_and_keeps_snapshot_unchanged() {
    let mock = MockAria2Server::spawn("").await;
    let mut task = proxy_task(None);
    task.engine_generation = crate::config::aria2::EngineGeneration::NativeV2;
    task.status = DownloadTaskStatus::Active;
    let tasks = TaskMemoryState::new(vec![task.clone()]);
    let error = sync_session_tasks_from_aria2(
        &tasks,
        &Aria2RpcClient::new(),
        &native_config(mock.addr.port()),
        Path::new("/private"),
        None,
    )
    .await
    .unwrap_err();
    assert!(error.contains("task_resume_state_invalid"));
    assert_eq!(tasks.list().unwrap(), vec![task]);
    mock.abort();
}

#[tokio::test]
async fn native_restore_rejects_changed_file_selection_before_updating_any_task() {
    let mock = MockAria2Server::spawn("").await;
    let mut task = proxy_task(None);
    task.engine_generation = crate::config::aria2::EngineGeneration::NativeV2;
    task.gid = Some("gid-session".to_string());
    task.files = vec![crate::tasks::DownloadTaskFile {
        index: 1,
        path: "/downloads/archive.zip".to_string(),
        name: "archive.zip".to_string(),
        length: 1024,
        completed_length: 512,
        selected: false,
    }];
    let tasks = TaskMemoryState::new(vec![task.clone()]);
    assert!(sync_session_tasks_from_aria2(
        &tasks,
        &Aria2RpcClient::new(),
        &native_config(mock.addr.port()),
        Path::new("/private"),
        None
    )
    .await
    .is_err());
    assert_eq!(tasks.list().unwrap(), vec![task]);
    mock.abort();
}

#[tokio::test]
async fn native_restore_initializes_only_unknown_single_http_file_length() {
    let mock = MockAria2Server::spawn("").await;
    for (source_type, length, total_length, accepted) in [
        (DownloadTaskSourceType::Url, 0, 0, true),
        (DownloadTaskSourceType::Url, 0, 1024, true),
        (DownloadTaskSourceType::Url, 0, 2048, false),
        (DownloadTaskSourceType::Url, 512, 1024, false),
        (DownloadTaskSourceType::Torrent, 0, 1024, false),
        (DownloadTaskSourceType::Magnet, 0, 1024, false),
    ] {
        let mut task = proxy_task(None);
        task.engine_generation = crate::config::aria2::EngineGeneration::NativeV2;
        task.gid = Some("gid-session".to_string());
        task.source_type = source_type;
        task.total_length = total_length;
        task.files = vec![crate::tasks::DownloadTaskFile {
            index: 1,
            path: "/downloads/archive.zip".to_string(),
            name: "archive.zip".to_string(),
            length,
            completed_length: 512,
            selected: true,
        }];
        let tasks = TaskMemoryState::new(vec![task.clone()]);
        let result = sync_session_tasks_from_aria2(
            &tasks,
            &Aria2RpcClient::new(),
            &native_config(mock.addr.port()),
            Path::new("/private"),
            None,
        )
        .await;
        assert_eq!(result.is_ok(), accepted, "{task:?}: {result:?}");
        if accepted {
            assert_eq!(result.unwrap()[0].files[0].length, 1024);
        } else {
            assert_eq!(tasks.list().unwrap(), vec![task]);
        }
    }
    mock.abort();
}

#[tokio::test]
async fn native_or_unknown_tasks_never_readd_a_missing_gid() {
    let mock = MockAria2Server::spawn("").await;
    for generation in [
        crate::config::aria2::EngineGeneration::NativeV2,
        crate::config::aria2::EngineGeneration::Unknown,
    ] {
        let mut task = proxy_task(None);
        task.engine_generation = generation;
        let error = readd_download_task(
            &Aria2RpcClient::new(),
            &native_config(mock.addr.port()),
            &task,
            Path::new("/missing-auth"),
            None,
        )
        .await
        .unwrap_err();
        assert!(error.contains("task_resume_state_invalid"));
    }
    assert!(mock.requests().is_empty());
    mock.abort();
}

fn test_config(port: u16) -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: crate::config::aria2::Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: port,
        rpc_secret: String::new(),
        session_path: None,
        state_dir: None,
        log_path: None,
    }
}

struct MockAria2Server {
    addr: SocketAddr,
    state: Arc<MockAria2State>,
    handle: tokio::task::JoinHandle<()>,
}

struct MockAria2State {
    current_proxy: String,
    requests: Mutex<Vec<Value>>,
}

impl MockAria2Server {
    async fn spawn(current_proxy: &str) -> Self {
        let state = Arc::new(MockAria2State {
            current_proxy: current_proxy.to_string(),
            requests: Mutex::new(Vec::new()),
        });
        let app = Router::new()
            .route("/jsonrpc", post(mock_aria2_rpc))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock Aria2 should serve");
        });
        Self {
            addr,
            state,
            handle,
        }
    }

    fn requests(&self) -> Vec<Value> {
        self.state
            .requests
            .lock()
            .expect("requests should lock")
            .clone()
    }

    fn methods(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter_map(|request| request["method"].as_str().map(str::to_string))
            .collect()
    }

    fn abort(self) {
        self.handle.abort();
    }
}

async fn mock_aria2_rpc(
    State(state): State<Arc<MockAria2State>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    state
        .requests
        .lock()
        .expect("requests should lock")
        .push(payload.clone());
    let method = payload["method"].as_str().unwrap_or_default();
    Json(match method {
        "aria2.tellActive" | "aria2.tellStopped" => json!({ "result": [] }),
        "aria2.tellWaiting" => json!({
            "result": [{
                "gid": "gid-session",
                "status": "paused",
                "totalLength": "1024",
                "completedLength": "512",
                "downloadSpeed": "0",
                "dir": "/downloads",
                "files": [{
                    "index": "1",
                    "path": "/downloads/archive.zip",
                    "length": "1024",
                    "completedLength": "512",
                    "selected": "true",
                    "uris": [{ "uri": "https://example.com/archive.zip" }]
                }]
            }]
        }),
        "aria2.getOption" => json!({
            "result": { "all-proxy": state.current_proxy }
        }),
        "aria2.changeOption" => json!({ "result": "gid-old" }),
        "aria2.removeDownloadResult" => json!({ "result": "gid-old" }),
        "aria2.addUri" => json!({ "result": "gid-new" }),
        other => json!({ "error": { "message": format!("unexpected method: {other}") } }),
    })
}
