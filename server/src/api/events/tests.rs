use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::runtime::broadcast_tasks_snapshot;
use crate::tasks::{DownloadTask, DownloadTaskStatus};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn sse_route_sends_initial_tasks_snapshot_event() {
    let app_data_dir = temp_dir("events-state");
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
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task()))
        .expect("tasks should lock");
    broadcast_tasks_snapshot(&state).expect("snapshot should broadcast");
    let app = Router::new()
        .nest("/api", routes())
        .with_state(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let mut body = response.into_body();
    let text = next_sse_frame(&mut body).await;
    assert!(text.contains("event: tasks.snapshot"));
    assert!(text.contains("\"archive.zip\""));
    assert!(text.contains("\"revision\":1"));
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
}

#[tokio::test]
async fn sse_route_resyncs_with_current_snapshot_after_lag() {
    let app_data_dir = temp_dir("events-lagged-state");
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
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    state
        .core
        .download_tasks
        .with_tasks_mut(|tasks| tasks.push(sample_task()))
        .expect("tasks should lock");
    let app = Router::new()
        .nest("/api", routes())
        .with_state(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/events")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("response should succeed");
    let mut body = response.into_body();
    assert!(next_sse_frame(&mut body).await.contains("\"revision\":0"));

    for _ in 0..40 {
        broadcast_tasks_snapshot(&state).expect("snapshot should broadcast");
    }

    let resync = next_sse_frame(&mut body).await;
    assert!(resync.contains("event: tasks.snapshot"));
    assert!(resync.contains("\"archive.zip\""));
    assert!(resync.contains("\"revision\":40"));
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
}

async fn next_sse_frame(body: &mut Body) -> String {
    let frame = body
        .frame()
        .await
        .expect("SSE frame should exist")
        .expect("SSE frame should be ok");
    let bytes = frame.into_data().expect("SSE frame should contain data");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn sample_task() -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        source_type: crate::tasks::DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: temp_dir("events-downloads").display().to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status: DownloadTaskStatus::Active,
        total_length: 1024,
        completed_length: 256,
        download_speed: 128,
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
        updated_at: 2,
    }
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}
