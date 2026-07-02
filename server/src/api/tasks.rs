use crate::api::error::ApiError;
use crate::api::extract::ApiJson;
use crate::app::HttpAppState;
use crate::runtime::{broadcast_tasks_snapshot, ensure_aria2_ready};
use crate::tasks::service::TaskService;
use crate::tasks::{CreateDownloadTaskRequest, DownloadTask};
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub fn routes() -> Router<Arc<HttpAppState>> {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/:id/pause", post(pause_task))
        .route("/tasks/:id/resume", post(resume_task))
        .route("/tasks/:id/redownload", post(redownload_task))
        .route("/tasks/:id", delete(delete_task))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DeleteTaskQuery {
    delete_files: Option<bool>,
}

async fn list_tasks(
    State(state): State<Arc<HttpAppState>>,
) -> Result<Json<Vec<DownloadTask>>, ApiError> {
    let service = task_service(&state);
    let config = if state.core.is_exiting.load(Ordering::SeqCst) {
        state.aria2_config()
    } else {
        ensure_aria2_ready(&state)
            .await
            .map_err(classify_aria2_ready_error)?
    };
    let tasks = service
        .list_download_tasks(&config)
        .await
        .map_err(classify_task_error)?;
    Ok(Json(tasks))
}

async fn create_task(
    State(state): State<Arc<HttpAppState>>,
    ApiJson(payload): ApiJson<CreateDownloadTaskRequest>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .create_download_task(&config, payload)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn pause_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .pause_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn resume_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .resume_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn redownload_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .redownload_download_task(&config, task_id)
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

async fn delete_task(
    State(state): State<Arc<HttpAppState>>,
    Path(task_id): Path<u64>,
    Query(query): Query<DeleteTaskQuery>,
) -> Result<Json<DownloadTask>, ApiError> {
    let service = task_service(&state);
    service.ensure_not_exiting().map_err(classify_task_error)?;
    let config = ensure_aria2_ready(&state)
        .await
        .map_err(classify_aria2_ready_error)?;
    let task = service
        .delete_download_task(&config, task_id, query.delete_files.unwrap_or(false))
        .await
        .map_err(classify_task_error)?;
    broadcast_tasks_snapshot(&state)
        .map_err(|error| ApiError::internal("tasks_snapshot_broadcast_failed", error))?;
    Ok(Json(task))
}

fn task_service(state: &HttpAppState) -> TaskService<'_> {
    TaskService::new(
        &state.core.database.pool,
        &state.core.download_tasks,
        &state.core.next_task_id,
        &state.core.debug_logs,
        &state.core.is_exiting,
    )
}

fn classify_aria2_ready_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("端口范围")
        || error.contains("已被其他进程占用")
        || error.contains("RPC 未就绪")
    {
        return ApiError::conflict("aria2_runtime_conflict", error);
    }
    ApiError::internal("aria2_runtime_failed", error)
}

