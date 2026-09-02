use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::config::aria2::Aria2BinarySource;
use crate::database::tasks::list_download_tasks;
use crate::runtime::ManagedAria2Process;
use crate::tasks::{DownloadTask, DownloadTaskStatus};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[tokio::test]
async fn shutdown_cleanup_pauses_tasks_persists_state_saves_session_and_stops_aria2() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Active)))
        .expect("tasks should lock");

    state.request_shutdown("收到停止信号");
    run_shutdown_cleanup(&state).await;

    let tasks = list_tasks(&state.core.download_tasks).expect("tasks should list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, DownloadTaskStatus::Paused);
    assert_eq!(tasks[0].download_speed, 0);

    let stored_tasks = list_download_tasks(&state.core.database.pool)
        .await
        .expect("stored tasks should load");
    assert_eq!(stored_tasks.len(), 1);
    assert_eq!(stored_tasks[0].status, DownloadTaskStatus::Paused);

    assert_eq!(mock.pause_calls(), 1);
    assert_eq!(mock.save_session_calls(), 1);
    assert_eq!(
        mock.calls(),
        vec![
            "aria2.tellStatus".to_string(),
            "aria2.pause".to_string(),
            "aria2.saveSession".to_string(),
        ]
    );
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(!state.core.aria2_runtime_path.exists());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());

    mock.abort();
}

#[tokio::test]
async fn shutdown_cleanup_preserves_runtime_when_session_and_stop_fail() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    mock.fail_session();

    std::thread::scope(|scope| {
        let process = &state.aria2_process;
        let handle = scope.spawn(move || {
            let _guard = process.lock().expect("process lock should succeed");
            panic!("force stop failure after poisoning process lock");
        });
        assert!(handle.join().is_err());
    });

    state.request_shutdown("测试 session 与停止失败");
    run_shutdown_cleanup(&state).await;

    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state.core.aria2_runtime_path.is_file());
    assert_eq!(mock.save_session_calls(), 1);

    state.aria2_process.clear_poison();
    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

#[tokio::test]
async fn shutdown_cleanup_stops_at_shared_deadline_when_aria2_rpc_hangs() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    mock.set_response_delay(Duration::from_secs(1));
    state.request_shutdown("测试退出总预算");

    let started_at = Instant::now();
    let completed = run_shutdown_cleanup_until(
        &state,
        tokio::time::Instant::now() + Duration::from_millis(80),
    )
    .await;

    assert!(!completed);
    assert!(started_at.elapsed() < Duration::from_millis(400));
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state.core.debug_logs.list().iter().any(|entry| {
        entry.module == "runtime.exit" && entry.message.contains("退出总预算耗尽")
    }));

    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

#[tokio::test]
async fn manual_stop_saves_session_once_before_stopping_idle_aria2() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");

    crate::runtime::stop_aria2(&state)
        .await
        .expect("manual stop should succeed");

    assert_eq!(mock.save_session_calls(), 1);
    assert_eq!(mock.calls(), vec!["aria2.saveSession".to_string()]);
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
    mock.abort();
}

#[tokio::test]
async fn manual_stop_preserves_runtime_when_session_save_fails() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    mock.fail_session();

    let error = crate::runtime::stop_aria2(&state)
        .await
        .expect_err("session failure should reject manual stop");

    assert!(error
        .to_string()
        .contains("手动停止前保存 Aria2 session 失败"));
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some());
    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

#[tokio::test]
async fn manual_stop_rejects_bt_seeding_activity() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| {
            let mut task = sample_task(DownloadTaskStatus::Complete);
            task.source_type = crate::tasks::DownloadTaskSourceType::Torrent;
            task.gid = Some("gid-bt".to_string());
            task.metadata_torrent_path = Some("/app-data/task.torrent".to_string());
            tasks.push(task);
        })
        .expect("tasks should be writable");
    mock.enable_bt_upload();

    let error = crate::runtime::stop_aria2(&state)
        .await
        .expect_err("BT seeding should keep Aria2 running");

    assert!(error.to_string().contains("活动或在途操作"));
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some());
    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

#[tokio::test]
async fn shutdown_wins_over_auto_stop_and_keeps_pause_semantics() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");

    state.request_shutdown("退出与自动停止竞态测试");
    let auto_state = state.clone();
    let auto_stop = tokio::spawn(async move { crate::runtime::auto_stop_aria2(&auto_state).await });
    run_shutdown_cleanup(&state).await;
    let auto_stop_result = auto_stop.await.expect("auto stop should not panic");

    let tasks = list_tasks(&state.core.download_tasks).expect("tasks should list");
    assert_eq!(tasks[0].status, DownloadTaskStatus::Paused);
    assert_eq!(mock.pause_calls(), 1);
    assert_eq!(mock.save_session_calls(), 1);
    assert_eq!(
        mock.calls(),
        vec![
            "aria2.tellStatus".to_string(),
            "aria2.pause".to_string(),
            "aria2.saveSession".to_string(),
        ]
    );
    assert!(auto_stop_result
        .expect_err("auto stop should yield to application exit")
        .contains("服务正在退出"));
    assert!(state.aria2_runtime_snapshot().is_none());
    mock.abort();
}

#[tokio::test]
async fn auto_stop_preserves_runtime_when_session_save_fails() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    mock.fail_session();

    let error = crate::runtime::auto_stop_aria2(&state)
        .await
        .expect_err("session failure should reject auto stop");

    assert!(error.contains("保存 Aria2 session 失败"));
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some());
    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

