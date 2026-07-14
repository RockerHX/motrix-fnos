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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

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
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(!state.core.aria2_runtime_path.exists());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());

    mock.abort();
}

fn sample_task(status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        file_name: "archive.zip".to_string(),
        save_dir: temp_dir("shutdown-downloads").display().to_string(),
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status,
        total_length: 1024,
        completed_length: 512,
        download_speed: 128,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        metadata_torrent_path: None,
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
        aria2_path: None,
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

    fn abort(self) {
        self.handle.abort();
    }
}

struct MockAria2State {
    tasks: Mutex<HashMap<String, MockTask>>,
    pause_calls: AtomicU64,
    save_session_calls: AtomicU64,
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
            pause_calls: AtomicU64::new(0),
            save_session_calls: AtomicU64::new(0),
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
            json!({ "result": "OK" })
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
