use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::tasks::DownloadTaskFile;
use axum::async_trait;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn create_download_task_rejects_when_runtime_is_exiting() {
    let fixture = ServiceFixture::new(Vec::new(), true);
    let config = test_config(6800, "");

    let error = fixture
        .service()
        .create_download_task(
            &config,
            CreateDownloadTaskRequest {
                url: "https://example.com/archive.zip".to_string(),
                file_name: Some("archive.zip".to_string()),
                save_dir: Some(temp_dir("service-exiting").display().to_string()),
                source_type: DownloadTaskSourceType::Url,
                start_mode: DownloadTaskStartMode::Now,
                category: None,
                advanced_options: CreateTaskAdvancedOptions::default(),
                aria2_options: serde_json::Map::new(),
            },
        )
        .await
        .expect_err("exiting runtime should reject task creation");

    assert!(error.contains("应用正在退出"));
    assert!(fixture.repository.upserted_tasks().is_empty());
    assert!(fixture.tasks.list().expect("tasks should list").is_empty());
}

#[tokio::test]
async fn create_download_task_persists_with_fake_repository() {
    let mock = MockAria2Server::spawn().await;
    let fixture = ServiceFixture::new(Vec::new(), false);
    let save_dir = temp_dir("service-create");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let config = test_config(mock.addr.port(), "secret");

    let task = fixture
        .service()
        .create_download_task(
            &config,
            CreateDownloadTaskRequest {
                url: "https://example.com/archive.zip".to_string(),
                file_name: Some("archive.zip".to_string()),
                save_dir: Some(save_dir.display().to_string()),
                source_type: DownloadTaskSourceType::Url,
                start_mode: DownloadTaskStartMode::Now,
                category: None,
                advanced_options: CreateTaskAdvancedOptions::default(),
                aria2_options: serde_json::Map::new(),
            },
        )
        .await
        .expect("task should create");

    assert_eq!(task.id, 1);
    assert_eq!(task.gid.as_deref(), Some("gid-created"));
    assert_eq!(task.status, DownloadTaskStatus::Pending);
    assert_eq!(fixture.repository.upserted_tasks().len(), 1);
    assert_eq!(fixture.tasks.list().expect("tasks should list").len(), 1);

    mock.abort();
}

#[tokio::test]
async fn create_torrent_download_task_persists_with_fake_repository() {
    let mock = MockAria2Server::spawn().await;
    let fixture = ServiceFixture::new(Vec::new(), false);
    let save_dir = temp_dir("service-create-torrent");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let config = test_config(mock.addr.port(), "secret");

    let task = fixture
        .service()
        .create_torrent_download_task(
            &config,
            CreateTorrentDownloadTaskRequest {
                torrent_file_name: "example.torrent".to_string(),
                torrent_data: b"torrent-bytes".to_vec(),
                save_dir: save_dir.display().to_string(),
                start_mode: DownloadTaskStartMode::Paused,
                category: None,
                advanced_options: CreateTaskAdvancedOptions::default(),
            },
        )
        .await
        .expect("torrent task should create");

    assert_eq!(task.id, 1);
    assert_eq!(task.gid.as_deref(), Some("gid-torrent"));
    assert_eq!(task.status, DownloadTaskStatus::Paused);
    assert_eq!(task.url, "torrent:example.torrent");
    assert_eq!(task.file_name, "example");
    assert!(PathBuf::from(&task.save_dir)
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with("example")));
    let metadata_path = task
        .metadata_torrent_path
        .as_deref()
        .expect("restore metadata path should persist");
    assert_eq!(
        std::fs::read(metadata_path).expect("restore metadata should read"),
        b"torrent-bytes"
    );
    assert_eq!(fixture.repository.upserted_tasks().len(), 1);

    mock.abort();
}

#[tokio::test]
async fn delete_download_task_marks_removed_and_persists() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("service-delete");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Active,
            "gid-1",
            save_dir.display().to_string(),
        )],
        false,
    );
    let config = test_config(mock.addr.port(), "secret");

    let task = fixture
        .service()
        .delete_download_task(&config, 1, false)
        .await
        .expect("task should delete");

    assert_eq!(task.status, DownloadTaskStatus::Removed);
    let persisted = fixture.repository.persisted_tasks();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].status, DownloadTaskStatus::Removed);
    let tasks = fixture.tasks.list().expect("tasks should list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, DownloadTaskStatus::Removed);

    mock.abort();
}

