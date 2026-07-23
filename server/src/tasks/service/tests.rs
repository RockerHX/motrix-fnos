use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::tasks::{DownloadTaskFile, TaskOperation};
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
    assert_eq!(task.owned_task_dir.as_deref(), Some(task.save_dir.as_str()));
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
            owned_task_dir: None,
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
            owned_task_dir: None,
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
    assert_eq!(task.owned_task_dir.as_deref(), Some(task.save_dir.as_str()));
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

#[tokio::test]
async fn restore_removed_url_task_returns_paused_task() {
    let mock = MockAria2Server::spawn().await;
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        temp_dir("restore-url").display().to_string(),
    );
    task.files_deleted = true;
    let fixture = ServiceFixture::new(vec![task], false);
    let config = test_config(mock.addr.port(), "secret");

    let restored = fixture
        .service()
        .restore_removed_task(&config, 1)
        .await
        .expect("removed URL task should restore");

    assert_eq!(restored.status, DownloadTaskStatus::Paused);
    assert_eq!(restored.gid.as_deref(), Some("gid-created"));
    assert_eq!(restored.completed_length, 0);
    assert!(!restored.files_deleted);
    assert_eq!(fixture.repository.persisted_tasks(), vec![restored]);

    mock.abort();
}

#[tokio::test]
async fn restore_removed_torrent_task_uses_private_metadata() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("restore-torrent");
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        save_dir.display().to_string(),
    );
    task.source_type = DownloadTaskSourceType::Torrent;
    task.url = "torrent:example.torrent".to_string();
    task.selected_file_indexes = vec![1, 3];
    task.files_deleted = true;
    let fixture = ServiceFixture::new(vec![task], false);
    let metadata_path = save_restore_torrent_metadata(&fixture.app_data_dir, 1, b"torrent")
        .expect("restore metadata should save");
    set_task_metadata_torrent_path(&fixture.tasks, 1, metadata_path.display().to_string())
        .expect("metadata path should update");
    let config = test_config(mock.addr.port(), "secret");

    let restored = fixture
        .service()
        .restore_removed_task(&config, 1)
        .await
        .expect("removed torrent task should restore");

    assert_eq!(restored.status, DownloadTaskStatus::Paused);
    assert_eq!(restored.gid.as_deref(), Some("gid-torrent"));
    assert!(save_dir.is_dir());

    mock.abort();
}

#[tokio::test]
async fn restore_removed_torrent_without_metadata_keeps_removed_state() {
    let mock = MockAria2Server::spawn().await;
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        temp_dir("restore-torrent-missing").display().to_string(),
    );
    task.source_type = DownloadTaskSourceType::Torrent;
    task.url = "torrent:missing.torrent".to_string();
    let fixture = ServiceFixture::new(vec![task], false);
    let config = test_config(mock.addr.port(), "secret");

    let error = fixture
        .service()
        .restore_removed_task(&config, 1)
        .await
        .expect_err("missing torrent metadata should reject restore");

    assert!(error.contains("缺少可恢复的源 metadata"));
    assert_eq!(
        fixture.tasks.list().expect("tasks should list")[0].status,
        DownloadTaskStatus::Removed
    );

    mock.abort();
}

#[tokio::test]
async fn restore_removed_magnet_without_metadata_restarts_parsing() {
    let mock = MockAria2Server::spawn().await;
    let task_dir = temp_dir("restore-magnet-missing").join("example");
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        task_dir.display().to_string(),
    );
    task.source_type = DownloadTaskSourceType::Magnet;
    task.url = "magnet:?xt=urn:btih:test".to_string();
    let fixture = ServiceFixture::new(vec![task], false);
    let config = test_config(mock.addr.port(), "secret");

    let restored = fixture
        .service()
        .restore_removed_task(&config, 1)
        .await
        .expect("magnet task should restart metadata parsing");

    assert_eq!(restored.status, DownloadTaskStatus::Pending);
    assert_eq!(restored.gid.as_deref(), Some("gid-created"));
    assert_eq!(
        restored.save_dir,
        task_dir.parent().unwrap().display().to_string()
    );
    assert!(fixture.app_data_dir.join("magnet-metadata/task-1").is_dir());

    mock.abort();
}

#[tokio::test]
async fn redownload_stages_old_file_until_new_task_is_running() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("redownload-safe");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"old file").expect("old file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Complete,
            "old-gid",
            save_dir.display().to_string(),
        )],
        false,
    );

    let task = fixture
        .service()
        .redownload_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect("redownload should succeed");

    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.gid.as_deref(), Some("gid-created"));
    assert!(
        !file_path.exists(),
        "old file should be removed only after restart"
    );
    assert!(std::fs::read_dir(&save_dir)
        .expect("save dir should read")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".motrix-redownload-backup")));

    mock.abort();
}

#[tokio::test]
async fn redownload_add_failure_keeps_old_file_and_task_snapshot() {
    let save_dir = temp_dir("redownload-add-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"old file").expect("old file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Complete,
            "old-gid",
            save_dir.display().to_string(),
        )],
        false,
    );

    let error = fixture
        .service()
        .redownload_download_task(&test_config(1, "secret"), 1)
        .await
        .expect_err("unreachable Aria2 should reject redownload");

    assert!(error.contains("无法连接 Aria2 RPC"));
    assert!(file_path.exists());
    assert_eq!(
        fixture.tasks.list().expect("tasks should list")[0].status,
        DownloadTaskStatus::Complete
    );
}

