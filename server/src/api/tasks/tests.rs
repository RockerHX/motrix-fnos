use super::*;
use crate::api::error::ErrorResponse;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::config::aria2::Aria2BinarySource;
use crate::runtime::ManagedAria2Process;
use crate::tasks::{DownloadTaskFile, DownloadTaskStatus};
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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use tower::ServiceExt;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn task_operation_conflict_maps_to_conflict_response() {
    let error = classify_task_error("该任务已有操作正在进行，请稍后重试".to_string());

    assert_eq!(error.status(), StatusCode::CONFLICT);
}

#[test]
fn file_cleanup_pending_maps_to_conflict_response() {
    let error = classify_task_error(
        "任务文件仍在后台清理，暂不能执行此操作（file_cleanup_pending）".to_string(),
    );

    assert_eq!(error.status(), StatusCode::CONFLICT);
}

#[test]
fn restore_and_redownload_proxy_override_body_is_optional() {
    assert!(parse_task_proxy_override_body(b"")
        .expect("empty body should inherit task proxy")
        .is_none());
    assert_eq!(
        parse_task_proxy_override_body(b"{}")
            .expect("empty JSON object should inherit task proxy")
            .expect("JSON body should be present")
            .use_proxy,
        None
    );
    assert_eq!(
        parse_task_proxy_override_body(br#"{"useProxy":true}"#)
            .expect("explicit override should parse")
            .expect("JSON body should be present")
            .use_proxy,
        Some(true)
    );
    assert_eq!(
        parse_task_proxy_override_body(b"not-json")
            .expect_err("invalid JSON should be rejected")
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn create_and_refresh_then_list_routes_work_with_ready_aria2() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

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

    let before_refresh = response_json::<Vec<DownloadTask>>(
        app.clone()
            .oneshot(
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
    assert_eq!(before_refresh.len(), 1);
    assert_eq!(before_refresh[0].status, DownloadTaskStatus::Pending);

    crate::runtime::monitor_tasks_once(&state)
        .await
        .expect("background refresh should succeed");

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

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_route_starts_paused_magnet_metadata_resolution() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-magnet-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

    let created = response_json::<DownloadTask>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &json!({
                "url": "magnet:?xt=urn:btih:test",
                "saveDir": save_dir.clone(),
                "sourceType": "magnet",
                "startMode": "paused"
            }),
        ))
        .await
        .expect("create response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(created.status, DownloadTaskStatus::Pending);
    assert_eq!(created.file_name, "磁力链接任务");
    assert_eq!(created.save_dir, save_dir);
    assert!(created.file_path.is_none());
    assert!(!PathBuf::from(&created.save_dir)
        .join("磁力链接任务")
        .exists());
    assert!(state
        .core
        .app_data_dir
        .join("magnet-metadata")
        .join("task-1")
        .is_dir());

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn confirm_task_files_route_validates_selection_and_starts_task() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-confirm-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

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

    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| {
            let task = tasks
                .iter_mut()
                .find(|task| task.id == created.id)
                .expect("created task should exist");
            std::fs::create_dir_all(&task.save_dir).expect("task dir should create");
            std::fs::write(
                std::path::Path::new(&task.save_dir).join("metadata.torrent"),
                b"torrent-bytes",
            )
            .expect("metadata torrent should write");
            task.gid = None;
            task.status = DownloadTaskStatus::Pending;
            task.confirmation_required = true;
            task.files = vec![DownloadTaskFile {
                index: 1,
                path: format!("{}/archive.zip", task.save_dir),
                name: "archive.zip".to_string(),
                length: 1024,
                completed_length: 0,
                selected: true,
            }];
        })
        .expect("tasks should lock");

    let error = response_json::<ErrorResponse>(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/api/tasks/1/confirm",
                &json!({ "selectedFileIndexes": [] }),
            ))
            .await
            .expect("empty confirm response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(error.code, "task_operation_failed");
    assert!(error.message.contains("至少选择一个文件"));

    let confirmed = response_json::<DownloadTask>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks/1/confirm",
            &json!({ "selectedFileIndexes": [1, 1] }),
        ))
        .await
        .expect("confirm response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(confirmed.status, DownloadTaskStatus::Active);
    assert_eq!(confirmed.gid.as_deref(), Some("gid-2"));
    assert!(!confirmed.confirmation_required);

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_route_accepts_category_and_advanced_options() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-advanced-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

    let created = response_json::<DownloadTask>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &json!({
                "url": "https://example.com/archive.zip",
                "fileName": "archive.zip",
                "saveDir": save_dir,
                "category": "电影",
                "advancedOptions": {
                    "connections": 8,
                    "downloadLimitKb": 512,
                    "proxy": "http://127.0.0.1:7890"
                }
            }),
        ))
        .await
        .expect("create response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(created.category, "电影");
    assert_eq!(created.gid.as_deref(), Some("gid-1"));
    assert!(created.use_proxy);
    let stored_override: Option<String> =
        sqlx::query_scalar("SELECT proxy_url FROM task_proxy_overrides WHERE task_id = ?")
            .bind(created.id as i64)
            .fetch_optional(&state.core.database.pool)
            .await
            .expect("private proxy override should load");
    assert_eq!(stored_override.as_deref(), Some("http://127.0.0.1:7890/"));

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_route_returns_structured_proxy_errors_without_side_effects() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-proxy-validation");
    let save_dir_text = save_dir.display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir_text));

    let conflict = response_json::<ErrorResponse>(
        app.clone()
            .oneshot(json_request(
                "POST",
                "/api/tasks",
                &json!({
                    "url": "https://example.com/archive.zip",
                    "saveDir": save_dir_text,
                    "advancedOptions": {
                        "useProxy": false,
                        "proxy": "http://127.0.0.1:7890"
                    }
                }),
            ))
            .await
            .expect("conflict response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(conflict.code, "proxy_conflict");

    let missing = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &json!({
                "url": "https://example.com/archive.zip",
                "saveDir": save_dir_text,
                "advancedOptions": { "useProxy": true }
            }),
        ))
        .await
        .expect("missing profile response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(missing.code, "proxy_not_configured");
    assert!(!save_dir.exists());
    assert!(state
        .core
        .download_tasks
        .list()
        .expect("tasks should list")
        .is_empty());

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_route_checks_proxy_before_starting_aria2() {
    let state = test_state().await;
    let save_dir = temp_dir("task-proxy-preflight");
    let save_dir_text = save_dir.display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir_text));
    let app = test_router(state.clone());

    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &json!({
                "url": "https://example.com/archive.zip",
                "saveDir": save_dir_text,
                "advancedOptions": { "useProxy": true }
            }),
        ))
        .await
        .expect("proxy preflight response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "proxy_not_configured");
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
    assert!(!save_dir.exists());
}