#[tokio::test]
async fn delete_download_task_cleans_metadata_dir_for_parsing_magnet_task() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("service-delete-magnet-save");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let fixture = ServiceFixture::new(
        vec![DownloadTask {
            id: 1,
            url: "magnet:?xt=urn:btih:test".to_string(),
            source_type: DownloadTaskSourceType::Magnet,
            file_name: "磁力链接任务".to_string(),
            save_dir: save_dir.display().to_string(),
            category: "默认".to_string(),
            gid: Some("gid-1".to_string()),
            status: DownloadTaskStatus::Pending,
            total_length: 0,
            completed_length: 0,
            download_speed: 0,
            error_code: None,
            error_message: None,
            file_path: None,
            metadata_torrent_path: None,
            files_deleted: false,
            selected_file_indexes: Vec::new(),
            confirmation_required: false,
            files: Vec::new(),
            created_at: 1,
            updated_at: 1,
        }],
        false,
    );
    let metadata_dir = fixture.app_data_dir.join("magnet-metadata").join("task-1");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir should create");
    std::fs::write(metadata_dir.join("metadata.torrent"), b"torrent")
        .expect("metadata file should write");
    let config = test_config(mock.addr.port(), "secret");

    fixture
        .service()
        .delete_download_task(&config, 1, false)
        .await
        .expect("task should delete");

    assert!(!metadata_dir.exists());

    mock.abort();
}

#[tokio::test]
async fn permanently_delete_removed_task_removes_memory_and_repository_record() {
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Removed,
            "gid-1",
            temp_dir("service-permanent-delete").display().to_string(),
        )],
        false,
    );
    let metadata_path = save_restore_torrent_metadata(&fixture.app_data_dir, 1, b"torrent")
        .expect("restore metadata should save");

    fixture
        .service()
        .permanently_delete_removed_task(1)
        .await
        .expect("removed task should permanently delete");

    assert_eq!(fixture.repository.deleted_task_ids(), vec![1]);
    assert!(fixture.tasks.list().expect("tasks should list").is_empty());
    assert!(!metadata_path.exists());
}

#[tokio::test]
async fn confirm_download_task_files_archives_restore_metadata() {
    let mock = MockAria2Server::spawn().await;
    let base_save_dir = temp_dir("service-confirm-magnet-save");
    std::fs::create_dir_all(&base_save_dir).expect("base save dir should create");
    let fixture = ServiceFixture::new(Vec::new(), false);
    let metadata_dir = fixture.app_data_dir.join("magnet-metadata").join("task-1");
    std::fs::create_dir_all(&metadata_dir).expect("metadata dir should create");
    let metadata_torrent_path = metadata_dir.join("metadata.torrent");
    std::fs::write(&metadata_torrent_path, b"torrent").expect("metadata torrent should write");
    let fixture = ServiceFixture {
        repository: fixture.repository.clone(),
        tasks: TaskMemoryState::new(vec![DownloadTask {
            id: 1,
            url: "magnet:?xt=urn:btih:test".to_string(),
            source_type: DownloadTaskSourceType::Magnet,
            file_name: "archlinux.iso".to_string(),
            save_dir: base_save_dir.display().to_string(),
            category: "默认".to_string(),
            gid: None,
            status: DownloadTaskStatus::Pending,
            total_length: 1024,
            completed_length: 0,
            download_speed: 0,
            error_code: None,
            error_message: None,
            file_path: None,
            metadata_torrent_path: Some(metadata_torrent_path.display().to_string()),
            files_deleted: false,
            selected_file_indexes: Vec::new(),
            confirmation_required: true,
            files: vec![DownloadTaskFile {
                index: 1,
                path: format!("{}/archlinux.iso/archlinux.iso", base_save_dir.display()),
                name: "archlinux.iso".to_string(),
                length: 1024,
                completed_length: 0,
                selected: true,
            }],
            created_at: 1,
            updated_at: 1,
        }]),
        next_task_id: AtomicU64::new(1),
        debug_logs: DebugLogStore::default(),
        shutdown: ShutdownState::new(),
        app_data_dir: fixture.app_data_dir.clone(),
    };
    let config = test_config(mock.addr.port(), "secret");

    let task = fixture
        .service()
        .confirm_download_task_files(&config, 1, vec![1])
        .await
        .expect("task files should confirm");

    assert!(!metadata_dir.exists());
    let restore_metadata_path = task
        .metadata_torrent_path
        .as_deref()
        .expect("restore metadata path should persist");
    assert_eq!(
        std::fs::read(restore_metadata_path).expect("restore metadata should read"),
        b"torrent"
    );
    assert_eq!(task.selected_file_indexes, [1]);
    let final_task_dir = PathBuf::from(&task.save_dir);
    assert!(final_task_dir.is_dir());
    assert_eq!(final_task_dir.file_name().unwrap(), "archlinux");
    assert_eq!(task.file_name, "archlinux.iso");
    assert!(std::fs::read_dir(&final_task_dir)
        .expect("final task dir should read")
        .filter_map(Result::ok)
        .all(|entry| entry.path().extension().and_then(|ext| ext.to_str()) != Some("torrent")));
    assert_eq!(task.gid.as_deref(), Some("gid-torrent"));

    mock.abort();
}