#[tokio::test]
async fn redownload_initial_persist_failure_restores_database_snapshot() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("redownload-initial-persist-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"old file").expect("old file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Complete,
            "old-gid",
            save_dir.display().to_string(),
        )],
        false,
    );
    fixture.repository.fail_persist_on_call(1);

    let error = fixture
        .service()
        .redownload_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("initial persistence failure should roll back redownload");

    assert!(error.contains("injected persist failure"));
    assert!(file_path.exists());
    let persisted = fixture.repository.persisted_tasks();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].status, DownloadTaskStatus::Complete);
    assert_eq!(persisted[0].gid.as_deref(), Some("old-gid"));

    mock.abort();
}

#[tokio::test]
async fn redownload_unpause_failure_restores_old_file_and_task_snapshot() {
    let mock = MockAria2Server::spawn_failing_unpause().await;
    let save_dir = temp_dir("redownload-unpause-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"old file").expect("old file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Complete,
            "old-gid",
            save_dir.display().to_string(),
        )],
        false,
    );

    let error = fixture
        .service()
        .redownload_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("unpause failure should roll back redownload");

    assert!(error.contains("cannot unpause"));
    assert_eq!(
        std::fs::read(&file_path).expect("old file should read"),
        b"old file"
    );
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Complete);
    assert_eq!(task.gid.as_deref(), Some("old-gid"));
    assert!(std::fs::read_dir(&save_dir)
        .expect("save dir should read")
        .filter_map(Result::ok)
        .all(|entry| !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".motrix-redownload-backup")));

    mock.abort();
}

#[tokio::test]
async fn redownload_final_persist_failure_restores_old_file_and_task_snapshot() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("redownload-persist-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"old file").expect("old file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Complete,
            "old-gid",
            save_dir.display().to_string(),
        )],
        false,
    );
    fixture.repository.fail_persist_on_call(2);

    let error = fixture
        .service()
        .redownload_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("final persistence failure should roll back redownload");

    assert!(error.contains("injected persist failure"));
    assert_eq!(
        std::fs::read(&file_path).expect("old file should read"),
        b"old file"
    );
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Complete);
    assert_eq!(task.gid.as_deref(), Some("old-gid"));

    mock.abort();
}

#[tokio::test]
async fn redownload_torrent_uses_add_torrent_and_preserves_metadata_source() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("redownload-torrent");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    std::fs::write(save_dir.join("payload.bin"), b"old payload").expect("old payload should write");
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Complete,
        "old-gid",
        save_dir.display().to_string(),
    );
    task.source_type = DownloadTaskSourceType::Torrent;
    task.url = "torrent:example.torrent".to_string();
    task.owned_task_dir = Some(save_dir.display().to_string());
    let fixture = ServiceFixture::new(vec![task], false);
    let metadata_path = save_restore_torrent_metadata(&fixture.app_data_dir, 1, b"torrent")
        .expect("metadata should save");
    set_task_metadata_torrent_path(&fixture.tasks, 1, metadata_path.display().to_string())
        .expect("metadata path should set");

    let task = fixture
        .service()
        .redownload_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect("torrent redownload should succeed");

    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.gid.as_deref(), Some("gid-torrent"));
    assert!(save_dir.is_dir());

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
    persist_calls: usize,
    fail_persist_call: Option<usize>,
}

impl FakeTaskRepository {
    fn fail_persist_on_call(&self, call: usize) {
        self.state
            .lock()
            .expect("repository state should lock")
            .fail_persist_call = Some(call);
    }

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
        let mut state = self.state.lock().expect("repository state should lock");
        state.persist_calls += 1;
        if state.fail_persist_call == Some(state.persist_calls) {
            return Err("injected persist failure".to_string());
        }
        state.persisted_tasks.push(task.clone());
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

    async fn begin_operation(&self, _operation: &TaskOperation) -> Result<(), String> {
        Ok(())
    }

    async fn update_operation(&self, _operation: &TaskOperation) -> Result<(), String> {
        Ok(())
    }

    async fn persist_task_state_with_operation(
        &self,
        task: &DownloadTask,
        _operation: &TaskOperation,
    ) -> Result<(), String> {
        self.persist_task_state(task).await
    }

    async fn list_unfinished_operations(&self) -> Result<Vec<TaskOperation>, String> {
        Ok(Vec::new())
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
        Self::spawn_with_router(app).await
    }

    async fn spawn_failing_unpause() -> Self {
        let app = Router::new().route("/jsonrpc", post(mock_aria2_rpc_failing_unpause));
        Self::spawn_with_router(app).await
    }

    async fn spawn_with_router(app: Router) -> Self {
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
    Json(mock_aria2_response(&payload))
}

async fn mock_aria2_rpc_failing_unpause(Json(payload): Json<Value>) -> Json<Value> {
    if payload.get("method").and_then(Value::as_str) == Some("aria2.unpause") {
        return Json(json!({ "error": { "message": "cannot unpause" } }));
    }
    Json(mock_aria2_response(&payload))
}

fn mock_aria2_response(payload: &Value) -> Value {
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();

    match method {
        "aria2.addUri" => json!({ "result": "gid-created" }),
        "aria2.addTorrent" => json!({ "result": "gid-torrent" }),
        "aria2.pause" | "aria2.unpause" => json!({ "result": "gid-created" }),
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
    }
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
        owned_task_dir: None,
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
