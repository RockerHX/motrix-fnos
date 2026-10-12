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
    let runtime = crate::app::ServerRuntimeConfig {
        database_path: root.join("motrix-fnos.sqlite"),
        accessible_paths_path: root.join("accessible-paths.json"),
        app_data_dir: root.clone(),
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
