use super::*;
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::tasks::{DownloadTaskFile, TaskOperation, TaskOperationStatus, TaskOperationType};
use axum::async_trait;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    assert_eq!(fixture.repository.persisted_tasks().len(), 1);
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Create);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(
        operations[0].context.new_gid.as_deref(),
        Some("gid-created")
    );
    assert_eq!(fixture.tasks.list().expect("tasks should list").len(), 1);

    mock.abort();
}

#[tokio::test]
async fn create_download_task_recovers_when_aria2_created_task_before_timeout() {
    let mock = TimeoutAfterAddAria2Server::spawn().await;
    let mut fixture = ServiceFixture::new(Vec::new(), false);
    fixture.aria2_rpc = crate::aria2::Aria2RpcClient::with_timeouts(
        Duration::from_secs(1),
        Duration::from_millis(10),
    );
    let save_dir = temp_dir("service-create-timeout-reconcile");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");

    let task = fixture
        .service()
        .create_download_task(
            &test_config(mock.addr.port(), "secret"),
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
        .expect("outcome reconciliation should recover the created task");

    assert_eq!(task.gid.as_deref(), Some("gid-timeout"));
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(
        operations[0]
            .context
            .aria2_request
            .as_ref()
            .map(|request| request.request_id.as_str()),
        Some(operations[0].id.as_str())
    );
    assert_eq!(
        mock.add_request_ids().first().map(String::as_str),
        Some(operations[0].id.as_str())
    );
    assert_eq!(
        operations[0]
            .context
            .completed_side_effects
            .iter()
            .filter(|effect| effect.as_str() == "aria2_task_created")
            .count(),
        1
    );

    mock.abort();
}

#[tokio::test]
async fn create_download_task_marks_failed_when_aria2_request_is_not_delivered() {
    let fixture = ServiceFixture::new(Vec::new(), false);
    let save_dir = temp_dir("service-create-not-delivered");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");

    let error = fixture
        .service()
        .create_download_task(
            &test_config(1, "secret"),
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
        .expect_err("connection failure should reject task creation");

    assert!(error.contains("无法连接 Aria2 RPC"));
    assert!(fixture.tasks.list().expect("tasks should list").is_empty());
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "aria2_failed");
    assert!(operations[0].context.aria2_request.is_some());
}

#[tokio::test]
async fn create_download_task_persist_failure_marks_operation_failed_and_removes_memory_state() {
    let mock = MockAria2Server::spawn().await;
    let fixture = ServiceFixture::new(Vec::new(), false);
    let save_dir = temp_dir("service-create-persist-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    fixture.repository.fail_persist_on_call(1);

    let error = fixture
        .service()
        .create_download_task(
            &test_config(mock.addr.port(), "secret"),
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
        .expect_err("persistence failure should reject creation");

    assert!(error.contains("injected persist failure"));
    assert!(fixture.tasks.list().expect("tasks should list").is_empty());
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "task_persist_failed");

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
    assert_eq!(fixture.repository.persisted_tasks().len(), 1);
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Create);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert!(operations[0]
        .context
        .completed_side_effects
        .contains(&"restore_metadata_saved".to_string()));

    mock.abort();
}

#[tokio::test]
async fn pause_download_task_rejects_when_the_same_task_is_operating() {
    let save_dir = temp_dir("service-operation-conflict");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Active,
            "gid-1",
            save_dir.display().to_string(),
        )],
        false,
    );
    let _operation = fixture
        .tasks
        .begin_operation(1)
        .expect("test operation should lock task");

    let error = fixture
        .service()
        .pause_download_task(&test_config(6800, ""), 1)
        .await
        .expect_err("same task operation should reject before Aria2 call");

    assert!(error.contains("已有操作正在进行"));
    assert!(fixture.repository.persisted_tasks().is_empty());
}

