use super::*;
use crate::config::aria2::Aria2BinarySource;
use std::io::{Read, Seek};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct NativeProcessStderr(std::fs::File);

impl Drop for NativeProcessStderr {
    fn drop(&mut self) {
        let mut output = String::new();
        // 通过 Rust 测试捕获输出，断言失败时仍保留子进程诊断。
        match self
            .0
            .rewind()
            .and_then(|_| self.0.read_to_string(&mut output))
        {
            Ok(0) => {}
            Ok(_) => eprintln!("模拟引擎标准错误：\n{output}"),
            Err(error) => eprintln!("读取模拟引擎标准错误失败：{error}"),
        }
    }
}

fn native_process(
    ignore_interrupt: bool,
) -> (
    PathBuf,
    Mutex<Option<ManagedAria2Process>>,
    NativeProcessStderr,
) {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "motrix-native-exit-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("interrupts"), b"").unwrap();
    let stderr = NativeProcessStderr(std::fs::File::create_new(root.join("stderr")).unwrap());
    let script = r#"
import pathlib, signal, sys, time
root = pathlib.Path(sys.argv[1])
def stop(*args):
    with (root / 'interrupts').open('a') as output:
        output.write('interrupt\n')
    if (root / 'checkpoint-delay').exists():
        time.sleep(float((root / 'checkpoint-delay').read_text()))
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
        .stderr(stderr.0.try_clone().unwrap())
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
        stderr,
    )
}

#[test]
fn native_stop_waits_for_checkpoint_and_normal_exit_before_reporting_success() {
    let (root, process, _stderr) = native_process(false);
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
    let (root, process, _stderr) = native_process(true);
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

#[test]
fn native_stop_waits_for_slow_checkpoint_within_the_daily_budget() {
    let (root, process, _stderr) = native_process(false);
    std::fs::write(root.join("checkpoint-delay"), "0.2").unwrap();
    let started = std::time::Instant::now();
    stop_process_with_timeout_inner(&process, None, Duration::from_secs(2), true).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(200));
    assert_eq!(
        std::fs::read(root.join("checkpoint")).unwrap(),
        b"committed"
    );
    assert!(process.lock().unwrap().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_stop_timeout_recovers_after_status_queries_observe_late_exit() {
    let (root, process, _stderr) = native_process(false);
    std::fs::write(root.join("checkpoint-delay"), "0.3").unwrap();
    let state = ready_native_state(&root, process, 0).await;
    let error = stop_process_with_timeout_inner(
        &state.aria2_process,
        None,
        Duration::from_millis(20),
        true,
    )
    .unwrap_err();
    assert!(error.contains("超时"));
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Faulted)
        .unwrap();

    let error = crate::runtime::ensure_aria2_ready(&state)
        .await
        .err()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while process_status(&state.aria2_process).unwrap().running {
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let retained_exit = state.aria2_process.lock().unwrap().is_some();
    let recovery = crate::runtime::monitor_tasks_once(&state).await;
    let runtime_cleared =
        state.aria2_runtime_snapshot().is_none() && !state.core.aria2_runtime_path.exists();
    let phase = state.aria2_lifecycle.snapshot().unwrap().phase;
    state.core.database.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();

    assert!(error.contains("正在停止"), "{error}");
    assert!(
        retained_exit,
        "status reads must retain the native exit result"
    );
    recovery.unwrap();
    assert!(runtime_cleared);
    assert_eq!(phase, Aria2LifecyclePhase::Stopped);
}

#[test]
fn native_stop_retry_does_not_interrupt_checkpoint_twice() {
    let (root, process, _stderr) = native_process(false);
    std::fs::write(root.join("checkpoint-delay"), "0.3").unwrap();
    assert!(
        stop_process_with_timeout_inner(&process, None, Duration::from_millis(20), true,)
            .unwrap_err()
            .contains("超时")
    );
    stop_process_with_timeout_inner(&process, None, Duration::from_secs(2), true).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("interrupts")).unwrap(),
        "interrupt\n"
    );
    assert_eq!(
        std::fs::read(root.join("checkpoint")).unwrap(),
        b"committed"
    );
    assert!(process.lock().unwrap().is_none());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn native_late_checkpoint_failure_keeps_runtime_after_status_queries() {
    let (root, process, _stderr) = native_process(false);
    std::fs::write(root.join("checkpoint-delay"), "0.3").unwrap();
    std::fs::create_dir(root.join("checkpoint")).unwrap();
    let state = ready_native_state(&root, process, 0).await;
    assert!(stop_process_with_timeout_inner(
        &state.aria2_process,
        None,
        Duration::from_millis(20),
        true,
    )
    .unwrap_err()
    .contains("超时"));
    state
        .aria2_lifecycle
        .set_phase(Aria2LifecyclePhase::Faulted)
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while process_status(&state.aria2_process).unwrap().running {
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    for _ in 0..2 {
        let error = crate::runtime::monitor_tasks_once(&state)
            .await
            .unwrap_err();
        assert!(error.contains("异常退出"), "{error}");
        let error = crate::runtime::ensure_aria2_ready(&state)
            .await
            .err()
            .unwrap();
        assert!(error.contains("异常退出"), "{error}");
        assert!(state.aria2_runtime_snapshot().is_some());
        assert!(state.core.aria2_runtime_path.exists());
        assert!(state.aria2_process.lock().unwrap().is_some());
    }
    stop_process(&state.aria2_process, &state.core.debug_logs).unwrap();
    state.core.database.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_stop_rejects_checkpoint_io_errors_and_preserves_previous_state() {
    use std::os::unix::fs::PermissionsExt;
    for readonly in [false, true] {
        if readonly && unsafe { libc::geteuid() } == 0 {
            eprintln!(
                "root bypasses directory write permissions; readonly checkpoint case skipped"
            );
            continue;
        }
        let (root, process, _stderr) = native_process(false);
        std::fs::write(root.join("previous-state"), b"preserved").unwrap();
        if readonly {
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).unwrap();
        } else {
            std::fs::create_dir(root.join("checkpoint")).unwrap();
        }
        let result = stop_process_with_timeout_inner(&process, None, Duration::from_secs(2), true);
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(result.unwrap_err().contains("异常退出"));
        assert_eq!(
            std::fs::read(root.join("previous-state")).unwrap(),
            b"preserved"
        );
        assert!(process.lock().unwrap().is_some());
        stop_process_with_timeout_inner(&process, None, Duration::from_secs(2), false).unwrap();
        assert!(process.lock().unwrap().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn native_idle_stop_keeps_confirmation_paused_and_removed_task_identities() {
    use axum::{routing::post, Json, Router};
    let (root, process, _stderr) = native_process(false);
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
        aria2_path: Some(crate::test_support::native_version_only_engine(root)),
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
        let (root, process, _stderr) = native_process(false);
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
