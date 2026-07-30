use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::config::aria2::Aria2BinarySource;
use crate::runtime::ManagedAria2Process;
use crate::tasks::{DownloadTask, DownloadTaskStatus};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[test]
fn idle_stop_debouncer_requires_full_window_and_resets_on_activity() {
    let start = Instant::now();
    let mut debouncer = IdleStopDebouncer::default();
    let idle = crate::runtime::Aria2ActivitySnapshot::default();
    let busy = crate::runtime::Aria2ActivitySnapshot {
        has_active_task: true,
        ..crate::runtime::Aria2ActivitySnapshot::default()
    };

    assert!(!debouncer.observe(idle, start, Duration::from_secs(30)));
    assert!(!debouncer.observe(
        idle,
        start + Duration::from_secs(29),
        Duration::from_secs(30)
    ));
    assert!(debouncer.observe(
        idle,
        start + Duration::from_secs(30),
        Duration::from_secs(30)
    ));
    assert!(!debouncer.observe(busy, start + Duration::from_secs(31), Duration::ZERO));
    assert!(!debouncer.observe(
        idle,
        start + Duration::from_secs(31),
        Duration::from_secs(30)
    ));
}

#[test]
fn idle_stop_debouncer_suppresses_duplicate_failure_logs() {
    let start = Instant::now();
    let mut debouncer = IdleStopDebouncer::default();

    assert!(debouncer.should_log_failure("session 失败", start));
    assert!(!debouncer.should_log_failure("session 失败", start + Duration::from_secs(5)));
    assert!(debouncer.should_log_failure("session 失败", start + Duration::from_secs(10)));
    assert!(debouncer.should_log_failure("其他失败", start + Duration::from_secs(11)));
}

#[tokio::test]
async fn monitor_tasks_once_broadcasts_snapshot_when_visible_tasks_change() {
    let mock = MockAria2Server::spawn("complete").await;
    let state = ready_state(&mock).await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    let mut receiver = state.runtime_events.subscribe();

    monitor_tasks_once(&state)
        .await
        .expect("monitor should complete");

    let event = receiver.recv().await.expect("event should be broadcast");
    match event {
        RuntimeEvent::TasksSnapshot(payload) => {
            assert_eq!(payload.revision, 1);
            assert_eq!(payload.tasks.len(), 1);
            assert_eq!(payload.tasks[0].status, DownloadTaskStatus::Complete);
        }
        RuntimeEvent::RuntimeExiting(_) => panic!("unexpected runtime exiting event"),
    }

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn task_snapshot_revisions_strictly_increase() {
    let mock = MockAria2Server::spawn("complete").await;
    let state = ready_state(&mock).await;
    let mut receiver = state.runtime_events.subscribe();

    broadcast_tasks_snapshot(&state).expect("first snapshot should broadcast");
    broadcast_tasks_snapshot(&state).expect("second snapshot should broadcast");

    let first_revision = match receiver
        .recv()
        .await
        .expect("first event should be broadcast")
    {
        RuntimeEvent::TasksSnapshot(payload) => payload.revision,
        RuntimeEvent::RuntimeExiting(_) => panic!("unexpected runtime exiting event"),
    };
    let second_revision = match receiver
        .recv()
        .await
        .expect("second event should be broadcast")
    {
        RuntimeEvent::TasksSnapshot(payload) => payload.revision,
        RuntimeEvent::RuntimeExiting(_) => panic!("unexpected runtime exiting event"),
    };
    assert_eq!(first_revision, 1);
    assert_eq!(second_revision, 2);

    cleanup_state(&state);
    mock.abort();
}

#[test]
fn monitor_matrix_only_accepts_pending_or_active_tasks_with_a_gid() {
    for status in [DownloadTaskStatus::Pending, DownloadTaskStatus::Active] {
        let task = sample_task(status);
        assert!(should_monitor_task(&task));

        let mut task_without_gid = task.clone();
        task_without_gid.gid = None;
        assert!(!should_monitor_task(&task_without_gid));
    }

    for status in [
        DownloadTaskStatus::Paused,
        DownloadTaskStatus::Complete,
        DownloadTaskStatus::Error,
        DownloadTaskStatus::Removed,
    ] {
        let mut task = sample_task(status);
        task.gid = Some("old-gid".to_string());
        assert!(!should_monitor_task(&task));
    }
}

#[tokio::test]
async fn static_tasks_do_not_start_aria2_without_session_or_runtime() {
    let app_data_dir = temp_dir("monitor-static-tasks");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");

    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| {
            for status in [
                DownloadTaskStatus::Pending,
                DownloadTaskStatus::Active,
                DownloadTaskStatus::Paused,
                DownloadTaskStatus::Complete,
                DownloadTaskStatus::Error,
            ] {
                let mut task = sample_task(status);
                task.id = tasks.len() as u64 + 1;
                task.gid = None;
                tasks.push(task);
            }
        })
        .expect("tasks should lock");

    monitor_tasks_once(&state)
        .await
        .expect("static task monitoring should be a no-op");

    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());

    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(app_data_dir);
}

