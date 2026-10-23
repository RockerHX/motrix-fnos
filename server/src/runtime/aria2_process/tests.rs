use super::resolve::{
    inspect_engine_generation, platform_binary_name, repo_debug_binary_path,
    resolve_aria2_binary_with,
};
use super::start::wait_for_rpc_ready;
use super::*;
use crate::app::{
    bootstrap_http_app_state, ServerRuntimeConfig, DEFAULT_HTTP_ADDR, DEFAULT_JSONRPC_ADDR,
};
use crate::aria2::Aria2RpcClient;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use crate::settings::service::{save_app_config, AppConfig};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex, OnceLock};
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

#[cfg(unix)]
#[test]
fn start_process_sets_utf8_path_encoding() {
    use std::os::unix::fs::PermissionsExt;

    const PROBE_ENV: &str = "MOTRIX_TEST_ENGINE_LOCALE_PROBE";
    if std::env::var_os(PROBE_ENV).is_none() {
        for lang in [None, Some("C"), Some("en_US.utf8")] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "runtime::aria2_process::tests::start_process_sets_utf8_path_encoding",
                ])
                .env(PROBE_ENV, "1")
                .env("LC_ALL", "C")
                .env_remove("LC_CTYPE")
                .env_remove("LANG");
            if let Some(lang) = lang {
                command.env("LANG", lang);
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "LANG={lang:?}: {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }

    let root = temp_dir("engine-locale");
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("fake-aria2");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s' \"${LANG-}\" > \"${0%/*}/engine-lang\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = sample_runtime(Some(binary));
    let mut config = sample_config();
    config.rpc_port = 0;
    let process = Mutex::new(None);
    start_process(&process, &runtime, &config, &DebugLogStore::default()).unwrap();
    let mut child = process.into_inner().unwrap().unwrap();
    let exited = child.wait_for_exit(Duration::from_secs(3)).unwrap();
    if !exited {
        child.kill().unwrap();
        assert!(child.wait_for_exit(Duration::from_secs(3)).unwrap());
    }
    let lang = std::fs::read_to_string(root.join("engine-lang"));
    std::fs::remove_dir_all(root).unwrap();
    assert!(exited, "locale probe should exit within the deadline");
    assert_eq!(lang.unwrap(), "C.UTF-8");
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
async fn concurrent_start_requests_apply_saved_download_config_once() {
    let _process_start_guard = process_start_test_guard().await;
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

if "--version" in sys.argv:
    print("aria2 version 2.5.5")
    sys.exit(0)

root = pathlib.Path(__file__).parent
process_count = root / "process-count.txt"
rpc_count = root / "rpc-count.txt"
global_options = root / "global-options.json"
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
        if payload.get("method") == "aria2.changeGlobalOption":
            global_options.write_text(json.dumps(payload), encoding="utf-8")
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
    };
    let initial_state = crate::app::bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    let default_download_dir = temp_dir.display().to_string();
    save_app_config(
        &initial_state.core.database.pool,
        AppConfig {
            default_download_dir: default_download_dir.clone(),
            max_concurrent_downloads: 9,
            http_max_connections: 6,
            connect_timeout: 90,
            max_tries: 8,
            download_limit: 1024,
            upload_limit: 2048,
            language: "zh-CN".to_string(),
        },
        &default_download_dir,
        std::slice::from_ref(&default_download_dir),
        &temp_dir,
    )
    .await
    .expect("download config should save");
    initial_state.core.database.pool.close().await;
    drop(initial_state);

    let mut state = crate::app::bootstrap_http_app_state(&runtime)
        .await
        .expect("saved state should bootstrap");
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
    state
        .clear_aria2_runtime()
        .expect("runtime record should clear");

    let process_count = std::fs::read_to_string(temp_dir.join("process-count.txt"))
        .expect("process count should be recorded");
    let rpc_methods = std::fs::read_to_string(temp_dir.join("rpc-count.txt"))
        .expect("RPC count should be recorded");
    let global_options: Value = serde_json::from_slice(
        &std::fs::read(temp_dir.join("global-options.json"))
            .expect("global options should be recorded"),
    )
    .expect("global options should be valid JSON");
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
        ["aria2.getVersion", "aria2.changeGlobalOption"]
    );
    assert_eq!(global_options["params"][1]["max-concurrent-downloads"], "9");
    assert_eq!(
        global_options["params"][1]["max-connection-per-server"],
        "6"
    );
    assert_eq!(global_options["params"][1]["split"], "6");
    assert_eq!(global_options["params"][1]["connect-timeout"], "90");
    assert_eq!(global_options["params"][1]["max-tries"], "8");
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