#[tokio::test]
async fn pause_and_resume_record_completed_operation_states() {
    let mock = MockAria2Server::spawn_with_tell_status().await;
    let save_dir = temp_dir("service-pause-resume-operation");
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

    let paused = fixture
        .service()
        .pause_download_task(&config, 1)
        .await
        .expect("task should pause");
    assert_eq!(paused.status, DownloadTaskStatus::Paused);
    let resumed = fixture
        .service()
        .resume_download_task(&config, 1)
        .await
        .expect("task should resume");
    assert_eq!(resumed.status, DownloadTaskStatus::Active);
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 2);
    assert_eq!(operations[0].operation_type, TaskOperationType::Pause);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(operations[1].operation_type, TaskOperationType::Resume);
    assert_eq!(operations[1].status, TaskOperationStatus::Completed);

    mock.abort();
}

#[tokio::test]
async fn pause_persist_failure_restores_the_original_task_state() {
    let mock = MockAria2Server::spawn_with_tell_status().await;
    let save_dir = temp_dir("service-pause-persist-failure");
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
    fixture.repository.fail_persist_on_call(1);

    let error = fixture
        .service()
        .pause_download_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("persistence failure should roll back the pause");

    assert!(error.contains("injected persist failure"));
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Active);
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "task_persist_failed");

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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Delete);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    let tasks = fixture.tasks.list().expect("tasks should list");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].status, DownloadTaskStatus::Removed);

    mock.abort();
}

#[tokio::test]
async fn delete_with_files_commits_staged_files_after_task_state_persists() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("service-delete-files");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"payload").expect("file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Active,
            "gid-1",
            save_dir.display().to_string(),
        )],
        false,
    );

    let task = fixture
        .service()
        .delete_download_task(&test_config(mock.addr.port(), "secret"), 1, true)
        .await
        .expect("task and files should delete");

    assert_eq!(task.status, DownloadTaskStatus::Removed);
    assert!(task.files_deleted);
    assert!(!file_path.exists());
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert!(operations[0]
        .context
        .completed_side_effects
        .contains(&"task_files_deleted".to_string()));

    mock.abort();
}

#[tokio::test]
async fn delete_with_files_aria2_failure_keeps_the_original_file() {
    let save_dir = temp_dir("service-delete-files-aria2-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"payload").expect("file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Active,
            "gid-1",
            save_dir.display().to_string(),
        )],
        false,
    );

    let error = fixture
        .service()
        .delete_download_task(&test_config(1, "secret"), 1, true)
        .await
        .expect_err("Aria2 failure should reject deletion before staging files");

    assert!(error.contains("无法连接 Aria2 RPC"));
    assert_eq!(
        std::fs::read(&file_path).expect("file should remain"),
        b"payload"
    );
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Active);
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "aria2_remove_failed");
}

#[tokio::test]
async fn delete_with_files_persist_failure_restores_the_original_file() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("service-delete-files-persist-failure");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("archive.zip");
    std::fs::write(&file_path, b"payload").expect("file should write");
    let fixture = ServiceFixture::new(
        vec![sample_task(
            1,
            DownloadTaskStatus::Active,
            "gid-1",
            save_dir.display().to_string(),
        )],
        false,
    );
    fixture.repository.fail_persist_on_call(1);

    let error = fixture
        .service()
        .delete_download_task(&test_config(mock.addr.port(), "secret"), 1, true)
        .await
        .expect_err("persistence failure should restore staged files");

    assert!(error.contains("injected persist failure"));
    assert_eq!(
        std::fs::read(&file_path).expect("file should restore"),
        b"payload"
    );
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.gid.as_deref(), Some("gid-1"));
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::ManualReview);
    assert_eq!(operations[0].phase, "task_remove_needs_reconcile");

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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(
        operations[0].operation_type,
        TaskOperationType::PermanentDelete
    );
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
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
        aria2_rpc: crate::aria2::Aria2RpcClient::new(),
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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Confirm);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(
        operations[0].context.new_gid.as_deref(),
        Some("gid-torrent")
    );

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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Restore);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(
        operations[0].context.new_gid.as_deref(),
        Some("gid-created")
    );

    mock.abort();
}