#[tokio::test]
async fn update_proxy_route_persists_without_starting_aria2() {
    let state = test_state().await;
    let task = sample_task(1, DownloadTaskStatus::Active);
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .expect("task should persist");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("task state should update");
    crate::database::settings::replace_download_proxy_config(
        &state.core.database.pool,
        "http://127.0.0.1:7890/".to_string(),
        1,
    )
    .await
    .expect("proxy profile should save");
    let app = test_router(state.clone());

    let updated = response_json::<DownloadTask>(
        app.oneshot(json_request(
            "PUT",
            "/api/tasks/1/proxy",
            &json!({ "enabled": true }),
        ))
        .await
        .expect("proxy update response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert!(updated.use_proxy);
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
    let stored = crate::database::tasks::list_download_tasks(&state.core.database.pool)
        .await
        .expect("stored tasks should load");
    assert!(stored[0].use_proxy);
}

#[tokio::test]
async fn update_proxy_route_applies_active_task_and_maps_rpc_failure() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Ready)
        .expect("lifecycle should be ready");
    let task = sample_task(1, DownloadTaskStatus::Active);
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .expect("task should persist");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("task state should update");
    crate::database::settings::replace_download_proxy_config(
        &state.core.database.pool,
        "http://127.0.0.1:7890/".to_string(),
        1,
    )
    .await
    .expect("proxy profile should save");
    let app = test_router(state.clone());

    let updated = response_json::<DownloadTask>(
        app.clone()
            .oneshot(json_request(
                "PUT",
                "/api/tasks/1/proxy",
                &json!({ "enabled": true }),
            ))
            .await
            .expect("proxy enable response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(updated.use_proxy);

    mock.state.fail_change_option.store(true, Ordering::SeqCst);
    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "PUT",
            "/api/tasks/1/proxy",
            &json!({ "enabled": false }),
        ))
        .await
        .expect("proxy disable failure response should succeed"),
        StatusCode::BAD_GATEWAY,
    )
    .await;
    assert_eq!(error.code, "proxy_apply_failed");
    assert!(state.core.download_tasks.list().expect("tasks should list")[0].use_proxy);

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn update_proxy_route_rejects_runtime_transition() {
    let state = test_state().await;
    let task = sample_task(1, DownloadTaskStatus::Active);
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .expect("task should persist");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("task state should update");
    crate::database::settings::replace_download_proxy_config(
        &state.core.database.pool,
        "http://127.0.0.1:7890/".to_string(),
        1,
    )
    .await
    .expect("proxy profile should save");
    state
        .aria2_lifecycle
        .set_phase(crate::runtime::Aria2LifecyclePhase::Starting)
        .expect("lifecycle should start transitioning");
    let app = test_router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "PUT",
            "/api/tasks/1/proxy",
            &json!({ "enabled": true }),
        ))
        .await
        .expect("runtime transition response should succeed"),
        StatusCode::CONFLICT,
    )
    .await;

    assert_eq!(error.code, "runtime_transition");
}

