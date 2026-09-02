use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use motrix_fnos_server::api::{jsonrpc_router, management_router};
use motrix_fnos_server::app::{
    bootstrap_http_app_state, HttpAppState, ServerRuntimeConfig, DEFAULT_HTTP_ADDR,
    DEFAULT_JSONRPC_ADDR,
};
use motrix_fnos_server::aria2::runtime_config;
use motrix_fnos_server::config::aria2::Aria2BinarySource;
use motrix_fnos_server::database::connect_database;
use motrix_fnos_server::debug_logs::{
    RollingFileMakeWriter, DEFAULT_FILE_LOG_MAX_BYTES, DEFAULT_FILE_LOG_RETENTION,
};
use motrix_fnos_server::runtime::{monitor_tasks_once, stop_process, ManagedAria2Process};
use motrix_fnos_server::settings::service::save_json_rpc_token;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tower::ServiceExt;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[tokio::test(flavor = "current_thread")]
async fn idle_monitor_and_readonly_requests_keep_application_files_unchanged() {
    let app_data_dir = temp_dir("idle-file-stability");
    let runtime = ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.clone(),
        http_addr: DEFAULT_HTTP_ADDR.parse().expect("HTTP addr should parse"),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR
            .parse()
            .expect("JSON-RPC addr should parse"),
        lan_jsonrpc_addr: "127.0.0.1:0"
            .parse()
            .expect("LAN JSON-RPC addr should parse"),
        aria2_path: None,
        trusted_proxy_ips: Vec::new(),
    };
    let initial_database = connect_database(runtime.database_path.clone())
        .await
        .expect("initial database should connect");
    save_json_rpc_token(&initial_database.pool, "idle-stability-token")
        .await
        .expect("idle JSON-RPC token should save");
    initial_database.pool.close().await;

    let state = bootstrap_http_app_state(&runtime)
        .await
        .expect("state should bootstrap");
    state
        .auth
        .service
        .setup("idle stability test password")
        .await
        .expect("test auth should initialize");
    state
        .auth
        .service
        .set_protection(false, "idle stability test password")
        .await
        .expect("test auth protection should disable");
    state.mark_listeners_ready();

    let (rpc_port, rpc_server) = spawn_mock_aria2().await;
    let config = state
        .with_aria2_runtime_paths(runtime_config(
            &state.base_aria2_config,
            rpc_port,
            "idle-stability-secret".to_string(),
        ))
        .expect("Aria2 runtime paths should initialize");
    let child = spawn_idle_child();
    let child_pid = child.id();
    state
        .aria2_process
        .lock()
        .expect("Aria2 process should lock")
        .replace(ManagedAria2Process::new(child, Aria2BinarySource::Sidecar));
    state
        .set_aria2_runtime(state.build_aria2_runtime_info(
            child_pid,
            &config,
            Aria2BinarySource::Sidecar,
            Vec::new(),
        ))
        .expect("Aria2 runtime should persist");

    initialize_lifecycle_files(&app_data_dir, child_pid);
    let management = management_router(state.clone());
    let jsonrpc = jsonrpc_router(state.clone());
    for _ in 0..2 {
        exercise_idle_window(&state, &management, &jsonrpc).await;
    }

    let server_log_path = app_data_dir.join("logs/server.log");
    let writer = RollingFileMakeWriter::new(
        &server_log_path,
        DEFAULT_FILE_LOG_MAX_BYTES,
        DEFAULT_FILE_LOG_RETENTION,
    )
    .expect("test server log writer should initialize");
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .with_writer(writer)
        .finish();
    let tracing_guard = tracing::subscriber::set_default(subscriber);
    tracing::info!(module = "test.idle", "空闲文件稳定性基线已建立");
    tokio::time::sleep(Duration::from_millis(25)).await;

    let baseline = snapshot_tree(&app_data_dir);
    assert_required_idle_files(&app_data_dir, &baseline);
    for _ in 0..3 {
        exercise_idle_window(&state, &management, &jsonrpc).await;
    }
    tokio::time::sleep(Duration::from_millis(25)).await;
    let observed = snapshot_tree(&app_data_dir);
    let changes = describe_snapshot_changes(&baseline, &observed);

    drop(tracing_guard);
    stop_process(&state.aria2_process, &state.core.debug_logs)
        .expect("test Aria2 process should stop");
    state.clear_aria2_runtime();
    state.core.database.pool.close().await;
    rpc_server.abort();
    let _ = rpc_server.await;
    let _ = fs::remove_dir_all(&app_data_dir);

    assert!(
        changes.is_empty(),
        "空闲观察窗口内应用数据文件发生变化：\n{}",
        changes.join("\n")
    );
}