#[tokio::test]
async fn restore_persist_failure_keeps_the_task_in_the_recycle_bin() {
    let mock = MockAria2Server::spawn().await;
    let task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        temp_dir("restore-persist-failure").display().to_string(),
    );
    let fixture = ServiceFixture::new(vec![task], false);
    fixture.repository.fail_persist_on_call(1);

    let error = fixture
        .service()
        .restore_removed_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("persistence failure should restore the recycle-bin state");

    assert!(error.contains("injected persist failure"));
    let task = &fixture.tasks.list().expect("tasks should list")[0];
    assert_eq!(task.status, DownloadTaskStatus::Removed);
    assert_eq!(task.gid.as_deref(), Some("old-gid"));
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "task_persist_failed");

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
async fn restore_removed_torrent_without_metadata_preserves_user_files() {
    let mock = MockAria2Server::spawn().await;
    let save_dir = temp_dir("restore-torrent-missing-file");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let user_file = save_dir.join("payload.bin");
    std::fs::write(&user_file, b"payload").expect("user file should write");
    let mut task = sample_task(
        1,
        DownloadTaskStatus::Removed,
        "old-gid",
        save_dir.display().to_string(),
    );
    task.source_type = DownloadTaskSourceType::Torrent;
    task.url = "torrent:missing.torrent".to_string();
    task.file_path = Some(user_file.display().to_string());
    let fixture = ServiceFixture::new(vec![task], false);

    let error = fixture
        .service()
        .restore_removed_task(&test_config(mock.addr.port(), "secret"), 1)
        .await
        .expect_err("missing torrent metadata should reject restore");

    assert!(error.contains("缺少可恢复的源 metadata"));
    assert_eq!(
        fixture.tasks.list().expect("tasks should list")[0].status,
        DownloadTaskStatus::Removed
    );
    assert!(user_file.is_file());
    assert_eq!(
        std::fs::read(&user_file).expect("user file should remain readable"),
        b"payload"
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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].operation_type, TaskOperationType::Redownload);
    assert_eq!(operations[0].status, TaskOperationStatus::Completed);
    assert_eq!(
        operations[0].context.new_gid.as_deref(),
        Some("gid-created")
    );
    assert!(operations[0]
        .context
        .completed_side_effects
        .contains(&"old_files_cleaned".to_string()));

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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "aria2_failed");
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
    let operations = fixture.repository.operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].status, TaskOperationStatus::Failed);
    assert_eq!(operations[0].phase, "rolled_back");

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

#[tokio::test]
async fn task_lifecycle_regression_preserves_operation_boundaries() {
    let save_dir = temp_dir("lifecycle-regression");
    std::fs::create_dir_all(&save_dir).expect("save dir should create");
    let mock = MockAria2Server::spawn_with_tell_status_dir(save_dir.display().to_string()).await;
    let fixture = ServiceFixture::new(Vec::new(), false);
    let config = test_config(mock.addr.port(), "secret");

    let created = fixture
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
    assert_eq!(created.status, DownloadTaskStatus::Pending);
    assert_eq!(created.gid.as_deref(), Some("gid-created"));

    let paused = fixture
        .service()
        .pause_download_task(&config, created.id)
        .await
        .expect("task should pause");
    assert_eq!(paused.status, DownloadTaskStatus::Paused);

    let resumed = fixture
        .service()
        .resume_download_task(&config, created.id)
        .await
        .expect("task should resume");
    assert_eq!(resumed.status, DownloadTaskStatus::Active);

    let removed = fixture
        .service()
        .delete_download_task(&config, created.id, false)
        .await
        .expect("task should enter recycle bin");
    assert_eq!(removed.status, DownloadTaskStatus::Removed);

    let restored = fixture
        .service()
        .restore_removed_task(&config, created.id)
        .await
        .expect("task should restore paused");
    assert_eq!(restored.status, DownloadTaskStatus::Paused);
    assert_eq!(restored.gid.as_deref(), Some("gid-created"));

    fixture
        .service()
        .delete_download_task(&config, created.id, false)
        .await
        .expect("restored task should enter recycle bin again");
    fixture
        .service()
        .permanently_delete_removed_task(created.id)
        .await
        .expect("removed task should permanently delete");

    assert!(fixture.tasks.list().expect("tasks should list").is_empty());
    assert_eq!(fixture.repository.deleted_task_ids(), vec![created.id]);
    let operations = fixture.repository.operations();
    assert_eq!(
        operations
            .iter()
            .map(|operation| operation.operation_type)
            .collect::<Vec<_>>(),
        vec![
            TaskOperationType::Create,
            TaskOperationType::Pause,
            TaskOperationType::Resume,
            TaskOperationType::Delete,
            TaskOperationType::Restore,
            TaskOperationType::Delete,
            TaskOperationType::PermanentDelete,
        ]
    );
    assert!(operations
        .iter()
        .all(|operation| operation.status == TaskOperationStatus::Completed));

    mock.abort();
}

