use crate::app::{HttpAppState, RuntimeEvent, TasksSnapshotPayload};
use crate::database::tasks::persist_download_task_states;
use crate::runtime::ensure_aria2_ready;
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

const TASK_MONITOR_INTERVAL: Duration = Duration::from_secs(5);

pub fn spawn_task_monitor(state: Arc<HttpAppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(TASK_MONITOR_INTERVAL).await;
            if state.core.is_exiting.load(Ordering::SeqCst) {
                state
                    .core
                    .debug_logs
                    .info("runtime.monitor", "服务正在退出，停止后台任务状态同步");
                break;
            }

            if let Err(error) = monitor_tasks_once(&state).await {
                state.core.debug_logs.warn(
                    "runtime.monitor",
                    format!("后台任务状态同步失败：{}", error),
                );
            }
        }
    });
}

pub async fn monitor_tasks_once(state: &Arc<HttpAppState>) -> Result<(), String> {
    let previous_tasks = visible_tasks_snapshot(state)?;
    if !previous_tasks.iter().any(should_monitor_task) {
        return Ok(());
    }

    let config = ensure_aria2_ready(state).await?;
    let tasks = refresh_tasks_from_aria2(
        &state.core.download_tasks,
        &config,
        Some(&state.core.debug_logs),
    )
    .await?;
    persist_download_task_states(&state.core.database.pool, &tasks).await?;
    let next_tasks = visible_tasks(tasks);
    if next_tasks != previous_tasks {
        let _ = state
            .runtime_events
            .send(RuntimeEvent::TasksSnapshot(TasksSnapshotPayload {
                tasks: next_tasks,
            }))?;
    }
    Ok(())
}

pub fn visible_tasks_snapshot(state: &HttpAppState) -> Result<Vec<DownloadTask>, String> {
    crate::tasks::list_tasks(&state.core.download_tasks).map(visible_tasks)
}

pub fn broadcast_tasks_snapshot(state: &HttpAppState) -> Result<(), String> {
    let tasks = visible_tasks_snapshot(state)?;
    let _ = state
        .runtime_events
        .send(RuntimeEvent::TasksSnapshot(TasksSnapshotPayload { tasks }))?;
    Ok(())
}

fn should_monitor_task(task: &DownloadTask) -> bool {
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

fn visible_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
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
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;

    #[tokio::test]
    async fn monitor_tasks_once_broadcasts_snapshot_when_visible_tasks_change() {
        let mock = MockAria2Server::spawn("complete").await;
        let state = ready_state(&mock).await;
        {
            let mut tasks = state.core.download_tasks.lock().expect("tasks should lock");
            tasks.push(sample_task(DownloadTaskStatus::Active));
        }
        let mut receiver = state.runtime_events.subscribe();

        monitor_tasks_once(&state)
            .await
            .expect("monitor should complete");

        let event = receiver.recv().await.expect("event should be broadcast");
        match event {
            RuntimeEvent::TasksSnapshot(payload) => {
                assert_eq!(payload.tasks.len(), 1);
                assert_eq!(payload.tasks[0].status, DownloadTaskStatus::Complete);
            }
            RuntimeEvent::RuntimeExiting(_) => panic!("unexpected runtime exiting event"),
        }

        cleanup_state(&state);
        mock.abort();
    }

    fn sample_task(status: DownloadTaskStatus) -> DownloadTask {
        DownloadTask {
            id: 1,
            url: "https://example.com/archive.zip".to_string(),
            file_name: "archive.zip".to_string(),
            save_dir: temp_dir("monitor-downloads").display().to_string(),
            gid: Some("gid-1".to_string()),
            status,
            total_length: 1024,
            completed_length: 0,
            download_speed: 128,
            error_code: None,
            error_message: None,
            file_path: None,
            created_at: 1,
            updated_at: 2,
        }
    }

    async fn ready_state(mock: &MockAria2Server) -> Arc<HttpAppState> {
        let app_data_dir = temp_dir("monitor-state");
        let runtime = ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.sqlite"),
            app_data_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
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
}
