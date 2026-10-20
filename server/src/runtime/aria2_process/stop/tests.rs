use super::*;
use crate::config::aria2::Aria2BinarySource;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

fn native_process(ignore_interrupt: bool) -> (PathBuf, Mutex<Option<ManagedAria2Process>>) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "motrix-native-exit-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let script = r#"
import pathlib, signal, sys, time
root = pathlib.Path(sys.argv[1])
def stop(*args):
    (root / 'checkpoint').write_bytes(b'committed')
    sys.exit(0)
signal.signal(signal.SIGINT, signal.SIG_IGN if sys.argv[2] == 'ignore' else stop)
(root / 'ready').touch()
while True:
    time.sleep(0.01)
"#;
    let child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&root)
        .arg(if ignore_interrupt { "ignore" } else { "save" })
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !root.join("ready").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "fake native engine did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    (
        root,
        Mutex::new(Some(ManagedAria2Process::new(
            child,
            Aria2BinarySource::ExternalPath,
        ))),
    )
}

#[test]
fn native_stop_waits_for_checkpoint_and_normal_exit_before_reporting_success() {
    let (root, process) = native_process(false);
    let status =
        stop_process_with_timeout_inner(&process, None, Duration::from_secs(2), true).unwrap();
    assert!(!status.running);
    assert_eq!(
        std::fs::read(root.join("checkpoint")).unwrap(),
        b"committed"
    );
    assert!(process.lock().unwrap().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_stop_timeout_keeps_owned_process_and_does_not_claim_checkpoint_success() {
    let (root, process) = native_process(true);
    let started = std::time::Instant::now();
    let error = stop_process_with_timeout_inner(&process, None, Duration::from_millis(80), true)
        .unwrap_err();
    assert!(error.contains("超时"));
    assert!(started.elapsed() < Duration::from_millis(400));
    assert!(!root.join("checkpoint").exists());
    let mut guard = process.lock().unwrap();
    let child = guard.as_mut().unwrap();
    assert!(child.is_running().unwrap());
    child.kill().unwrap();
    assert!(child.wait_for_exit(Duration::from_secs(2)).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_idle_stop_keeps_confirmation_paused_and_removed_task_identities() {
    use axum::{routing::post, Json, Router};
    let (root, process) = native_process(false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Router::new().route(
        "/jsonrpc",
        post(|Json(body): Json<serde_json::Value>| async move {
            let result = if body["method"] == "aria2.tellActive" {
                serde_json::json!([])
            } else {
                serde_json::json!("OK")
            };
            Json(serde_json::json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        }),
    );
    let rpc = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let state = ready_native_state(&root, process, port).await;
    let tasks = ["pending", "paused", "removed"].into_iter().enumerate().map(|(index, status)| {
        serde_json::from_value::<DownloadTask>(serde_json::json!({
            "id": index + 1, "engineGeneration": "native-v2", "url": if index == 0 { "magnet:?xt=urn:btih:test" } else { "https://example.test/file" },
            "sourceType": if index == 0 { "magnet" } else { "url" }, "fileName": "file", "saveDir": root.join("downloads").display().to_string(),
            "category": "默认", "gid": format!("original-{}", index + 1), "status": status,
            "totalLength": 0, "completedLength": 0, "downloadSpeed": 0,
            "useProxy": false, "confirmationRequired": index == 0, "files": [], "createdAt": 1, "updatedAt": 1,
        })).unwrap()
    }).collect::<Vec<_>>();
    state
        .core
        .download_tasks
        .with_tasks_mut(|stored| *stored = tasks.clone())
        .unwrap();
    auto_stop_aria2(&state).await.unwrap();
    assert_eq!(state.core.download_tasks.list().unwrap(), tasks);
    assert_eq!(
        crate::database::tasks::list_download_tasks(&state.core.database.pool)
            .await
            .unwrap()
            .len(),
        3
    );
    assert!(state.aria2_runtime_snapshot().is_none());
    assert!(!state.core.aria2_runtime_path.exists());
    assert_eq!(
        std::fs::read(root.join("checkpoint")).unwrap(),
        b"committed"
    );
    state.core.database.pool.close().await;
    rpc.abort();
    std::fs::remove_dir_all(root).unwrap();
}

async fn ready_native_state(
    root: &std::path::Path,
    process: Mutex<Option<ManagedAria2Process>>,
    port: u16,
) -> std::sync::Arc<HttpAppState> {
    let runtime = crate::app::ServerRuntimeConfig {
        database_path: root.join("motrix-fnos.sqlite"),
        accessible_paths_path: root.join("accessible-paths.json"),
        app_data_dir: root.to_path_buf(),
        http_addr: "127.0.0.1:0".parse().unwrap(),
        jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };
    let state = crate::app::bootstrap_http_app_state(&runtime)
        .await
        .unwrap();
    let mut config = state.base_aria2_config.clone();
    config.rpc_port = port;
    config.state_dir = Some("native".to_string());
    let config = state.with_aria2_runtime_paths(config).unwrap();
    let child = process.into_inner().unwrap().unwrap();
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            child.id(),
            &config,
            Aria2BinarySource::ExternalPath,
            Vec::new(),
        ))
        .unwrap();
    *state.aria2_process.lock().unwrap() = Some(child);
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Ready)
        .unwrap();
    state
}

#[tokio::test]
async fn native_shutdown_requires_pause_before_session_and_checkpoint() {
    use axum::{routing::post, Json, Router};
    use serde_json::{json, Value};
    use std::sync::{atomic::AtomicBool, Arc};
    for fail_pause in [false, true] {
        let (root, process) = native_process(false);
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let paused = Arc::new(AtomicBool::new(false));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let rpc_calls = calls.clone();
        let router = Router::new().route(
            "/jsonrpc",
            post(move |Json(body): Json<Value>| {
                let calls = rpc_calls.clone();
                let paused = paused.clone();
                async move {
                    let method = body["method"].as_str().unwrap();
                    calls.lock().unwrap().push(method.to_string());
                    if method == "aria2.forcePauseAll" && fail_pause {
                        return Json(json!({"error":{"code":1,"message":"pause failed"}}));
                    }
                    let result = match method {
                        "aria2.forcePauseAll" => {
                            paused.store(true, Ordering::SeqCst);
                            json!("OK")
                        }
                        "aria2.tellActive" if !paused.load(Ordering::SeqCst) => {
                            json!([{"gid":"seed","bittorrent":{},"seeder":true,"uploadSpeed":"32"}])
                        }
                        "aria2.tellActive" => json!([]),
                        _ => json!("OK"),
                    };
                    Json(json!({"result":result}))
                }
            }),
        );
        let rpc = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let state = ready_native_state(&root, process, port).await;
        state.request_shutdown("测试原生退出");
        let completed = crate::runtime::run_shutdown_cleanup_until(
            &state,
            Instant::now() + Duration::from_secs(2),
        )
        .await;
        assert_eq!(completed, !fail_pause);
        assert_eq!(
            &calls.lock().unwrap()[..2],
            ["aria2.forcePauseAll", "aria2.saveSession"]
        );
        assert_eq!(root.join("checkpoint").exists(), !fail_pause);
        assert_eq!(state.aria2_runtime_snapshot().is_some(), fail_pause);
        assert_eq!(state.core.aria2_runtime_path.exists(), fail_pause);
        if fail_pause {
            stop_process(&state.aria2_process, &state.core.debug_logs).unwrap();
            state.clear_aria2_runtime().unwrap();
        }
        state.core.database.pool.close().await;
        rpc.abort();
        drop(state);
        std::fs::remove_dir_all(root).unwrap();
    }
}