async fn exercise_idle_window(
    state: &Arc<HttpAppState>,
    management: &axum::Router,
    jsonrpc: &axum::Router,
) {
    monitor_tasks_once(state)
        .await
        .expect("idle task monitor tick should succeed");

    for uri in [
        "/api/app/ready",
        "/api/app/info",
        "/api/app/ping",
        "/api/settings",
        "/api/settings/jsonrpc-token",
        "/api/aria2/config",
        "/api/aria2/process",
        "/api/aria2/rpc",
    ] {
        let response = management
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("management request should build"),
            )
            .await
            .expect("management response should succeed");
        assert_eq!(response.status(), StatusCode::OK, "uri: {uri}");
        let _ = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("management response body should read");
    }

    let response = jsonrpc
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": "idle-version",
                        "method": "aria2.getVersion",
                        "params": []
                    })
                    .to_string(),
                ))
                .expect("JSON-RPC request should build"),
        )
        .await
        .expect("JSON-RPC response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("JSON-RPC body should read"),
    )
    .expect("JSON-RPC response should parse");
    assert_eq!(payload["result"]["version"], "2.5.5");

    let response = jsonrpc
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/jsonrpc")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": "idle-global-option",
                        "method": "aria2.getGlobalOption",
                        "params": ["token:idle-stability-token"]
                    })
                    .to_string(),
                ))
                .expect("JSON-RPC request should build"),
        )
        .await
        .expect("JSON-RPC response should succeed");
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("JSON-RPC body should read"),
    )
    .expect("JSON-RPC response should parse");
    assert_eq!(payload["result"]["dir"], "");
}

async fn spawn_mock_aria2() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock Aria2 listener should bind");
    let port = listener
        .local_addr()
        .expect("mock Aria2 addr should exist")
        .port();
    let handle = tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/jsonrpc",
            axum::routing::post(|| async {
                axum::Json(json!({
                    "jsonrpc": "2.0",
                    "id": "motrix-fnos-version-check",
                    "result": { "version": "2.5.5" }
                }))
            }),
        );
        axum::serve(listener, app)
            .await
            .expect("mock Aria2 server should serve");
    });
    (port, handle)
}

