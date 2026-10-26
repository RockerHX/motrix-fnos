use super::*;
use crate::database::{
    connect_database,
    tasks::{delete_download_task_record, list_download_tasks, upsert_download_task},
    AppDatabase,
};
use axum::{routing::post, Json, Router};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

#[tokio::test]
async fn late_rpc_preserves_pause_resume_proxy_delete_and_active_operations() {
    for action in [
        "pause",
        "resume",
        "proxy",
        "delete",
        "busy",
        "database-proxy",
        "database-complete",
    ] {
        let (root, database, tasks) = fixture(action).await;
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let (config, server) = mock_status(
            if action == "database-complete" {
                "complete"
            } else {
                "active"
            },
            started.clone(),
            release.clone(),
        )
        .await;
        let original = tasks.list().unwrap().remove(0);
        let monitor_tasks = tasks.clone();
        let pool = database.pool.clone();
        let refresh = tokio::spawn(async move {
            refresh_tasks_from_aria2(&monitor_tasks, &pool, &Aria2RpcClient::new(), &config, None)
                .await
        });
        started.notified().await;
        let operation = tasks.begin_operation(1).unwrap();
        let mut next = original.clone();
        match action {
            "pause" | "database-complete" => next.status = DownloadTaskStatus::Paused,
            "resume" => next.updated_at += 1,
            "proxy" | "database-proxy" => {
                next.proxy_binding =
                    TaskProxyBinding::override_url("http://new-user:new-pass@proxy:8080".into())
            }
            "delete" | "busy" => {}
            _ => unreachable!(),
        }
        if action == "delete" {
            remove_task_record(&tasks, 1).unwrap();
            delete_download_task_record(&database.pool, 1)
                .await
                .unwrap();
        } else if action != "busy" {
            upsert_download_task(&database.pool, &next).await.unwrap();
            if !matches!(action, "database-proxy" | "database-complete") {
                tasks
                    .with_tasks_mut(|stored| stored[0] = next.clone())
                    .unwrap();
            }
        }
        let operation = if action == "busy" {
            Some(operation)
        } else {
            drop(operation);
            None
        };
        release.notify_one();
        let result = refresh.await.unwrap().unwrap();
        if action == "delete" {
            assert!(result.is_empty());
            assert!(list_download_tasks(&database.pool)
                .await
                .unwrap()
                .is_empty());
        } else {
            let expected = if matches!(action, "database-proxy" | "database-complete") {
                &original
            } else {
                &next
            };
            assert_eq!(&result[0], expected, "{action}");
            assert_eq!(
                list_download_tasks(&database.pool).await.unwrap()[0],
                next,
                "{action}"
            );
        }
        assert!(root.join("file.bin.aria2").exists(), "{action}");
        drop(operation);
        server.abort();
        database.pool.close().await;
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[tokio::test]
async fn failed_final_snapshot_retries_before_memory_publication_and_survives_reopen() {
    let (root, database, tasks) = fixture("retry-completion").await;
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let (config, server) = mock_status("complete", started, release.clone()).await;
    sqlx::query("CREATE TRIGGER fail_monitor BEFORE UPDATE ON download_tasks BEGIN SELECT RAISE(ABORT, 'injected monitor failure'); END").execute(&database.pool).await.unwrap();
    release.notify_one();
    assert!(refresh_tasks_from_aria2(
        &tasks,
        &database.pool,
        &Aria2RpcClient::new(),
        &config,
        None
    )
    .await
    .unwrap_err()
    .contains("injected monitor failure"));
    assert!(root.join("file.bin.aria2").exists());
    assert_eq!(tasks.list().unwrap()[0].status, DownloadTaskStatus::Active);
    assert_eq!(
        list_download_tasks(&database.pool).await.unwrap()[0].status,
        DownloadTaskStatus::Active
    );
    sqlx::query("DROP TRIGGER fail_monitor")
        .execute(&database.pool)
        .await
        .unwrap();
    release.notify_one();
    let saved = refresh_tasks_from_aria2(
        &tasks,
        &database.pool,
        &Aria2RpcClient::new(),
        &config,
        None,
    )
    .await
    .unwrap();
    assert_eq!(saved[0].status, DownloadTaskStatus::Complete);
    assert_eq!(saved[0].completed_length, 60);
    assert!(!root.join("file.bin.aria2").exists());
    database.pool.close().await;
    let reopened = connect_database(root.join("app.sqlite")).await.unwrap();
    assert_eq!(list_download_tasks(&reopened.pool).await.unwrap(), saved);
    reopened.pool.close().await;
    server.abort();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn identical_poll_does_not_write_or_change_the_business_timestamp() {
    let (root, database, tasks) = fixture("unchanged").await;
    let release = Arc::new(Notify::new());
    let (config, server) = mock_status("active", Arc::new(Notify::new()), release.clone()).await;
    release.notify_one();
    let first = refresh_tasks_from_aria2(
        &tasks,
        &database.pool,
        &Aria2RpcClient::new(),
        &config,
        None,
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_redundant_update BEFORE UPDATE ON download_tasks BEGIN SELECT RAISE(ABORT, 'unchanged task was written'); END").execute(&database.pool).await.unwrap();
    release.notify_one();
    let second = refresh_tasks_from_aria2(
        &tasks,
        &database.pool,
        &Aria2RpcClient::new(),
        &config,
        None,
    )
    .await
    .unwrap();
    assert_eq!(first, second);
    database.pool.close().await;
    server.abort();
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn removed_unconfirmed_tasks_are_never_polled_or_restored_by_monitoring() {
    let (root, database, tasks) = fixture("removed-confirmation").await;
    tasks
        .with_tasks_mut(|stored| {
            stored[0].status = DownloadTaskStatus::Removed;
            stored[0].confirmation_required = true;
        })
        .unwrap();
    let original = tasks.list().unwrap();
    upsert_download_task(&database.pool, &original[0])
        .await
        .unwrap();
    let config = Aria2Config {
        rpc_port: 0,
        state_dir: Some("/private/state".into()),
        ..Aria2Config::from_env()
    };
    let result = refresh_tasks_from_aria2(
        &tasks,
        &database.pool,
        &Aria2RpcClient::new(),
        &config,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result, original);
    database.pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}

async fn fixture(label: &str) -> (PathBuf, AppDatabase, Arc<TaskMemoryState>) {
    let root = std::env::temp_dir().join(format!(
        "motrix-refresh-{label}-{}-{}",
        std::process::id(),
        current_timestamp_ms()
    ));
    let database = connect_database(root.join("app.sqlite")).await.unwrap();
    let mut task: DownloadTask = serde_json::from_value(json!({
        "id":1, "engineGeneration":"native-v2", "url":"https://example.com/file.bin", "sourceType":"url",
        "fileName":"file.bin", "saveDir":"/downloads", "category":"默认", "gid":"same-gid", "status":"active",
        "totalLength":100, "completedLength":40, "downloadSpeed":10, "useProxy":true, "confirmationRequired":false,
        "files":[], "createdAt":1, "updatedAt":1
    })).unwrap();
    task.save_dir = root.display().to_string();
    std::fs::write(root.join("file.bin.aria2"), b"preserve until committed").unwrap();
    task.proxy_binding =
        TaskProxyBinding::override_url("http://old-user:old-pass@proxy:8080".into());
    upsert_download_task(&database.pool, &task).await.unwrap();
    (root, database, Arc::new(TaskMemoryState::new(vec![task])))
}

async fn mock_status(
    status: &'static str,
    started: Arc<Notify>,
    release: Arc<Notify>,
) -> (Aria2Config, tokio::task::JoinHandle<()>) {
    let app = Router::new().route("/jsonrpc", post(move |Json(_): Json<Value>| {
        let started = started.clone();
        let release = release.clone();
        async move {
            started.notify_one();
            release.notified().await;
            Json(json!({"result":{"gid":"same-gid", "status":status, "totalLength":"100", "completedLength":"60", "downloadSpeed":"10"}}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = Aria2Config {
        rpc_port: listener.local_addr().unwrap().port(),
        state_dir: Some("/private/state".into()),
        ..Aria2Config::from_env()
    };
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (config, server)
}