#[tokio::test]
async fn auto_stop_persists_task_state_before_saving_session() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Complete)))
        .expect("tasks should be writable");

    crate::runtime::auto_stop_aria2(&state)
        .await
        .expect("auto stop should persist task state and succeed");

    let stored_tasks = list_download_tasks(&state.core.database.pool)
        .await
        .expect("stored tasks should load");
    assert_eq!(stored_tasks.len(), 1);
    assert_eq!(stored_tasks[0].status, DownloadTaskStatus::Complete);
    assert_eq!(mock.calls(), vec!["aria2.saveSession".to_string()]);

    state.core.database.pool.close().await;
    mock.abort();
}

#[tokio::test]
async fn auto_stop_preserves_runtime_when_task_state_persist_fails() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(DownloadTaskStatus::Complete)))
        .expect("tasks should be writable");
    state.core.database.pool.close().await;

    let error = crate::runtime::auto_stop_aria2(&state)
        .await
        .expect_err("task state persistence failure should reject auto stop");

    assert!(error.contains("自动停止前持久化任务状态失败"));
    assert_eq!(mock.save_session_calls(), 0);
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some());
    crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test cleanup should stop the child process");
    state.clear_aria2_runtime();
    mock.abort();
}

fn sample_task(status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: temp_dir("shutdown-downloads").display().to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status,
        total_length: 1024,
        completed_length: 512,
        download_speed: 128,
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
        created_at: 1,
        updated_at: 2,
    }
}

async fn ready_state(mock: &MockAria2Server) -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("shutdown-state");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
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

fn temp_dir(label: &str) -> PathBuf {
    static NEXT_TEMP_DIR_ID: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}-{}",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos(),
        NEXT_TEMP_DIR_ID.fetch_add(1, Ordering::SeqCst),
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
    state: Arc<MockAria2State>,
}

impl MockAria2Server {
    async fn spawn() -> Self {
        let state = Arc::new(MockAria2State::default());
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
                .expect("mock server should serve");
        });
        Self {
            addr,
            handle,
            state,
        }
    }

    fn pause_calls(&self) -> u64 {
        self.state.pause_calls.load(Ordering::SeqCst)
    }

    fn save_session_calls(&self) -> u64 {
        self.state.save_session_calls.load(Ordering::SeqCst)
    }

    fn calls(&self) -> Vec<String> {
        self.state.calls.lock().expect("calls should lock").clone()
    }

    fn fail_session(&self) {
        self.state.fail_save_session.store(true, Ordering::SeqCst);
    }

    fn set_response_delay(&self, delay: Duration) {
        self.state
            .response_delay_ms
            .store(delay.as_millis() as u64, Ordering::SeqCst);
    }

    fn enable_bt_upload(&self) {
        self.state.bt_upload_active.store(true, Ordering::SeqCst);
    }

    fn abort(self) {
        self.handle.abort();
    }
}

struct MockAria2State {
    tasks: Mutex<HashMap<String, MockTask>>,
    calls: Mutex<Vec<String>>,
    pause_calls: AtomicU64,
    save_session_calls: AtomicU64,
    fail_save_session: AtomicBool,
    bt_upload_active: AtomicBool,
    response_delay_ms: AtomicU64,
}

impl Default for MockAria2State {
    fn default() -> Self {
        let mut tasks = HashMap::new();
        tasks.insert(
            "gid-1".to_string(),
            MockTask {
                status: "active".to_string(),
                dir: temp_dir("shutdown-downloads").display().to_string(),
                file_name: "archive.zip".to_string(),
            },
        );
        Self {
            tasks: Mutex::new(tasks),
            calls: Mutex::new(Vec::new()),
            pause_calls: AtomicU64::new(0),
            save_session_calls: AtomicU64::new(0),
            fail_save_session: AtomicBool::new(false),
            bt_upload_active: AtomicBool::new(false),
            response_delay_ms: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
struct MockTask {
    status: String,
    dir: String,
    file_name: String,
}

async fn mock_aria2_rpc(
    State(state): State<Arc<MockAria2State>>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let response_delay_ms = state.response_delay_ms.load(Ordering::SeqCst);
    if response_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(response_delay_ms)).await;
    }
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    state
        .calls
        .lock()
        .expect("calls should lock")
        .push(method.to_string());
    let params = payload
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Json(match method {
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
                    "status": task.status,
                    "totalLength": "1024",
                    "completedLength": "512",
                    "downloadSpeed": if task.status == "paused" { "0" } else { "128" },
                    "dir": task.dir,
                    "files": [{
                        "path": format!("{}/{}", task.dir, task.file_name),
                        "uris": []
                    }]
                }
            })
        }
        "aria2.pause" => {
            let gid = gid_param(&params);
            state.pause_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(task) = state.tasks.lock().expect("tasks should lock").get_mut(&gid) {
                task.status = "paused".to_string();
            }
            json!({ "result": gid })
        }
        "aria2.saveSession" => {
            state.save_session_calls.fetch_add(1, Ordering::SeqCst);
            if state.fail_save_session.load(Ordering::SeqCst) {
                json!({
                    "error": {
                        "code": 1,
                        "message": "forced session save failure"
                    }
                })
            } else {
                json!({ "result": "OK" })
            }
        }
        "aria2.tellActive" => {
            if state.bt_upload_active.load(Ordering::SeqCst) {
                json!({
                    "result": [{
                        "gid": "gid-bt",
                        "uploadSpeed": "32",
                        "seeder": true,
                        "bittorrent": {}
                    }]
                })
            } else {
                json!({ "result": [] })
            }
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
