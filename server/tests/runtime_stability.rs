#![cfg(unix)]

use motrix_fnos_server::app::{bootstrap_http_app_state, ServerRuntimeConfig};
use motrix_fnos_server::database::tasks::upsert_download_task;
use motrix_fnos_server::settings::service::save_json_rpc_token;
use motrix_fnos_server::tasks::{DownloadTask, DownloadTaskStatus};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

struct Installation(PathBuf);

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Server(Child);

impl Server {
    async fn wait(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "server exceeded its exit budget");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
async fn native_idle_server_preserves_state_across_restarts_crash_and_http_drain_timeout() {
    let installation = Installation(std::env::temp_dir().join(format!(
        "motrix-stability-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
    )));
    let root = &installation.0;
    fs::create_dir_all(root).unwrap();
    let engine = root.join("aria2-next");
    fs::write(&engine, "#!/bin/sh\nif [ \"$1\" = --version ]; then printf 'aria2 version 2.8.6\\n'; else touch \"$0.started\"; exit 99; fi\n").unwrap();
    fs::set_permissions(&engine, fs::Permissions::from_mode(0o755)).unwrap();
    let management = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let rpc = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let lan = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let runtime = ServerRuntimeConfig {
        database_path: root.join("data/motrix-fnos.sqlite"),
        accessible_paths_path: root.join("data/accessible-paths.json"),
        app_data_dir: root.join("data"),
        http_addr: management.local_addr().unwrap(),
        jsonrpc_addr: rpc.local_addr().unwrap(),
        lan_jsonrpc_addr: lan.local_addr().unwrap(),
        aria2_path: Some(engine.clone()),
        trusted_proxy_ips: Vec::new(),
    };
    fs::create_dir_all(&runtime.app_data_dir).unwrap();
    let state = bootstrap_http_app_state(&runtime).await.unwrap();
    assert_eq!(
        state.compatibility_snapshot().mode,
        "normal",
        "{:?}",
        state.compatibility_snapshot()
    );
    let auth = state
        .auth
        .service
        .initialize_password("stability test password")
        .await
        .unwrap();
    let token = state.auth.service.issue_admin_token(&auth).await.unwrap();
    save_json_rpc_token(&state.core.database.pool, "stability-rpc")
        .await
        .unwrap();
    for (index, status) in ["paused", "complete", "removed"].into_iter().enumerate() {
        let task: DownloadTask = serde_json::from_value(json!({
            "id": index + 1, "engineGeneration": "native-v2", "url": "https://example.test/file",
            "sourceType": "url", "fileName": "file", "saveDir": root.join("downloads"),
            "category": "默认", "gid": format!("original-{}", index + 1), "status": status,
            "totalLength": 1024, "completedLength": if status == "complete" { 1024 } else { 512 },
            "downloadSpeed": 0, "useProxy": false, "confirmationRequired": false, "files": [], "createdAt": 1, "updatedAt": 1,
        })).unwrap();
        upsert_download_task(&state.core.database.pool, &task)
            .await
            .unwrap();
    }
    let native = runtime.app_data_dir.join("engine/native-v2/state");
    fs::create_dir_all(&native).unwrap();
    fs::write(native.join("preserved.bin"), b"private engine state").unwrap();
    state.core.database.pool.close().await;
    drop(state);
    drop((management, rpc, lan));
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let base = format!("http://{}", runtime.http_addr);
    let mut worst_stop = Duration::ZERO;
    let server_log = root.join("server.log");
    // After 30 clean exits, exercise SIGKILL/recovery and a deliberately stalled HTTP body.
    for round in 0..33 {
        let mut server = Server(
            Command::new(env!("CARGO_BIN_EXE_motrix-fnos-server"))
                .env("MOTRIX_FNOS_APP_DATA_DIR", &runtime.app_data_dir)
                .env("MOTRIX_FNOS_ARIA2_PATH", &engine)
                .env("MOTRIX_FNOS_HTTP_ADDR", runtime.http_addr.to_string())
                .env("MOTRIX_FNOS_JSONRPC_ADDR", runtime.jsonrpc_addr.to_string())
                .env(
                    "MOTRIX_FNOS_LAN_JSONRPC_ADDR",
                    runtime.lan_jsonrpc_addr.to_string(),
                )
                .stdout(Stdio::null())
                .stderr(fs::File::create(&server_log).unwrap())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(response) = client.get(format!("{base}/api/app/ready")).send().await {
                if response.status().is_success() {
                    break;
                }
            }
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "server failed in round {round}: {}",
                fs::read_to_string(&server_log).unwrap()
            );
            assert!(
                Instant::now() < deadline,
                "server not ready in round {round}"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let before = fs::read(&runtime.database_path).unwrap();
        let wal = runtime.database_path.with_extension("sqlite-wal");
        let before_wal = fs::read(&wal).unwrap_or_default();
        let mut sse = client
            .get(format!("{base}/api/events"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert!(sse.status().is_success());
        let mut initial = String::new();
        while !initial.contains("event: tasks.snapshot") {
            initial.push_str(&String::from_utf8_lossy(
                &sse.chunk().await.unwrap().unwrap(),
            ));
        }
        for _ in 0..3 {
            for path in ["/api/tasks", "/api/aria2/process", "/api/aria2/rpc"] {
                let response = client
                    .get(format!("{base}{path}"))
                    .bearer_auth(&token)
                    .send()
                    .await
                    .unwrap();
                assert!(response.status().is_success(), "{path} in round {round}");
                let body: Value = response.json().await.unwrap();
                if path == "/api/tasks" {
                    let mut tasks = body.as_array().unwrap().clone();
                    tasks.sort_by_key(|task| task["id"].as_u64().unwrap());
                    assert_eq!(tasks.len(), 2);
                    assert_eq!(tasks[0]["gid"], "original-1");
                    assert_eq!(tasks[0]["status"], "paused");
                    assert_eq!(tasks[0]["completedLength"], 512);
                    assert_eq!(tasks[1]["gid"], "original-2");
                    assert_eq!(tasks[1]["status"], "complete");
                    assert_eq!(tasks[1]["completedLength"], 1024);
                }
            }
            let response: Value = client.post(format!("http://{}/jsonrpc", runtime.jsonrpc_addr))
                .json(&json!({"jsonrpc":"2.0","id":1,"method":"aria2.getVersion","params":["token:stability-rpc"]}))
                .send().await.unwrap().json().await.unwrap();
            assert!(response.get("result").is_some(), "{response}");
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert_eq!(
            fs::read(&runtime.database_path).unwrap(),
            before,
            "idle DB writes in round {round}"
        );
        assert_eq!(
            fs::read(&wal).unwrap_or_default(),
            before_wal,
            "idle WAL writes in round {round}"
        );
        assert_eq!(
            fs::read(native.join("preserved.bin")).unwrap(),
            b"private engine state"
        );
        assert_eq!(fs::read_dir(&native).unwrap().count(), 1);
        assert!(
            !root.join("aria2-next.started").exists(),
            "a read awakened the engine"
        );
        let stalled = if round == 32 {
            let mut socket = tokio::net::TcpStream::connect(runtime.jsonrpc_addr)
                .await
                .unwrap();
            socket.write_all(b"POST /jsonrpc HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: 4096\r\n\r\n{").await.unwrap();
            tokio::time::sleep(Duration::from_millis(200)).await;
            Some(socket)
        } else {
            None
        };
        let started = Instant::now();
        if round == 30 {
            server.0.kill().unwrap();
            assert!(!server.wait().await.success());
        } else {
            assert_eq!(unsafe { libc::kill(server.0.id() as i32, libc::SIGINT) }, 0);
            let status = server.wait().await;
            if round == 32 {
                assert!(
                    !status.success(),
                    "a drain timeout must be reported as failure"
                );
                assert!(started.elapsed() >= Duration::from_secs(9));
                assert!(started.elapsed() < Duration::from_secs(12));
                assert!(fs::read_to_string(&server_log)
                    .unwrap()
                    .contains("HTTP 服务排空超过退出总时限"));
            } else {
                assert!(status.success(), "unclean exit in round {round}");
                assert!(
                    started.elapsed() < Duration::from_secs(3),
                    "idle SSE held shutdown open"
                );
                worst_stop = worst_stop.max(started.elapsed());
            }
        }
        drop(stalled);
        drop(sse);
        for address in [
            runtime.http_addr,
            runtime.jsonrpc_addr,
            runtime.lan_jsonrpc_addr,
        ] {
            assert!(tokio::net::TcpStream::connect(address).await.is_err());
        }
        assert!(!runtime.app_data_dir.join("aria2-runtime.json").exists());
    }
    let database = motrix_fnos_server::database::connect_database(runtime.database_path.clone())
        .await
        .unwrap();
    let mut tasks = motrix_fnos_server::database::tasks::list_download_tasks(&database.pool)
        .await
        .unwrap();
    tasks.sort_by_key(|task| task.id);
    assert_eq!(tasks.len(), 3);
    assert_eq!(
        tasks
            .iter()
            .map(|task| task.gid.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["original-1", "original-2", "original-3"]
    );
    assert_eq!(
        tasks
            .iter()
            .map(|task| task.status.clone())
            .collect::<Vec<_>>(),
        [
            DownloadTaskStatus::Paused,
            DownloadTaskStatus::Complete,
            DownloadTaskStatus::Removed
        ]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA quick_check")
            .fetch_one(&database.pool)
            .await
            .unwrap(),
        "ok"
    );
    database.pool.close().await;
    eprintln!("30 clean restarts + SIGKILL/recovery + 10s HTTP drain deadline passed; worst graceful stop: {worst_stop:?}");
}