struct ServiceFixture {
    repository: Arc<FakeTaskRepository>,
    tasks: TaskMemoryState,
    next_task_id: AtomicU64,
    debug_logs: DebugLogStore,
    aria2_rpc: crate::aria2::Aria2RpcClient,
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
            aria2_rpc: crate::aria2::Aria2RpcClient::new(),
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
            &self.aria2_rpc,
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
    operations: Vec<TaskOperation>,
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

    fn operations(&self) -> Vec<TaskOperation> {
        self.state
            .lock()
            .expect("repository state should lock")
            .operations
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

    async fn begin_operation(&self, operation: &TaskOperation) -> Result<(), String> {
        self.state
            .lock()
            .expect("repository state should lock")
            .operations
            .push(operation.clone());
        Ok(())
    }

    async fn update_operation(&self, operation: &TaskOperation) -> Result<(), String> {
        let mut state = self.state.lock().expect("repository state should lock");
        let stored = state
            .operations
            .iter_mut()
            .find(|stored| stored.id == operation.id)
            .ok_or_else(|| format!("测试任务操作不存在：{}", operation.id))?;
        *stored = operation.clone();
        Ok(())
    }

    async fn persist_task_state_with_operation(
        &self,
        task: &DownloadTask,
        operation: &TaskOperation,
    ) -> Result<(), String> {
        self.persist_task_state(task).await?;
        self.update_operation(operation).await
    }

    async fn list_unfinished_operations(&self) -> Result<Vec<TaskOperation>, String> {
        Ok(Vec::new())
    }

    async fn delete_task_record_with_operation(
        &self,
        task_id: u64,
        operation: &TaskOperation,
    ) -> Result<bool, String> {
        let mut guard = self.state.lock().expect("repository state should lock");
        guard.deleted_task_ids.push(task_id);
        let stored = guard
            .operations
            .iter_mut()
            .find(|stored| stored.id == operation.id)
            .ok_or_else(|| format!("测试任务操作不存在：{}", operation.id))?;
        *stored = operation.clone();
        Ok(guard.delete_result || !guard.deleted_task_ids.is_empty())
    }
}

struct MockAria2Server {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
}

struct TimeoutAfterAddAria2Server {
    addr: SocketAddr,
    handle: tokio::task::JoinHandle<()>,
    add_request_ids: Arc<Mutex<Vec<String>>>,
}

impl TimeoutAfterAddAria2Server {
    async fn spawn() -> Self {
        let add_request_ids = Arc::new(Mutex::new(Vec::new()));
        let task = Arc::new(Mutex::new(None));
        let request_ids = add_request_ids.clone();
        let task_state = task.clone();
        let app = Router::new().route(
            "/jsonrpc",
            post(move |Json(payload): Json<Value>| {
                let request_ids = request_ids.clone();
                let task_state = task_state.clone();
                async move {
                    let method = payload
                        .get("method")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if method == "aria2.addUri" {
                        let request_id = payload
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        request_ids
                            .lock()
                            .expect("request IDs should lock")
                            .push(request_id);
                        let params = payload
                            .get("params")
                            .and_then(Value::as_array)
                            .expect("addUri params should be present");
                        let url = params
                            .iter()
                            .find_map(Value::as_array)
                            .and_then(|urls| urls.first())
                            .and_then(Value::as_str)
                            .expect("URL should be present")
                            .to_string();
                        let save_dir = params
                            .iter()
                            .find_map(Value::as_object)
                            .and_then(|options| options.get("dir"))
                            .and_then(Value::as_str)
                            .expect("save dir should be present")
                            .to_string();
                        *task_state.lock().expect("task state should lock") = Some((url, save_dir));
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        return Json(json!({ "result": "gid-timeout" }));
                    }

                    if matches!(
                        method,
                        "aria2.tellActive" | "aria2.tellWaiting" | "aria2.tellStopped"
                    ) {
                        let result = if method == "aria2.tellActive" {
                            task_state
                                .lock()
                                .expect("task state should lock")
                                .as_ref()
                                .map(|(url, save_dir)| {
                                    vec![json!({
                                        "gid": "gid-timeout",
                                        "status": "waiting",
                                        "totalLength": "0",
                                        "completedLength": "0",
                                        "downloadSpeed": "0",
                                        "dir": save_dir,
                                        "files": [{
                                            "index": "1",
                                            "path": format!("{save_dir}/archive.zip"),
                                            "uris": [{ "uri": url }]
                                        }]
                                    })]
                                })
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        return Json(json!({ "result": result }));
                    }

                    Json(json!({ "error": { "message": format!("unexpected method: {method}") } }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let addr = listener.local_addr().expect("local addr should exist");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock server should serve");
        });
        Self {
            addr,
            handle,
            add_request_ids,
        }
    }

    fn add_request_ids(&self) -> Vec<String> {
        self.add_request_ids
            .lock()
            .expect("request IDs should lock")
            .clone()
    }

    fn abort(self) {
        self.handle.abort();
    }
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

    async fn spawn_with_tell_status() -> Self {
        let app = Router::new().route("/jsonrpc", post(mock_aria2_rpc_with_tell_status));
        Self::spawn_with_router(app).await
    }

    async fn spawn_with_tell_status_dir(save_dir: String) -> Self {
        let app = Router::new().route(
            "/jsonrpc",
            post(move |Json(payload): Json<Value>| {
                let save_dir = save_dir.clone();
                async move {
                    Json(mock_aria2_response_with_tell_status_dir(
                        &payload, &save_dir,
                    ))
                }
            }),
        );
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

async fn mock_aria2_rpc_with_tell_status(Json(payload): Json<Value>) -> Json<Value> {
    Json(mock_aria2_response_with_tell_status(&payload))
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

fn mock_aria2_response_with_tell_status(payload: &Value) -> Value {
    mock_aria2_response_with_tell_status_dir(payload, "/downloads")
}

fn mock_aria2_response_with_tell_status_dir(payload: &Value, save_dir: &str) -> Value {
    if payload.get("method").and_then(Value::as_str) == Some("aria2.tellStatus") {
        let gid = payload
            .get("params")
            .and_then(Value::as_array)
            .and_then(|params| {
                params
                    .iter()
                    .find_map(|value| value.as_str().filter(|value| !value.starts_with("token:")))
            })
            .unwrap_or("gid-created");
        return json!({
            "result": {
                "gid": gid,
                "status": "paused",
                "totalLength": "1024",
                "completedLength": "256",
                "downloadSpeed": "0",
                "dir": save_dir,
                "files": []
            }
        });
    }

    mock_aria2_response(payload)
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
