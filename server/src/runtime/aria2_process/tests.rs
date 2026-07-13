use super::resolve::{platform_binary_name, repo_debug_binary_path, resolve_aria2_binary_with};
use super::start::wait_for_rpc_ready;
use super::*;
use crate::app::{ServerRuntimeConfig, DEFAULT_HTTP_ADDR};
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

#[test]
fn resolve_aria2_binary_prefers_explicit_env_path() {
    let temp_dir = temp_dir("resolve-env");
    let explicit_path = temp_dir.join("custom-aria2");
    std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
    std::fs::write(&explicit_path, b"").expect("explicit path should exist");

    let runtime = sample_runtime(Some(explicit_path.clone()));
    let config = sample_config();
    let resolved = resolve_aria2_binary_with(&runtime, &config, None, None)
        .expect("explicit binary should resolve");

    assert_eq!(resolved.path, explicit_path);
    assert_eq!(resolved.source, Aria2BinarySource::ExternalPath);

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn resolve_aria2_binary_uses_packaged_path_before_repo_fallback() {
    let temp_dir = temp_dir("resolve-packaged");
    let current_exe = temp_dir.join("server").join("motrix-fnos-server");
    let packaged_path = current_exe
        .parent()
        .expect("current exe should have parent")
        .join("bin")
        .join(platform_binary_name("aria2-next"));
    let repo_root = temp_dir.join("repo");
    let repo_path = repo_debug_binary_path(&repo_root, &sample_config());

    std::fs::create_dir_all(
        packaged_path
            .parent()
            .expect("packaged parent should exist"),
    )
    .expect("packaged dir should create");
    std::fs::write(&packaged_path, b"").expect("packaged path should exist");
    std::fs::create_dir_all(repo_path.parent().expect("repo path should have parent"))
        .expect("repo dir should create");
    std::fs::write(&repo_path, b"").expect("repo path should exist");

    let runtime = sample_runtime(None);
    let config = sample_config();
    let resolved = resolve_aria2_binary_with(
        &runtime,
        &config,
        Some(current_exe.as_path()),
        Some(repo_root.as_path()),
    )
    .expect("packaged binary should resolve");

    assert_eq!(resolved.path, packaged_path);
    assert_eq!(resolved.source, Aria2BinarySource::Sidecar);

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn resolve_aria2_binary_falls_back_to_repo_debug_binary() {
    let temp_dir = temp_dir("resolve-repo");
    let repo_root = temp_dir.join("repo");
    let repo_path = repo_debug_binary_path(&repo_root, &sample_config());

    std::fs::create_dir_all(repo_path.parent().expect("repo path should have parent"))
        .expect("repo dir should create");
    std::fs::write(&repo_path, b"").expect("repo path should exist");

    let runtime = sample_runtime(None);
    let config = sample_config();
    let resolved = resolve_aria2_binary_with(&runtime, &config, None, Some(repo_root.as_path()))
        .expect("repo binary should resolve");

    assert_eq!(resolved.path, repo_path);
    assert_eq!(resolved.source, Aria2BinarySource::Sidecar);

    let _ = std::fs::remove_dir_all(temp_dir);
}

#[test]
fn process_status_reports_not_started_when_process_missing() {
    let process = Mutex::new(None);

    let status = process_status(&process).expect("status should load");

    assert!(!status.running);
    assert_eq!(status.pid, None);
    assert_eq!(status.binary_source, None);
    assert_eq!(status.message, "Aria2 进程未启动");
}

#[test]
fn process_status_clears_finished_process_handle() {
    let child = spawn_quick_exit_child();
    let process = Mutex::new(Some(ManagedAria2Process::new(
        child,
        Aria2BinarySource::Sidecar,
    )));
    std::thread::sleep(Duration::from_millis(80));

    let status = process_status(&process).expect("status should load");

    assert!(!status.running);
    assert!(status.pid.is_some());
    assert_eq!(status.binary_source, Some(Aria2BinarySource::Sidecar));
    assert!(process.lock().expect("lock should succeed").is_none());
}

#[test]
fn stop_process_succeeds_when_no_process_running() {
    let process = Mutex::new(None);
    let status = stop_process(&process, &DebugLogStore::default()).expect("stop should succeed");

    assert!(!status.running);
    assert_eq!(status.pid, None);
    assert_eq!(status.binary_source, None);
    assert_eq!(status.message, "Aria2 进程已停止");
}

#[tokio::test]
async fn wait_for_rpc_ready_only_writes_debug_success_after_startup() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock listener should bind");
    let port = listener
        .local_addr()
        .expect("mock addr should exist")
        .port();
    let app = Router::new().route("/jsonrpc", post(mock_version_rpc));
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock should serve");
    });
    let mut config = sample_config();
    config.rpc_port = port;
    let store = DebugLogStore::default();

    wait_for_rpc_ready(&config, &store, false)
        .await
        .expect("rpc should be ready");
    assert!(!store
        .list()
        .iter()
        .any(|entry| entry.message.contains("Aria2 RPC ready")));

    wait_for_rpc_ready(&config, &store, true)
        .await
        .expect("rpc should be ready");
    assert!(store
        .list()
        .iter()
        .any(|entry| entry.module == "aria2.rpc" && entry.message.contains("Aria2 RPC ready")));
}

async fn mock_version_rpc(Json(_payload): Json<Value>) -> Json<Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-version-check",
        "result": { "version": "2.4.9" }
    }))
}

fn sample_runtime(aria2_path: Option<PathBuf>) -> ServerRuntimeConfig {
    let app_data_dir = temp_dir("runtime");
    ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.db"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        gateway_socket_path: None,
        aria2_path,
    }
}

fn sample_config() -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: 6800,
        rpc_secret: "secret".to_string(),
        session_path: None,
        log_path: None,
    }
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-{}-{}",
        label,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis()
    ))
}

#[cfg(unix)]
fn spawn_quick_exit_child() -> Child {
    Command::new("sh")
        .args(["-c", "exit 0"])
        .spawn()
        .expect("shell should spawn")
}

#[cfg(windows)]
fn spawn_quick_exit_child() -> Child {
    Command::new("cmd")
        .args(["/C", "exit 0"])
        .spawn()
        .expect("cmd should spawn")
}
