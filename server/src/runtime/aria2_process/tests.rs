use super::resolve::{platform_binary_name, repo_debug_binary_path, resolve_aria2_binary_with};
use super::start::wait_for_rpc_ready;
use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::aria2::Aria2RpcClient;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
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
    let client = Aria2RpcClient::new();

    wait_for_rpc_ready(&client, &config, &store, false)
        .await
        .expect("rpc should be ready");
    assert!(!store
        .list()
        .iter()
        .any(|entry| entry.message.contains("Aria2 RPC ready")));

    wait_for_rpc_ready(&client, &config, &store, true)
        .await
        .expect("rpc should be ready");
    assert!(store
        .list()
        .iter()
        .any(|entry| entry.module == "aria2.rpc" && entry.message.contains("Aria2 RPC ready")));
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_start_requests_share_one_process_config_and_rpc_ready() {
    let temp_dir = temp_dir("concurrent-start");
    std::fs::create_dir_all(&temp_dir).expect("test directory should create");
    let aria2_path = temp_dir.join("fake-aria2");
    std::fs::write(
        &aria2_path,
        r##"#!/usr/bin/env python3
import json
import pathlib
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer

root = pathlib.Path(__file__).parent
process_count = root / "process-count.txt"
rpc_count = root / "rpc-count.txt"
process_count.open("a", encoding="utf-8").write("started\n")

port = next(
    int(argument.split("=", 1)[1])
    for argument in sys.argv[1:]
    if argument.startswith("--rpc-listen-port=")
)

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(length))
        with rpc_count.open("a", encoding="utf-8") as output:
            output.write(payload.get("method", "") + "\n")
        body = json.dumps({
            "jsonrpc": "2.0",
            "id": payload.get("id"),
            "result": {"version": "2.5.5"},
        }).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass

HTTPServer(("127.0.0.1", port), Handler).serve_forever()
"##,
    )
    .expect("fake Aria2 script should write");
    let mut permissions = std::fs::metadata(&aria2_path)
        .expect("fake Aria2 script metadata should load")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    std::fs::set_permissions(&aria2_path, permissions)
        .expect("fake Aria2 script should be executable");

    let runtime = ServerRuntimeConfig {
        database_path: temp_dir.join("motrix-fnos.db"),
        accessible_paths_path: temp_dir.join("accessible-paths.json"),
        app_data_dir: temp_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: Some(aria2_path),
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let mut state = crate::app::bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    let state_mut = Arc::get_mut(&mut state).expect("state should be uniquely owned");
    state_mut.base_aria2_config.rpc_host = "127.0.0.1".to_string();
    state_mut.base_aria2_config.rpc_port = 6800;
    state_mut.base_aria2_config.rpc_secret.clear();
    state_mut.base_aria2_config.session_path = None;
    state_mut.base_aria2_config.log_path = None;

    let requests = (0..8)
        .map(|_| {
            let state = Arc::clone(&state);
            tokio::spawn(async move { ensure_aria2_ready(&state).await })
        })
        .collect::<Vec<_>>();
    let mut results = Vec::with_capacity(requests.len());
    for request in requests {
        results.push(request.await.expect("start request should not panic"));
    }

    let stop_result = stop_process(&state.aria2_process, &state.core.debug_logs);
    state.clear_aria2_runtime();

    let process_count = std::fs::read_to_string(temp_dir.join("process-count.txt"))
        .expect("process count should be recorded");
    let rpc_methods = std::fs::read_to_string(temp_dir.join("rpc-count.txt"))
        .expect("RPC count should be recorded");
    std::fs::remove_dir_all(&temp_dir).expect("test directory should remove");

    stop_result.expect("Aria2 process should stop");
    let configs = results
        .into_iter()
        .map(|result| result.expect("concurrent start should succeed"))
        .collect::<Vec<_>>();
    let first = configs
        .first()
        .expect("at least one start result should exist");
    assert!(configs.iter().all(|config| {
        config.rpc_host == first.rpc_host
            && config.rpc_port == first.rpc_port
            && config.rpc_secret == first.rpc_secret
            && config.session_path == first.session_path
            && config.log_path == first.log_path
    }));
    assert_eq!(process_count.lines().count(), 1);
    assert_eq!(
        rpc_methods.lines().collect::<Vec<_>>(),
        ["aria2.getVersion"]
    );
}

#[tokio::test]
async fn start_failure_keeps_stopped_runtime_state() {
    let temp_dir = temp_dir("start-failure");
    let runtime = ServerRuntimeConfig {
        database_path: temp_dir.join("motrix-fnos.db"),
        accessible_paths_path: temp_dir.join("accessible-paths.json"),
        app_data_dir: temp_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path: Some(temp_dir.join("missing-aria2")),
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
    };
    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");

    let error = start_aria2(&state)
        .await
        .expect_err("missing Aria2 binary should reject startup");

    assert!(error.contains("路径不存在或不是文件"));
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(state
        .aria2_process
        .lock()
        .expect("process lock should succeed")
        .is_none());
    assert_eq!(
        state
            .aria2_lifecycle
            .snapshot()
            .expect("lifecycle snapshot should load")
            .phase,
        crate::runtime::Aria2LifecyclePhase::Faulted
    );
    state.core.database.pool.close().await;
    let _ = std::fs::remove_dir_all(temp_dir);
}

async fn mock_version_rpc(Json(_payload): Json<Value>) -> Json<Value> {
    Json(json!({
        "jsonrpc": "2.0",
        "id": "motrix-fnos-version-check",
        "result": { "version": "2.5.5" }
    }))
}

fn sample_runtime(aria2_path: Option<PathBuf>) -> ServerRuntimeConfig {
    let app_data_dir = temp_dir("runtime");
    ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.db"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir,
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().expect("addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().expect("addr should parse"),
        aria2_path,
        trusted_proxy_ips: Vec::new(),
        web_cookie_secure: false,
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