fn initialize_lifecycle_files(app_data_dir: &Path, child_pid: u32) {
    let log_dir = app_data_dir.join("logs");
    let run_dir = app_data_dir.join("run");
    fs::create_dir_all(&log_dir).expect("log directory should create");
    fs::create_dir_all(&run_dir).expect("run directory should create");
    fs::write(log_dir.join("lifecycle.log"), b"initialized\n")
        .expect("lifecycle log should initialize");
    fs::write(
        run_dir.join("motrix-fnos-server.pid"),
        format!("{}\n", child_pid),
    )
    .expect("PID file should initialize");
    fs::write(
        run_dir.join("motrix-fnos-server.starttime"),
        b"idle-stability-starttime\n",
    )
    .expect("start time file should initialize");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntrySnapshot {
    kind: EntryKind,
    size: u64,
    modified_ns: u128,
    contents: Option<Vec<u8>>,
}

type TreeSnapshot = BTreeMap<PathBuf, EntrySnapshot>;

fn snapshot_tree(root: &Path) -> TreeSnapshot {
    let mut snapshot = BTreeMap::new();
    snapshot_entry(root, root, &mut snapshot);
    snapshot
}

fn snapshot_entry(root: &Path, path: &Path, snapshot: &mut TreeSnapshot) {
    let metadata = fs::symlink_metadata(path)
        .unwrap_or_else(|error| panic!("读取快照元数据失败：{}（{}）", path.display(), error));
    let file_type = metadata.file_type();
    let kind = if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_file() {
        EntryKind::File
    } else if file_type.is_symlink() {
        EntryKind::Symlink
    } else {
        EntryKind::Other
    };
    let relative = path
        .strip_prefix(root)
        .expect("snapshot path should be rooted");
    let relative = if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative.to_path_buf()
    };
    snapshot.insert(
        relative,
        EntrySnapshot {
            kind,
            size: metadata.len(),
            modified_ns: modified_ns(&metadata),
            contents: file_type.is_file().then(|| {
                fs::read(path).unwrap_or_else(|error| {
                    panic!("读取快照文件失败：{}（{}）", path.display(), error)
                })
            }),
        },
    );

    if file_type.is_dir() {
        let mut children = fs::read_dir(path)
            .unwrap_or_else(|error| panic!("读取快照目录失败：{}（{}）", path.display(), error))
            .map(|entry| entry.expect("snapshot directory entry should read").path())
            .collect::<Vec<_>>();
        children.sort();
        for child in children {
            snapshot_entry(root, &child, snapshot);
        }
    }
}

fn modified_ns(metadata: &fs::Metadata) -> u128 {
    metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn assert_required_idle_files(app_data_dir: &Path, snapshot: &TreeSnapshot) {
    let database = app_data_dir.join("motrix-fnos.sqlite");
    for path in [
        database.clone(),
        PathBuf::from(format!("{}-wal", database.display())),
        PathBuf::from(format!("{}-shm", database.display())),
        app_data_dir.join("aria2/aria2.session"),
        app_data_dir.join("aria2-runtime.json"),
        app_data_dir.join("logs/server.log"),
        app_data_dir.join("logs/lifecycle.log"),
        app_data_dir.join("run/motrix-fnos-server.pid"),
        app_data_dir.join("run/motrix-fnos-server.starttime"),
    ] {
        let relative = path
            .strip_prefix(app_data_dir)
            .expect("required path should be under app data");
        assert!(
            snapshot.contains_key(relative),
            "空闲稳定性基线缺少文件：{}",
            path.display()
        );
    }
}

fn describe_snapshot_changes(before: &TreeSnapshot, after: &TreeSnapshot) -> Vec<String> {
    let paths = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut changes = Vec::new();
    for path in paths {
        match (before.get(&path), after.get(&path)) {
            (None, Some(_)) => changes.push(format!("新增：{}", path.display())),
            (Some(_), None) => changes.push(format!("删除：{}", path.display())),
            (Some(before), Some(after)) => {
                if before.kind != after.kind {
                    changes.push(format!(
                        "类型变化：{}（{:?} -> {:?}）",
                        path.display(),
                        before.kind,
                        after.kind
                    ));
                }
                if before.size != after.size {
                    changes.push(format!(
                        "大小变化：{}（{} -> {}）",
                        path.display(),
                        before.size,
                        after.size
                    ));
                }
                if before.modified_ns != after.modified_ns {
                    changes.push(format!(
                        "mtime 变化：{}（{} -> {}）",
                        path.display(),
                        before.modified_ns,
                        after.modified_ns
                    ));
                }
                if before.contents != after.contents {
                    changes.push(format!("内容变化：{}", path.display()));
                }
            }
            (None, None) => unreachable!("path union should contain at least one entry"),
        }
    }
    changes
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-{label}-{}-{sequence}",
        std::process::id()
    ))
}

#[cfg(unix)]
fn spawn_idle_child() -> Child {
    Command::new("sh")
        .args(["-c", "sleep 10"])
        .spawn()
        .expect("idle child should spawn")
}

#[cfg(windows)]
fn spawn_idle_child() -> Child {
    Command::new("powershell")
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"])
        .spawn()
        .expect("idle child should spawn")
}