#[tokio::test]
async fn auto_stop_allows_missing_metadata_record_and_clears_runtime() {
    let mock = MockAria2Server::spawn("complete").await;
    let state = ready_state(&mock).await;
    let mut task = sample_task(DownloadTaskStatus::Complete);
    task.source_type = crate::tasks::DownloadTaskSourceType::Torrent;
    task.gid = None;
    task.metadata_torrent_path = None;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("tasks should lock");
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");

    auto_stop_aria2(&state)
        .await
        .expect("auto stop should succeed");

    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(!state.core.aria2_runtime_path.exists());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());

    state.core.database.pool.close().await;
    mock.abort();
}

fn sample_task(status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: temp_dir("monitor-downloads").display().to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status,
        total_length: 1024,
        completed_length: 0,
        download_speed: 128,
        error_code: None,
        error_message: None,
        file_path: None,
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 2,
    }
}

async fn ready_state(mock: &MockAria2Server) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("monitor-state");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    let child = spawn_sleep_child();
    let pid = child.id();
    let config = crate::aria2::runtime_config(
        &state.base_aria2_config,
        mock.addr.port(),
        "secret".to_string(),
    );
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            pid,
            &config,
            Aria2BinarySource::ExternalPath,
            vec!["--mock".to_string()],
        ))
        .expect("runtime should persist");
    *state
        .aria2_process
        .lock()
        .expect("process lock should succeed") = Some(ManagedAria2Process::new(
        child,
        Aria2BinarySource::ExternalPath,
    ));
    state
}

fn cleanup_state(state: &Arc<HttpAppState>) {
    state.clear_aria2_runtime();
    if let Some(mut child) = state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .take()
    {
        let pid = child.id();
        let _ = child.kill();
        let _ = crate::aria2::terminate_process(pid);
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}

#[cfg(unix)]
fn spawn_sleep_child() -> std::process::Child {
    std::process::Command::new("sh")
        .args(["-c", "sleep 30"])
        .spawn()
        .expect("sleep child should spawn")
}

#[cfg(windows)]
fn spawn_sleep_child() -> std::process::Child {
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
        .spawn()
        .expect("sleep child should spawn")
}

struct MockAria2Server {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

impl MockAria2Server {
    async fn spawn(task_status: &'static str) -> Self {
        let state = Arc::new(MockAria2State::new(task_status));
        let app = Router::new()
            .route("/jsonrpc", post(mock_aria2_rpc))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock server should serve");
        });
        Self { addr, handle }
    }

    fn abort(self) {
        self.handle.abort();
    }
}

struct MockAria2State {
    task_status: &'static str,
    tasks: Mutex<HashMap<String, MockTask>>,
    next_gid: AtomicU64,
}

impl MockAria2State {
    fn new(task_status: &'static str) -> Self {
        let mut tasks = HashMap::new();
        tasks.insert(
            "gid-1".to_string(),
            MockTask {
                dir: temp_dir("monitor-downloads").display().to_string(),
                file_name: "archive.zip".to_string(),
            },
        );
        Self {
            task_status,
            tasks: Mutex::new(tasks),
            next_gid: AtomicU64::new(1),
        }
    }
}

#[derive(Clone)]
struct MockTask {
    dir: String,
    file_name: String,
}

async fn mock_aria2_rpc(
    State(state): State<Arc<MockAria2State>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = payload
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Json(match method {
        "aria2.getVersion" => json!({ "result": { "version": "1.37.0" } }),
        "aria2.tellStatus" => {
            let gid = gid_param(&params);
            let task = state
                .tasks
                .lock()
                .expect("tasks should lock")
                .get(&gid)
                .cloned()
                .expect("task should exist");
            json!({
                "result": {
                    "gid": gid,
                    "status": state.task_status,
                    "totalLength": "1024",
                    "completedLength": "1024",
                    "downloadSpeed": "0",
                    "dir": task.dir,
                    "files": [{
                        "path": format!("{}/{}", task.dir, task.file_name),
                        "uris": []
                    }]
                }
            })
        }
        "aria2.addUri" => {
            let gid = format!("gid-{}", state.next_gid.fetch_add(1, Ordering::SeqCst) + 1);
            json!({ "result": gid })
        }
        _ => json!({ "result": "ok" }),
    })
}

fn gid_param(params: &[Value]) -> String {
    let index = params
        .first()
        .and_then(Value::as_str)
        .map(|value| usize::from(value.starts_with("token:")))
        .unwrap_or(0);
    params
        .get(index)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