struct ServiceFixture {
    repository: Arc<FakeTaskRepository>,
    tasks: TaskMemoryState,
    next_task_id: AtomicU64,
    debug_logs: DebugLogStore,
    shutdown: ShutdownState,
    app_data_dir: PathBuf,
}

impl ServiceFixture {
    fn new(tasks: Vec<DownloadTask>, exiting: bool) -> Self {
        let shutdown = ShutdownState::new();
        if exiting {
            shutdown.mark_exiting();
        }

        Self {
            repository: Arc::new(FakeTaskRepository::default()),
            tasks: TaskMemoryState::new(tasks),
            next_task_id: AtomicU64::new(1),
            debug_logs: DebugLogStore::default(),
            shutdown,
            app_data_dir: temp_dir("service-app-data"),
        }
    }

    fn service(&self) -> TaskService<'_> {
        TaskService::new(
            Box::new(self.repository.clone()),
            &self.tasks,
            &self.next_task_id,
            &self.app_data_dir,
            &self.debug_logs,
            RuntimeGuard::new(&self.shutdown),
        )
    }
}

#[derive(Default)]
struct FakeTaskRepository {
    state: Mutex<FakeRepositoryState>,
}

#[derive(Default)]
struct FakeRepositoryState {
    upserted_tasks: Vec<DownloadTask>,
    persisted_tasks: Vec<DownloadTask>,
    persisted_task_batches: Vec<Vec<DownloadTask>>,
    deleted_task_ids: Vec<u64>,
    delete_result: bool,
}

impl FakeTaskRepository {
    fn upserted_tasks(&self) -> Vec<DownloadTask> {
        self.state
            .lock()
            .expect("repository state should lock")
            .upserted_tasks
            .clone()
    }

    fn persisted_tasks(&self) -> Vec<DownloadTask> {
        self.state
            .lock()
            .expect("repository state should lock")
            .persisted_tasks
            .clone()
    }

    fn deleted_task_ids(&self) -> Vec<u64> {
        self.state
            .lock()
            .expect("repository state should lock")
            .deleted_task_ids
            .clone()
    }
}

#[async_trait]
impl TaskRepository for Arc<FakeTaskRepository> {
    async fn upsert_task(&self, task: &DownloadTask) -> Result<(), String> {
        self.state
            .lock()
            .expect("repository state should lock")
            .upserted_tasks
            .push(task.clone());
        Ok(())
    }

    async fn persist_task_state(&self, task: &DownloadTask) -> Result<(), String> {
        self.state
            .lock()
            .expect("repository state should lock")
            .persisted_tasks
            .push(task.clone());
        Ok(())
    }

    async fn persist_task_states(&self, tasks: &[DownloadTask]) -> Result<(), String> {
        self.state
            .lock()
            .expect("repository state should lock")
            .persisted_task_batches
            .push(tasks.to_vec());
        Ok(())
    }

    async fn delete_task_record(&self, task_id: u64) -> Result<bool, String> {
        let mut guard = self.state.lock().expect("repository state should lock");
        guard.deleted_task_ids.push(task_id);
        Ok(guard.delete_result || !guard.deleted_task_ids.is_empty())
    }
}

struct MockAria2Server {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

impl MockAria2Server {
    async fn spawn() -> Self {
        let app = Router::new().route("/jsonrpc", post(mock_aria2_rpc));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock server should serve");
        });

        Self { addr, handle }
    }

    fn abort(self) {
        self.handle.abort();
    }
}

async fn mock_aria2_rpc(Json(payload): Json<Value>) -> Json<Value> {
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();

    Json(match method {
        "aria2.addUri" => json!({ "result": "gid-created" }),
        "aria2.addTorrent" => json!({ "result": "gid-torrent" }),
        "aria2.remove" | "aria2.removeDownloadResult" => {
            let gid = payload
                .get("params")
                .and_then(Value::as_array)
                .and_then(|params| params.iter().find_map(Value::as_str))
                .map(str::to_string)
                .unwrap_or_else(|| "gid-1".to_string());
            json!({ "result": gid })
        }
        other => json!({ "error": { "message": format!("unexpected method: {other}") } }),
    })
}

fn test_config(port: u16, rpc_secret: &str) -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: port,
        rpc_secret: rpc_secret.to_string(),
        session_path: None,
        log_path: None,
    }
}

pub(super) fn sample_task(
    id: u64,
    status: DownloadTaskStatus,
    gid: &str,
    save_dir: String,
) -> DownloadTask {
    DownloadTask {
        id,
        url: "https://example.com/archive.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: save_dir.clone(),
        category: "默认".to_string(),
        gid: Some(gid.to_string()),
        status,
        total_length: 1024,
        completed_length: 256,
        download_speed: 64,
        error_code: None,
        error_message: None,
        file_path: Some(
            PathBuf::from(&save_dir)
                .join("archive.zip")
                .display()
                .to_string(),
        ),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "motrix-fnos-task-service-{}-{}-{}",
        label,
        std::process::id(),
        counter
    ))
}

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);
