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
    let task = proxy_task(Some("socks5://127.0.0.1:1080"));

    let gid = readd_download_task(
        &Aria2RpcClient::new(),
        &test_config(mock.addr.port()),
        &task,
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

    let restored = sync_session_tasks_from_aria2(&tasks, &client, &config, None)
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