#[tokio::test]
async fn update_proxy_route_cleans_legacy_override_when_disabled() {
    let state = test_state().await;
    let mut task = sample_task(1, DownloadTaskStatus::Complete);
    task.use_proxy = true;
    task.proxy_binding = crate::tasks::TaskProxyBinding::override_url(
        "socks5://legacy.example.com:1080".to_string(),
    );
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .expect("legacy proxy task should persist");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("task state should update");
    let app = test_router(state.clone());

    let updated = response_json::<DownloadTask>(
        app.oneshot(json_request(
            "PUT",
            "/api/tasks/1/proxy",
            &json!({ "enabled": false }),
        ))
        .await
        .expect("proxy disable response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert!(!updated.use_proxy);
    let override_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM task_proxy_overrides WHERE task_id = 1")
            .fetch_one(&state.core.database.pool)
            .await
            .expect("private proxy override count should load");
    assert_eq!(override_count, 0);
    let source: String = sqlx::query_scalar("SELECT proxy_source FROM download_tasks WHERE id = 1")
        .fetch_one(&state.core.database.pool)
        .await
        .expect("proxy source should load");
    assert_eq!(source, "profile");
}

#[tokio::test]
async fn update_proxy_route_returns_structured_errors() {
    let state = test_state().await;
    let task = sample_task(1, DownloadTaskStatus::Active);
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &task)
        .await
        .expect("task should persist");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(task))
        .expect("task state should update");
    let app = test_router(state.clone());

    let missing_profile = response_json::<ErrorResponse>(
        app.clone()
            .oneshot(json_request(
                "PUT",
                "/api/tasks/1/proxy",
                &json!({ "enabled": true }),
            ))
            .await
            .expect("missing proxy response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;
    assert_eq!(missing_profile.code, "proxy_not_configured");

    let missing_task = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "PUT",
            "/api/tasks/999/proxy",
            &json!({ "enabled": true }),
        ))
        .await
        .expect("missing task response should succeed"),
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(missing_task.code, "task_not_found");
}

