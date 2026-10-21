#![cfg(unix)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::Router;
use motrix_fnos_server::api::management_router;
use motrix_fnos_server::app::{bootstrap_http_app_state, HttpAppState, ServerRuntimeConfig};
use motrix_fnos_server::runtime::{
    ensure_aria2_ready, process_status, run_shutdown_cleanup, stop_process,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

struct Installation(PathBuf);

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Engine(Arc<HttpAppState>);

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = stop_process(&self.0.aria2_process, &self.0.core.debug_logs);
    }
}

async fn api(
    state: &Arc<HttpAppState>,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = management_router(state.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "requires the SHA-256 pinned native 2.8.6 candidate via MOTRIX_ARIA2_CANDIDATE_DIR"]
async fn malformed_source_api_rejects_pause_and_delete_then_recovers_original_task_after_bounded_shutdown(
) {
    let (asset, hash) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => (
            "macos-arm64",
            "6767bc6e79e22dbc491fe7bbcd36daca9dc0fb3b28abca7b9a9cfb8e6e67ece5",
        ),
        ("macos", "x86_64") => (
            "macos-x86_64",
            "933fb85f42adb6acd56e0e728b5d561908669dcdcb5e13459ec5c0195a960e95",
        ),
        ("linux", "aarch64") => (
            "linux-aarch64",
            "c72d3bc670aa3ede1ba1964109397c388b3b6bd85ca5d269370b87652e9694ad",
        ),
        ("linux", "x86_64") => (
            "linux-x86_64",
            "51a053d0b4d63a33425ed757a4acbc2a5d479bbd811b82d9a68c7d7f7fe9020c",
        ),
        other => panic!("unsupported candidate platform: {other:?}"),
    };
    let binary = PathBuf::from(std::env::var_os("MOTRIX_ARIA2_CANDIDATE_DIR").unwrap())
        .join(format!("aria2-next-2.8.6-{asset}"))
        .canonicalize()
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(fs::read(&binary).unwrap())),
        hash
    );
    let installation = Installation(std::env::temp_dir().join(format!(
            "motrix-native-safety-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let root = &installation.0;
    fs::create_dir_all(root.join("downloads")).unwrap();
    // Restrict the real engine to controlled loopback sources, without changing production defaults.
    let wrapper = root.join("aria2-next");
    fs::write(&wrapper, format!("#!/bin/sh\nexec '{}' \"$@\" --enable-dht=false --bt-enable-lpd=false --enable-peer-exchange=false --bt-port-mapping=false --interface=127.0.0.1 --disable-ipv6=true\n",
        binary.display().to_string().replace('\'', "'\\''"))).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let runtime = ServerRuntimeConfig {
        database_path: root.join("data/motrix-fnos.sqlite"),
        accessible_paths_path: root.join("data/accessible-paths.json"),
        app_data_dir: root.join("data"),
        http_addr: "127.0.0.1:0".parse().unwrap(),
        jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        aria2_path: Some(wrapper),
        trusted_proxy_ips: Vec::new(),
    };
    fs::create_dir_all(&runtime.app_data_dir).unwrap();
    let state = bootstrap_http_app_state(&runtime).await.unwrap();
    let engine = Engine(state.clone());
    assert_eq!(
        state.compatibility_snapshot().mode,
        "normal",
        "{:?}",
        state.compatibility_snapshot()
    );
    fs::write(
        &runtime.accessible_paths_path,
        json!({"paths": [root.join("downloads")]}).to_string(),
    )
    .unwrap();
    let auth = state
        .auth
        .service
        .initialize_password("native safety test password")
        .await
        .unwrap();
    let token = state.auth.service.issue_admin_token(&auth).await.unwrap();
    let source = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source_addr = source.local_addr().unwrap();
    let source_server = tokio::spawn(async move {
        axum::serve(
            source,
            Router::new().route(
                "/bad-range",
                get(|| async {
                    (
                        StatusCode::PARTIAL_CONTENT,
                        [("content-range", "bytes 0-131071/131072")],
                        vec![9u8; 262144],
                    )
                }),
            ),
        )
        .await
        .unwrap();
    });
    let (status, task) = api(&state, &token, "POST", "/api/tasks", json!({
        "url": format!("http://{source_addr}/bad-range"), "saveDir": root.join("downloads"),
        "fileName": "partial.bin", "advancedOptions": {"connections": 4}, "aria2Options": {"max-tries": "1"}
    })).await;
    assert_eq!(status, StatusCode::OK, "{task}");
    let id = task["id"].as_u64().unwrap();
    let gid = task["gid"].as_str().unwrap();
    let file = root.join("downloads/partial.bin");
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::metadata(&file).map_or(true, |m| m.len() == 0) {
        assert!(
            Instant::now() < deadline,
            "fixture did not write partial bytes"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    for (method, path) in [
        ("POST", format!("/api/tasks/{id}/pause")),
        ("DELETE", format!("/api/tasks/{id}?deleteFiles=true")),
    ] {
        let started = Instant::now();
        let (status, error) = api(&state, &token, method, &path, Value::Null).await;
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "operation exceeded confirmation budget"
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{error}");
        assert!(
            error.to_string().contains("task_pause_unconfirmed"),
            "{error}"
        );
        assert!(file.exists());
        assert_eq!(
            state.core.download_tasks.list().unwrap()[0].gid.as_deref(),
            Some(gid)
        );
        assert_ne!(
            state.core.download_tasks.list().unwrap()[0].status,
            motrix_fnos_server::tasks::DownloadTaskStatus::Removed
        );
    }
    assert!(
        motrix_fnos_server::database::task_operations::list_unfinished_task_operations(
            &state.core.database.pool
        )
        .await
        .unwrap()
        .is_empty(),
        "HTTP timeout must not strand an in-progress pause/delete"
    );
    state.core.shutdown.mark_exiting();
    let started = Instant::now();
    run_shutdown_cleanup(&state).await;
    assert!(started.elapsed() < Duration::from_secs(12));
    // Model the lifecycle script's final termination using only the owned Child handle.
    let forced = process_status(&state.aria2_process).unwrap().running;
    stop_process(&state.aria2_process, &state.core.debug_logs).unwrap();
    let bytes = fs::read(&file).unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.iter().all(|byte| *byte == 9));
    state.core.database.pool.close().await;
    drop(engine);
    drop(state);
    let restored = bootstrap_http_app_state(&runtime).await.unwrap();
    let restored_engine = Engine(restored.clone());
    let ready = ensure_aria2_ready(&restored).await.unwrap();
    let config = &*ready;
    let rpc: Value = reqwest::Client::new().post(config.rpc_url()).json(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "aria2.tellStatus", "params": [format!("token:{}", config.rpc_secret), gid]
    })).send().await.unwrap().json().await.unwrap();
    assert_eq!(rpc["result"]["gid"], gid, "{rpc}");
    assert_eq!(rpc["result"]["status"], "paused", "{rpc}");
    assert_eq!(fs::read(&file).unwrap(), bytes);
    let (status, tasks) = api(&restored, &token, "GET", "/api/tasks", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{tasks}");
    assert_eq!(tasks[0]["id"], id);
    assert_eq!(tasks[0]["gid"], gid);
    assert_eq!(tasks[0]["status"], "paused");
    eprintln!("real 2.8.6 {asset}: pause/delete rejected; shutdown {:?}; forced={forced}; original GID and {} file bytes retained", started.elapsed(), bytes.len());
    drop(ready);
    restored.core.shutdown.mark_exiting();
    run_shutdown_cleanup(&restored).await;
    assert!(!process_status(&restored.aria2_process).unwrap().running);
    restored.core.database.pool.close().await;
    drop(restored_engine);
    source_server.abort();
}