fn classify_task_error(error: String) -> ApiError {
    if error.contains("应用正在退出") {
        return ApiError::conflict("runtime_exiting", error);
    }
    if error.contains("下载任务不存在")
        || error.contains("只有已完成任务可以重新下载")
        || error.contains("URL")
        || error.contains("文件名")
        || error.contains("保存目录")
        || error.contains("拒绝删除")
        || error.contains("当前仅支持删除单文件")
    {
        return ApiError::bad_request("task_operation_failed", error);
    }
    ApiError::internal("task_operation_failed", error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::error::ErrorResponse;
    use crate::app::{bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
    use crate::config::aria2::Aria2BinarySource;
    use crate::runtime::ManagedAria2Process;
    use crate::tasks::DownloadTaskStatus;
    use axum::response::Response;
    use axum::routing::post;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use serde_json::json;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;
    use std::sync::Mutex;
    use tower::ServiceExt;

    #[tokio::test]
    async fn create_and_list_routes_work_with_ready_aria2() {
        let mock = MockAria2Server::spawn().await;
        let (state, child_pid) = ready_state(&mock).await;
        let app = test_router(state.clone());
        let save_dir = temp_dir("task-downloads");

        let created = response_json::<DownloadTask>(
            app.clone()
                .oneshot(json_request(
                    "POST",
                    "/api/tasks",
                    &json!({
                        "url": "https://example.com/archive.zip",
                        "fileName": "archive.zip",
                        "saveDir": save_dir
                    }),
                ))
                .await
                .expect("create response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(created.id, 1);
        assert_eq!(created.gid.as_deref(), Some("gid-1"));
        assert_eq!(created.status, DownloadTaskStatus::Pending);

        let listed = response_json::<Vec<DownloadTask>>(
            app.oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("list response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, DownloadTaskStatus::Active);

        cleanup_state(&state, child_pid);
        mock.abort();
    }

    #[tokio::test]
    async fn pause_resume_and_delete_routes_update_task_state() {
        let mock = MockAria2Server::spawn().await;
        let (state, child_pid) = ready_state(&mock).await;
        let app = test_router(state.clone());
        let save_dir = temp_dir("task-downloads");

        let _ = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/tasks",
                &json!({
                    "url": "https://example.com/archive.zip",
                    "fileName": "archive.zip",
                    "saveDir": save_dir
                }),
            ))
            .await
            .expect("create response should succeed");

        let paused = response_json::<DownloadTask>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/tasks/1/pause")
                        .body(Body::empty())
                        .expect("pause request should build"),
                )
                .await
                .expect("pause response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(paused.status, DownloadTaskStatus::Paused);

        let resumed = response_json::<DownloadTask>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/tasks/1/resume")
                        .body(Body::empty())
                        .expect("resume request should build"),
                )
                .await
                .expect("resume response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(resumed.status, DownloadTaskStatus::Active);

        let removed = response_json::<DownloadTask>(
            app.clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri("/api/tasks/1?deleteFiles=false")
                        .body(Body::empty())
                        .expect("delete request should build"),
                )
                .await
                .expect("delete response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert_eq!(removed.status, DownloadTaskStatus::Removed);

        let listed = response_json::<Vec<DownloadTask>>(
            app.oneshot(
                Request::builder()
                    .uri("/api/tasks")
                    .body(Body::empty())
                    .expect("list request should build"),
            )
            .await
            .expect("list response should succeed"),
            StatusCode::OK,
        )
        .await;
        assert!(listed.is_empty());

        cleanup_state(&state, child_pid);
        mock.abort();
    }

    #[tokio::test]
    async fn task_mutations_reject_when_runtime_is_exiting() {
        let state = test_state().await;
        state.core.is_exiting.store(true, Ordering::SeqCst);
        let app = test_router(state);

        for request in [
            json_request(
                "POST",
                "/api/tasks",
                &json!({
                    "url": "https://example.com/archive.zip",
                    "fileName": "archive.zip",
                    "saveDir": temp_dir("task-downloads")
                }),
            ),
            Request::builder()
                .method("POST")
                .uri("/api/tasks/1/pause")
                .body(Body::empty())
                .expect("pause request should build"),
            Request::builder()
                .method("POST")
                .uri("/api/tasks/1/resume")
                .body(Body::empty())
                .expect("resume request should build"),
            Request::builder()
                .method("DELETE")
                .uri("/api/tasks/1?deleteFiles=false")
                .body(Body::empty())
                .expect("delete request should build"),
        ] {
            let error = response_json::<ErrorResponse>(
                app.clone()
                    .oneshot(request)
                    .await
                    .expect("response should succeed"),
                StatusCode::CONFLICT,
            )
            .await;
            assert_eq!(error.code, "runtime_exiting");
        }
    }

    fn test_router(state: Arc<HttpAppState>) -> Router {
        Router::new().nest("/api", routes()).with_state(state)
    }

    async fn test_state() -> Arc<HttpAppState> {
        let app_data_dir = temp_dir("tasks-api");
        let runtime = ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.sqlite"),
            app_data_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path: None,
        };

        bootstrap_http_app_state(&runtime)
            .await
            .expect("state should bootstrap")
    }

    async fn ready_state(mock: &MockAria2Server) -> (Arc<HttpAppState>, u32) {
        let state = test_state().await;
        let child = spawn_sleep_child();
        let child_pid = child.id();
        let config = crate::aria2::runtime_config(
            &state.base_aria2_config,
            mock.addr.port(),
            "secret".to_string(),
        );
        state
            .set_aria2_runtime(state.build_aria2_runtime_info(
                child_pid,
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

        (state, child_pid)
    }

    fn cleanup_state(state: &Arc<HttpAppState>, child_pid: u32) {
        state.clear_aria2_runtime();
        if let Some(mut child) = state
            .aria2_process
            .lock()
            .expect("process lock should succeed")
            .take()
        {
            let _ = child.kill();
        }
        let _ = crate::aria2::terminate_process(child_pid);
    }

    async fn response_json<T: DeserializeOwned>(
        response: Response,
        expected_status: StatusCode,
    ) -> T {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        assert_eq!(
            status,
            expected_status,
            "unexpected response body: {}",
            String::from_utf8_lossy(&body)
        );
        serde_json::from_slice(&body).expect("response json should deserialize")
    }

    fn json_request<T: Serialize>(method: &str, uri: &str, payload: &T) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_vec(payload).expect("payload should serialize"),
            ))
            .expect("request should build")
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
        async fn spawn() -> Self {
            let state = Arc::new(MockAria2State::default());
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

    #[derive(Default)]
    struct MockAria2State {
        next_gid: AtomicU64,
        tasks: Mutex<HashMap<String, MockTask>>,
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
            "aria2.getVersion" => json!({
                "result": {
                    "version": "1.37.0"
                }
            }),
            "aria2.addUri" => {
                let options_index = if first_param_is_token(&params) { 2 } else { 1 };
                let options = params
                    .get(options_index)
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let dir = options
                    .get("dir")
                    .and_then(Value::as_str)
                    .unwrap_or("/downloads")
                    .to_string();
                let file_name = options
                    .get("out")
                    .and_then(Value::as_str)
                    .unwrap_or("archive.zip")
                    .to_string();
                let gid = format!("gid-{}", state.next_gid.fetch_add(1, Ordering::SeqCst) + 1);
                state.tasks.lock().expect("tasks should lock").insert(
                    gid.clone(),
                    MockTask {
                        status: "active".to_string(),
                        dir,
                        file_name,
                    },
                );
                json!({ "result": gid })
            }
            "aria2.pause" => {
                let gid = gid_param(&params);
                if let Some(task) = state.tasks.lock().expect("tasks should lock").get_mut(&gid) {
                    task.status = "paused".to_string();
                }
                json!({ "result": gid })
            }
            "aria2.unpause" => {
                let gid = gid_param(&params);
                if let Some(task) = state.tasks.lock().expect("tasks should lock").get_mut(&gid) {
                    task.status = "active".to_string();
                }
                json!({ "result": gid })
            }
            "aria2.remove" | "aria2.removeDownloadResult" => {
                let gid = gid_param(&params);
                state.tasks.lock().expect("tasks should lock").remove(&gid);
                json!({ "result": gid })
            }
            "aria2.tellStatus" => {
                let gid = gid_param(&params);
                if let Some(task) = state
                    .tasks
                    .lock()
                    .expect("tasks should lock")
                    .get(&gid)
                    .cloned()
                {
                    json!({
                        "result": {
                            "gid": gid,
                            "status": task.status,
                            "totalLength": "1024",
                            "completedLength": "256",
                            "downloadSpeed": "128",
                            "dir": task.dir,
                            "files": [
                                {
                                    "path": format!("{}/{}", task.dir, task.file_name),
                                    "uris": []
                                }
                            ]
                        }
                    })
                } else {
                    json!({
                        "error": {
                            "message": "GID not found"
                        }
                    })
                }
            }
            _ => json!({
                "error": {
                    "message": format!("unsupported method: {}", method)
                }
            }),
        })
    }

    fn first_param_is_token(params: &[Value]) -> bool {
        params
            .first()
            .and_then(Value::as_str)
            .map(|value| value.starts_with("token:"))
            .unwrap_or(false)
    }

    fn gid_param(params: &[Value]) -> String {
        let index = if first_param_is_token(params) { 1 } else { 0 };
        params
            .get(index)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
}