#[tokio::test]
async fn create_batch_route_returns_created_and_failed_items() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-batch-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));
    crate::database::settings::replace_download_proxy_config(
        &state.core.database.pool,
        "http://127.0.0.1:7890/".to_string(),
        1,
    )
    .await
    .expect("proxy profile should save");

    let result = response_json::<CreateBatchDownloadTasksResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks/batch",
            &json!({
                "urls": [
                    "https://example.com/archive-a.zip",
                    "ftp://example.com/archive-b.zip"
                ],
                "saveDir": save_dir,
                "advancedOptions": { "useProxy": true }
            }),
        ))
        .await
        .expect("batch response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(result.created.len(), 1);
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.created[0].url, "https://example.com/archive-a.zip");
    assert!(result.created[0].use_proxy);
    assert_eq!(result.failed[0].input, "ftp://example.com/archive-b.zip");

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_batch_route_returns_bad_request_when_all_items_fail() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-batch-failed-downloads")
        .display()
        .to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

    let result = response_json::<CreateBatchDownloadTasksResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks/batch",
            &json!({
                "urls": ["ftp://example.com/archive.zip"],
                "saveDir": save_dir
            }),
        ))
        .await
        .expect("batch response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert!(result.created.is_empty());
    assert_eq!(result.failed.len(), 1);
    assert_eq!(result.failed[0].input, "ftp://example.com/archive.zip");

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn create_batch_reports_proxy_preflight_failure_per_item_without_starting_aria2() {
    let state = test_state().await;
    let save_dir = temp_dir("task-batch-proxy-preflight");
    let save_dir_text = save_dir.display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir_text));
    let app = test_router(state.clone());

    let result = response_json::<CreateBatchDownloadTasksResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks/batch",
            &json!({
                "urls": [
                    "https://example.com/archive-a.zip",
                    "https://example.com/archive-b.zip"
                ],
                "saveDir": save_dir_text,
                "advancedOptions": { "useProxy": true }
            }),
        ))
        .await
        .expect("batch proxy preflight response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert!(result.created.is_empty());
    assert_eq!(result.failed.len(), 2);
    assert!(result
        .failed
        .iter()
        .all(|failure| failure.message.contains("未配置下载代理")));
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(!save_dir.exists());
}