#[tokio::test]
async fn engine_generation_probe_accepts_only_verified_releases_and_has_a_deadline() {
    use crate::config::aria2::EngineGeneration;
    use std::os::unix::fs::PermissionsExt;
    let root = temp_dir("generation-probe");
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("engine");
    for (header, expected) in [
        ("aria2 version 2.5.5", Some(EngineGeneration::LegacyV1)),
        ("aria2 version 2.8.6", Some(EngineGeneration::NativeV2)),
        ("Aria2 Next version 2.8.6", Some(EngineGeneration::NativeV2)),
        ("Aria2 Next version 2.8.7", None),
        ("Aria2 Next version 3.0.8", None),
        ("aria2 version 99.0.0", None),
    ] {
        std::fs::write(&binary, format!("#!/bin/sh\nprintf '{header}\\n'\n")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = inspect_engine_generation(&binary).await;
        match expected {
            Some(generation) => assert_eq!(result.unwrap(), generation),
            None => assert!(result.is_err()),
        }
    }
    std::fs::write(&binary, "#!/bin/sh\nexit 1\n").unwrap();
    assert!(inspect_engine_generation(&binary).await.is_err());
    // exec 保持单个子进程，超时后不会遗留 shell 的后代进程。
    std::fs::write(
        &binary,
        "#!/bin/sh\nexec python3 -c 'import time; time.sleep(30)'\n",
    )
    .unwrap();
    let started = std::time::Instant::now();
    assert!(inspect_engine_generation(&binary)
        .await
        .unwrap_err()
        .contains("超时"));
    assert!(started.elapsed() < Duration::from_secs(4));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_startup_checks_empty_sessions_and_never_bypasses_a_failed_restore() {
    let _process_start_guard = process_start_test_guard().await;
    use std::os::unix::fs::PermissionsExt;
    for scenario in ["empty", "unknown", "bt-proxy", "bt-proxy-rejected"] {
        let unknown_gid = scenario == "unknown";
        let reject_proxy = scenario == "bt-proxy-rejected";
        let root = temp_dir(&format!("native-{scenario}"));
        std::fs::create_dir_all(&root).unwrap();
        let binary = root.join("fake-native");
        let script = r#"#!/usr/bin/env python3
import json, pathlib, signal, sys
from http.server import BaseHTTPRequestHandler, HTTPServer
if '--version' in sys.argv:
    print('aria2 version 2.8.6')
    sys.exit(0)
signal.signal(signal.SIGINT, lambda *args: sys.exit(0))
root = pathlib.Path(__file__).parent
(root / 'launch-args.json').write_text(json.dumps(sys.argv[1:]))
with (root / 'starts').open('a') as file: file.write('started\n')
port = int(next(arg.split('=', 1)[1] for arg in sys.argv if arg.startswith('--rpc-listen-port=')))
bt_proxy = ''
conf = next((arg.split('=', 1)[1] for arg in sys.argv if arg.startswith('--conf-path=')), None)
if (root / 'expected-proxy').exists():
    assert conf is not None and '--no-conf=true' not in sys.argv
    config = pathlib.Path(conf)
    assert config.stat().st_mode & 0o777 == 0o600
    bt_proxy = config.read_text().strip().split('=', 1)[1]
    assert bt_proxy == (root / 'expected-proxy').read_text()
class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        global bt_proxy
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        method = body['method']
        with (root / 'calls').open('a') as file: file.write(method + '\n')
        if method == 'aria2.getVersion': result = {'version': '2.8.6'}
        elif method == 'aria2.changeGlobalOption' and 'bt-proxy' in body['params'][-1]:
            bt_proxy = body['params'][-1]['bt-proxy']
            result = 'OK'
        elif method == 'aria2.getGlobalOption':
            result = {'bt-proxy': '' if (root / 'reject-proxy').exists() else bt_proxy}
        elif method == 'aria2.tellWaiting' and (root / 'unknown').exists():
            result = [{'gid': 'untracked-gid', 'status': 'paused', 'totalLength': '0', 'completedLength': '0', 'downloadSpeed': '0'}]
        elif method.startswith('aria2.tell'): result = []
        else: result = 'OK'
        response = json.dumps({'jsonrpc': '2.0', 'id': body['id'], 'result': result}).encode()
        self.send_response(200)
        self.send_header('Content-Length', str(len(response)))
        self.end_headers()
        self.wfile.write(response)
    def log_message(self, *args): pass
HTTPServer(('127.0.0.1', port), Handler).serve_forever()
"#;
        std::fs::write(&binary, script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        if unknown_gid {
            std::fs::write(root.join("unknown"), b"unknown").unwrap();
        }
        if reject_proxy {
            std::fs::write(root.join("reject-proxy"), b"reject").unwrap();
        }
        let runtime = ServerRuntimeConfig {
            database_path: root.join("motrix-fnos.sqlite"),
            accessible_paths_path: root.join("accessible-paths.json"),
            app_data_dir: root.clone(),
            http_addr: DEFAULT_HTTP_ADDR.parse().unwrap(),
            jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().unwrap(),
            lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
            aria2_path: Some(binary),
            trusted_proxy_ips: Vec::new(),
        };
        let state = bootstrap_http_app_state(&runtime).await.unwrap();
        if scenario.starts_with("bt-proxy") {
            std::fs::write(
                root.join("expected-proxy"),
                "socks5://test-user:test-secret@proxy.example:1080",
            )
            .unwrap();
            crate::database::settings::replace_download_proxy_config(
                &state.core.database.pool,
                Some("socks5://test-user:test-secret@proxy.example:1080".to_string()),
                Some(true),
                crate::database::settings::BtRuntimeApplyStatus::Deferred,
                1,
            )
            .await
            .unwrap();
        }
        for _ in 0..2 {
            let result = ensure_aria2_ready(&state).await;
            if unknown_gid || reject_proxy {
                assert!(result.err().unwrap().contains(if unknown_gid {
                    "task_resume_state_invalid"
                } else {
                    "BT 全局代理"
                }));
                assert_eq!(
                    state.aria2_lifecycle.snapshot().unwrap().phase,
                    crate::runtime::Aria2LifecyclePhase::Faulted
                );
            } else {
                let ready = result.unwrap();
                assert_eq!(
                    ready.engine_generation(),
                    crate::config::aria2::EngineGeneration::NativeV2
                );
                assert_eq!(
                    ready.session_path.as_deref(),
                    root.join("engine/native-v2/session/aria2.session").to_str()
                );
            }
        }
        let calls = std::fs::read_to_string(root.join("calls")).unwrap();
        assert!(calls.contains("aria2.forcePauseAll"));
        assert!(calls.contains("aria2.getGlobalOption"));
        if scenario.starts_with("bt-proxy") {
            let proxy =
                crate::database::settings::get_download_proxy_config(&state.core.database.pool)
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                proxy.bt_runtime_apply,
                if reject_proxy {
                    crate::database::settings::BtRuntimeApplyStatus::Blocked
                } else {
                    crate::database::settings::BtRuntimeApplyStatus::Applied
                }
            );
        }
        assert!(!calls.contains("aria2.add"));
        assert!(!calls.contains("aria2.unpause"));
        assert_eq!(
            calls
                .lines()
                .filter(|line| *line == "aria2.tellWaiting")
                .count(),
            if unknown_gid {
                2
            } else {
                usize::from(!reject_proxy)
            }
        );
        assert_eq!(
            std::fs::read_to_string(root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            if reject_proxy { 2 } else { 1 }
        );
        if scenario.starts_with("bt-proxy") {
            assert_eq!(
                std::fs::read_dir(root.join("engine/native-v2/startup"))
                    .unwrap()
                    .count(),
                0
            );
        }
        if reject_proxy {
            assert!(!process_status(&state.aria2_process).unwrap().running);
        }
        let args: Vec<String> =
            serde_json::from_slice(&std::fs::read(root.join("launch-args.json")).unwrap()).unwrap();
        assert!(!args.iter().any(|arg| arg.contains("test-secret")));
        assert!(args.iter().any(|arg| arg == "--pause=true"));
        assert!(args.iter().any(|arg| arg
            == &format!(
                "--state-dir={}",
                root.join("engine/native-v2/state").display()
            )));
        stop_process(&state.aria2_process, &state.core.debug_logs).unwrap();
        state.clear_aria2_runtime().unwrap();
        state.core.database.pool.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}

async fn process_start_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
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
        state_dir: None,
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