#[tokio::test]
async fn create_torrent_route_accepts_multipart_upload() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-torrent-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

    let created = response_json::<DownloadTask>(
        app.oneshot(multipart_torrent_request(
            "/api/tasks/torrent",
            "example.torrent",
            b"torrent-bytes",
            &json!({
                "saveDir": save_dir,
                "startMode": "paused"
            }),
        ))
        .await
        .expect("torrent response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(created.url, "torrent:example.torrent");
    assert_eq!(created.file_name, "example");
    assert_eq!(
        PathBuf::from(&created.save_dir).file_name().unwrap(),
        "example"
    );
    assert_eq!(created.status, DownloadTaskStatus::Paused);

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn pause_resume_and_delete_routes_update_task_state() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let app = test_router(state.clone());
    let save_dir = temp_dir("task-downloads").display().to_string();
    write_accessible_paths(&state, std::slice::from_ref(&save_dir));

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
        app.clone()
            .oneshot(
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

    let removed_list = response_json::<Vec<DownloadTask>>(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri("/api/tasks?status=removed")
                    .body(Body::empty())
                    .expect("removed list request should build"),
            )
            .await
            .expect("removed list response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(removed_list.len(), 1);
    assert_eq!(removed_list[0].status, DownloadTaskStatus::Removed);

    let restored = response_json::<DownloadTask>(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/tasks/1/restore")
                    .body(Body::empty())
                    .expect("restore request should build"),
            )
            .await
            .expect("restore response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(restored.status, DownloadTaskStatus::Paused);

    let _ = response_json::<DownloadTask>(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/tasks/1?deleteFiles=false")
                    .body(Body::empty())
                    .expect("second delete request should build"),
            )
            .await
            .expect("second delete response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_status(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/tasks/1/permanent")
                    .body(Body::empty())
                    .expect("permanent delete request should build"),
            )
            .await
            .expect("permanent delete response should succeed"),
        StatusCode::NO_CONTENT,
    )
    .await;

    let removed_list = response_json::<Vec<DownloadTask>>(
        app.oneshot(
            Request::builder()
                .uri("/api/tasks?status=removed")
                .body(Body::empty())
                .expect("removed list request should build"),
        )
        .await
        .expect("removed list response should succeed"),
        StatusCode::OK,
    )
    .await;
    assert!(removed_list.is_empty());

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn task_mutations_reject_when_runtime_is_exiting() {
    let state = test_state().await;
    state.core.shutdown.mark_exiting();
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
            .method("POST")
            .uri("/api/tasks/1/restore")
            .body(Body::empty())
            .expect("restore request should build"),
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

#[tokio::test]
async fn list_removed_tasks_does_not_require_ready_aria2() {
    let state = test_state().await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| {
            tasks.extend([
                sample_task(1, DownloadTaskStatus::Active),
                sample_task(2, DownloadTaskStatus::Removed),
            ]);
        })
        .expect("tasks should lock");
    let app = test_router(state);

    let removed_list = response_json::<Vec<DownloadTask>>(
        app.oneshot(
            Request::builder()
                .uri("/api/tasks?status=removed")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(removed_list.len(), 1);
    assert_eq!(removed_list[0].id, 2);
    assert_eq!(removed_list[0].status, DownloadTaskStatus::Removed);
}

#[tokio::test]
async fn list_tasks_returns_memory_snapshot_when_aria2_is_stopped() {
    let state = test_state().await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(1, DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    let app = test_router(state.clone());

    let listed = response_json::<Vec<DownloadTask>>(
        app.oneshot(
            Request::builder()
                .uri("/api/tasks")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, DownloadTaskStatus::Active);
    assert!(state.aria2_runtime_snapshot().is_none());
}

#[tokio::test]
async fn list_tasks_does_not_refresh_or_persist_when_aria2_is_ready() {
    let mock = MockAria2Server::spawn().await;
    let state = ready_state(&mock).await;
    let persisted = sample_task(1, DownloadTaskStatus::Paused);
    crate::database::tasks::upsert_download_task(&state.core.database.pool, &persisted)
        .await
        .expect("persisted task should save");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(1, DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    let app = test_router(state.clone());
    let request_count = mock.request_count();

    let listed = response_json::<Vec<DownloadTask>>(
        app.oneshot(
            Request::builder()
                .uri("/api/tasks")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::OK,
    )
    .await;

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].status, DownloadTaskStatus::Active);
    assert_eq!(mock.request_count(), request_count);
    assert!(state.aria2_runtime_snapshot().is_some());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_some());

    let stored = crate::database::tasks::list_download_tasks(&state.core.database.pool)
        .await
        .expect("stored tasks should load");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].status, DownloadTaskStatus::Paused);
    assert_eq!(stored[0].updated_at, persisted.updated_at);

    cleanup_state(&state);
    mock.abort();
}

#[tokio::test]
async fn list_tasks_rejects_unsupported_status_filter() {
    let state = test_state().await;
    let app = test_router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(
            Request::builder()
                .uri("/api/tasks?status=active")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "task_status_filter_invalid");
}

#[tokio::test]
async fn permanent_delete_rejects_non_removed_task() {
    let state = test_state().await;
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task(1, DownloadTaskStatus::Active)))
        .expect("tasks should lock");
    let app = test_router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/tasks/1/permanent")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "task_operation_failed");
}

#[tokio::test]
async fn create_route_rejects_unauthorized_save_dir() {
    let state = test_state().await;
    write_accessible_paths(&state, &["/vol1/authorized".to_string()]);
    let app = test_router(state);

    let error = response_json::<ErrorResponse>(
        app.oneshot(json_request(
            "POST",
            "/api/tasks",
            &json!({
                "url": "https://example.com/archive.zip",
                "fileName": "archive.zip",
                "saveDir": "/vol1/other"
            }),
        ))
        .await
        .expect("response should succeed"),
        StatusCode::BAD_REQUEST,
    )
    .await;

    assert_eq!(error.code, "save_dir_not_authorized");
}

fn test_router(state: Arc<HttpAppState>) -> Router {
    Router::new()
        .nest("/api", routes().merge(torrent_routes()))
        .with_state(state)
}

async fn test_state() -> Arc<HttpAppState> {
    let app_data_dir = temp_dir("tasks-api");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };

    bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap")
}

fn write_accessible_paths(state: &Arc<HttpAppState>, paths: &[String]) {
    std::fs::write(
        &state.runtime.accessible_paths_path,
        serde_json::to_vec(&json!({ "paths": paths })).expect("paths should serialize"),
    )
    .expect("accessible paths should write");
}

async fn ready_state(mock: &MockAria2Server) -> Arc<HttpAppState> {
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

    state
}

fn cleanup_state(state: &Arc<HttpAppState>) {
    let _ = crate::runtime::stop_process(&state.aria2_process, &state.core.debug_logs);
    state.clear_aria2_runtime();
}

async fn response_json<T: DeserializeOwned>(response: Response, expected_status: StatusCode) -> T {
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

async fn assert_status(response: Response, expected_status: StatusCode) {
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

fn multipart_torrent_request(
    uri: &str,
    file_name: &str,
    data: &[u8],
    request_payload: &Value,
) -> Request<Body> {
    let boundary = "motrix-fnos-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"torrent\"; filename=\"{file_name}\"\r\nContent-Type: application/x-bittorrent\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(data);
    body.extend_from_slice(
        format!(
            "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"request\"\r\nContent-Type: application/json\r\n\r\n{}\r\n--{boundary}--\r\n",
            serde_json::to_string(request_payload).expect("request payload should serialize")
        )
        .as_bytes(),
    );

    Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("multipart request should build")
}

fn sample_task(id: u64, status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id,
        url: format!("https://example.com/archive-{id}.zip"),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: format!("archive-{id}.zip"),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some(format!("gid-{id}")),
        status,
        total_length: 1024,
        completed_length: 256,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some(format!("/downloads/archive-{id}.zip")),
        use_proxy: false,
        proxy_binding: crate::tasks::TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: id,
        updated_at: id,
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}-{}-{}",
        label,
        std::process::id(),
        counter,
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

    fn request_count(&self) -> u64 {
        self.state.request_count.load(Ordering::SeqCst)
    }

    fn abort(self) {
        self.handle.abort();
    }
}

#[derive(Default)]
struct MockAria2State {
    next_gid: AtomicU64,
    request_count: AtomicU64,
    fail_change_option: AtomicBool,
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
    state.request_count.fetch_add(1, Ordering::SeqCst);
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
        "aria2.addTorrent" => {
            let options_index = if first_param_is_token(&params) { 3 } else { 2 };
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
            let gid = format!("gid-{}", state.next_gid.fetch_add(1, Ordering::SeqCst) + 1);
            state.tasks.lock().expect("tasks should lock").insert(
                gid.clone(),
                MockTask {
                    status: "active".to_string(),
                    dir,
                    file_name: "example".to_string(),
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
        "aria2.changeOption" => {
            let gid = gid_param(&params);
            if state.fail_change_option.load(Ordering::SeqCst) {
                json!({ "error": { "code": 1, "message": "cannot change option" } })
            } else {
                json!({ "result": gid })
            }
        }
        "aria2.getOption" => json!({ "result": {} }),
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
                                "index": 1,
                                "path": format!("{}/{}", task.dir, task.file_name),
                                "length": "1024",
                                "completedLength": "256",
                                "selected": "true",
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
