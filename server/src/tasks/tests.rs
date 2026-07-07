use super::*;
use crate::tasks::aria2_rpc::{
    build_add_torrent_request, build_add_uri_request, build_gid_control_request,
    build_tell_many_request, build_tell_status_request,
};
use crate::tasks::files::delete_file_candidates;
use crate::tasks::prepare::{default_download_dir, expand_home_dir, resolve_save_dir_with_logs};
use crate::tasks::progress::normalize_aria2_error_code;
use crate::tasks::session::find_matching_sqlite_task;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;

fn test_config() -> Aria2Config {
    Aria2Config {
        aria2_path: None,
        binary_source: crate::config::aria2::Aria2BinarySource::Sidecar,
        sidecar_name: "aria2-next".to_string(),
        target_triple: "test-target".to_string(),
        rpc_host: "127.0.0.1".to_string(),
        rpc_port: 6800,
        rpc_secret: String::new(),
        session_path: None,
        log_path: None,
    }
}

fn temp_download_dir(name: &str) -> String {
    let dir = env::temp_dir().join(format!(
        "motrix-fnos-test-{}-{}",
        name,
        current_timestamp_ms()
    ));
    dir.display().to_string()
}

fn sample_task(file_path: Option<String>, save_dir: String) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/file.zip".to_string(),
        file_name: "file.zip".to_string(),
        save_dir,
        category: "默认".to_string(),
        gid: Some("abc123".to_string()),
        status: DownloadTaskStatus::Active,
        total_length: 100,
        completed_length: 40,
        download_speed: 20,
        error_code: Some("old".to_string()),
        error_message: Some("old".to_string()),
        file_path,
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn prepare_task_accepts_https_url() {
    let task = prepare_task(CreateDownloadTaskRequest {
        url: " https://example.com/file.zip?token=1 ".to_string(),
        file_name: None,
        save_dir: Some(format!(" {} ", temp_download_dir("prepare"))),
        source_type: DownloadTaskSourceType::Url,
        start_mode: DownloadTaskStartMode::Now,
        category: None,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
    })
    .expect("https task should be prepared");

    assert_eq!(task.url, "https://example.com/file.zip?token=1");
    assert_eq!(task.file_name, "file.zip");
    assert!(Path::new(&task.save_dir).is_dir());
}

#[test]
fn prepare_task_rejects_non_http_url() {
    let error = prepare_task(CreateDownloadTaskRequest {
        url: "magnet:?xt=urn:btih:test".to_string(),
        file_name: None,
        save_dir: None,
        source_type: DownloadTaskSourceType::Url,
        start_mode: DownloadTaskStartMode::Now,
        category: None,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
    })
    .expect_err("non-http url should fail");

    assert!(error.contains("HTTP / HTTPS"));
}

#[test]
fn prepare_task_accepts_magnet_url_when_source_type_is_magnet() {
    let task = prepare_task(CreateDownloadTaskRequest {
        url: " magnet:?xt=urn:btih:test ".to_string(),
        file_name: None,
        save_dir: Some(temp_download_dir("prepare-magnet")),
        source_type: DownloadTaskSourceType::Magnet,
        start_mode: DownloadTaskStartMode::Now,
        category: None,
        advanced_options: CreateTaskAdvancedOptions::default(),
        aria2_options: serde_json::Map::new(),
    })
    .expect("magnet task should be prepared");

    assert_eq!(task.url, "magnet:?xt=urn:btih:test");
    assert_eq!(task.file_name, "磁力链接任务");
    assert_eq!(task.source_type, DownloadTaskSourceType::Magnet);
}

#[test]
fn store_created_task_persists_gid() {
    let tasks = TaskMemoryState::new(Vec::new());
    let next_id = AtomicU64::new(1);
    let task = store_created_task(
        &tasks,
        &next_id,
        PreparedDownloadTask {
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            category: "默认".to_string(),
            source_type: DownloadTaskSourceType::Url,
            start_mode: DownloadTaskStartMode::Now,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        },
        "abc123".to_string(),
    )
    .expect("task should be stored");

    assert_eq!(task.id, 1);
    assert_eq!(task.gid.as_deref(), Some("abc123"));
    assert_eq!(
        list_tasks(&tasks).expect("tasks should be readable").len(),
        1
    );
}

#[test]
fn store_created_task_preserves_paused_start_mode() {
    let tasks = TaskMemoryState::new(Vec::new());
    let next_id = AtomicU64::new(1);
    let task = store_created_task(
        &tasks,
        &next_id,
        PreparedDownloadTask {
            url: "https://example.com/file.zip".to_string(),
            file_name: "file.zip".to_string(),
            save_dir: "/downloads".to_string(),
            category: "默认".to_string(),
            source_type: DownloadTaskSourceType::Url,
            start_mode: DownloadTaskStartMode::Paused,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        },
        "abc123".to_string(),
    )
    .expect("task should be stored");

    assert_eq!(task.status, DownloadTaskStatus::Paused);
}

#[test]
fn task_gid_rejects_removed_task() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Removed;
    let tasks = TaskMemoryState::new(vec![task]);

    let error = task_gid(&tasks, 1).expect_err("removed task should be rejected");

    assert!(error.contains("已删除"));
}

#[test]
fn startup_force_pause_scope_matches_exit_pause_scope() {
    let mut task = sample_task(None, "/downloads".to_string());

    task.status = DownloadTaskStatus::Pending;
    assert!(should_force_pause_task_on_startup(&task));

    task.status = DownloadTaskStatus::Active;
    assert!(should_force_pause_task_on_startup(&task));

    task.status = DownloadTaskStatus::Paused;
    assert!(!should_force_pause_task_on_startup(&task));

    task.status = DownloadTaskStatus::Complete;
    assert!(!should_force_pause_task_on_startup(&task));
}

#[test]
fn exit_pause_scope_only_includes_unfinished_tasks() {
    let mut task = sample_task(None, "/downloads".to_string());

    task.status = DownloadTaskStatus::Pending;
    assert!(should_pause_task_on_exit(&task));

    task.status = DownloadTaskStatus::Active;
    assert!(should_pause_task_on_exit(&task));

    task.status = DownloadTaskStatus::Paused;
    assert!(!should_pause_task_on_exit(&task));

    task.status = DownloadTaskStatus::Complete;
    assert!(!should_pause_task_on_exit(&task));

    task.status = DownloadTaskStatus::Error;
    assert!(!should_pause_task_on_exit(&task));

    task.status = DownloadTaskStatus::Removed;
    assert!(!should_pause_task_on_exit(&task));
}

#[test]
fn mark_task_paused_updates_status_and_speed() {
    let tasks = TaskMemoryState::new(vec![sample_task(None, "/downloads".to_string())]);

    let task = mark_task_paused(&tasks, 1).expect("task should be paused");

    assert_eq!(task.status, DownloadTaskStatus::Paused);
    assert_eq!(task.download_speed, 0);
    assert_eq!(task.error_message, None);
}

#[test]
fn mark_task_paused_by_gid_updates_matching_task() {
    let tasks = TaskMemoryState::new(vec![sample_task(None, "/downloads".to_string())]);

    let task = mark_task_paused_by_gid(&tasks, "abc123").expect("task should be paused");

    assert_eq!(task.status, DownloadTaskStatus::Paused);
    assert_eq!(task.download_speed, 0);
    assert_eq!(task.error_message, None);
}

#[test]
fn mark_task_resumed_updates_status() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Paused;
    let tasks = TaskMemoryState::new(vec![task]);

    let task = mark_task_resumed(&tasks, 1).expect("task should be resumed");

    assert_eq!(task.status, DownloadTaskStatus::Active);
}

#[test]
fn mark_task_redownloaded_resets_completed_task_progress() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Complete;
    task.total_length = 100;
    task.completed_length = 100;
    task.download_speed = 0;
    let tasks = TaskMemoryState::new(vec![task]);

    let task = mark_task_redownloaded(&tasks, 1, "new-gid".to_string())
        .expect("completed task should be redownloaded");

    assert_eq!(task.gid.as_deref(), Some("new-gid"));
    assert_eq!(task.status, DownloadTaskStatus::Pending);
    assert_eq!(task.total_length, 0);
    assert_eq!(task.completed_length, 0);
    assert_eq!(task.download_speed, 0);
    assert!(task.error_code.is_none());
    assert!(task.error_message.is_none());
    assert_eq!(task.file_path.as_deref(), Some("/downloads/file.zip"));
}

#[test]
fn mark_task_redownloaded_rejects_unfinished_task() {
    let task = sample_task(None, "/downloads".to_string());
    let tasks = TaskMemoryState::new(vec![task]);

    let error = mark_task_redownloaded(&tasks, 1, "new-gid".to_string())
        .expect_err("unfinished task should be rejected");

    assert!(error.contains("已完成任务"));
}

#[test]
fn delete_task_files_removes_completed_file_before_redownload() {
    let save_dir = PathBuf::from(temp_download_dir("redownload-delete"));
    fs::create_dir_all(&save_dir).expect("save dir should be created");
    let file_path = save_dir.join("file.zip");
    fs::write(&file_path, b"completed").expect("file should be written");
    let aria2_path = save_dir.join("file.zip.aria2");
    fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
    let mut task = sample_task(
        Some(file_path.display().to_string()),
        save_dir.display().to_string(),
    );
    task.status = DownloadTaskStatus::Complete;

    delete_task_files(&task).expect("files should delete");

    assert!(!file_path.exists());
    assert!(!aria2_path.exists());
}

fn session_status(gid: &str, url: &str, dir: &str, path: &str) -> Aria2TaskStatus {
    Aria2TaskStatus {
        gid: Some(gid.to_string()),
        status: "paused".to_string(),
        total_length: "100".to_string(),
        completed_length: "40".to_string(),
        download_speed: "0".to_string(),
        error_code: None,
        error_message: None,
        dir: Some(dir.to_string()),
        files: Some(vec![Aria2FileStatus {
            path: path.to_string(),
            uris: vec![Aria2UriStatus {
                uri: url.to_string(),
            }],
        }]),
    }
}

#[test]
fn session_task_matches_by_url_dir_and_file() {
    let task = sample_task(
        Some("/downloads/file.zip".to_string()),
        "/downloads".to_string(),
    );
    let session_task = session_status(
        "newgid",
        "https://example.com/file.zip",
        "/downloads",
        "/downloads/file.zip",
    );

    assert_eq!(find_matching_sqlite_task(&[task], &session_task), Some(0));
}

#[test]
fn session_task_does_not_match_unknown_url() {
    let task = sample_task(None, "/downloads".to_string());
    let session_task = session_status(
        "newgid",
        "https://example.com/other.zip",
        "/downloads",
        "/downloads/file.zip",
    );

    assert_eq!(find_matching_sqlite_task(&[task], &session_task), None);
}

#[test]
fn tell_many_request_uses_offsets_for_waiting_tasks() {
    let config = test_config();
    let request = build_tell_many_request(&config, "aria2.tellWaiting");

    assert_eq!(request["method"], "aria2.tellWaiting");
    assert_eq!(request["params"][0], 0);
    assert_eq!(request["params"][1], 1000);
}

#[test]
fn is_stale_aria2_gid_error_detects_unrecoverable_resume_errors() {
    assert!(is_stale_aria2_gid_error("No URI available"));
    assert!(is_stale_aria2_gid_error(
        "GID 6c4e6a308ea8d57e is not found"
    ));
    assert!(!is_stale_aria2_gid_error("GID#123 cannot be unpaused now"));
    assert!(!is_stale_aria2_gid_error("download failed"));
}

#[test]
fn resume_error_readds_when_gid_is_not_found() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Error;

    assert!(should_readd_task_after_resume_error(
        &task,
        "恢复任务失败：GID 6c4e6a308ea8d57e is not found"
    ));
}

#[test]
fn resume_error_readds_only_when_task_already_has_stale_gid_error() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Error;
    task.error_message = Some("No URI available.".to_string());

    assert!(should_readd_task_after_resume_error(
        &task,
        "GID#abc cannot be unpaused now"
    ));

    task.error_message = Some("download failed".to_string());
    assert!(should_readd_task_after_resume_error(
        &task,
        "GID#abc cannot be unpaused now"
    ));

    task.status = DownloadTaskStatus::Active;
    assert!(!should_readd_task_after_resume_error(
        &task,
        "GID#abc cannot be unpaused now"
    ));
}

#[test]
fn mark_task_removed_deletes_file_under_save_dir() {
    let save_dir = PathBuf::from(temp_download_dir("delete-file"));
    fs::create_dir_all(&save_dir).expect("save dir should be created");
    let file_path = save_dir.join("file.zip");
    fs::write(&file_path, b"test").expect("file should be written");
    let aria2_path = save_dir.join("file.zip.aria2");
    fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
    let tasks = TaskMemoryState::new(vec![sample_task(
        Some(file_path.display().to_string()),
        save_dir.display().to_string(),
    )]);

    let task = mark_task_removed(&tasks, 1, true).expect("task should be removed");

    assert_eq!(task.status, DownloadTaskStatus::Removed);
    assert!(!file_path.exists());
    assert!(!aria2_path.exists());
}

#[test]
fn mark_task_removed_deletes_orphan_aria2_control_file() {
    let save_dir = PathBuf::from(temp_download_dir("delete-orphan-aria2"));
    fs::create_dir_all(&save_dir).expect("save dir should be created");
    let file_path = save_dir.join("file.zip");
    let aria2_path = save_dir.join("file.zip.aria2");
    fs::write(&aria2_path, b"control").expect("aria2 control file should be written");
    let tasks = TaskMemoryState::new(vec![sample_task(
        Some(file_path.display().to_string()),
        save_dir.display().to_string(),
    )]);

    let task = mark_task_removed(&tasks, 1, true).expect("task should be removed");

    assert_eq!(task.status, DownloadTaskStatus::Removed);
    assert!(!aria2_path.exists());
}

#[test]
fn delete_file_candidates_include_aria2_control_file() {
    let candidates = delete_file_candidates(Path::new("/downloads/file.iso"));

    assert_eq!(candidates[0], PathBuf::from("/downloads/file.iso"));
    assert_eq!(candidates[1], PathBuf::from("/downloads/file.iso.aria2"));
}

#[test]
fn mark_task_removed_refuses_file_outside_save_dir() {
    let save_dir = PathBuf::from(temp_download_dir("safe-delete-save"));
    let outside_dir = PathBuf::from(temp_download_dir("safe-delete-outside"));
    fs::create_dir_all(&save_dir).expect("save dir should be created");
    fs::create_dir_all(&outside_dir).expect("outside dir should be created");
    let file_path = outside_dir.join("file.zip");
    fs::write(&file_path, b"test").expect("file should be written");
    let tasks = TaskMemoryState::new(vec![sample_task(
        Some(file_path.display().to_string()),
        save_dir.display().to_string(),
    )]);

    let error = mark_task_removed(&tasks, 1, true).expect_err("outside file should be rejected");

    assert!(error.contains("保存目录外"));
    assert!(file_path.exists());
}

#[test]
fn tell_status_request_contains_gid_and_fields() {
    let request = build_tell_status_request(&test_config(), "abc123");

    assert_eq!(request["method"], "aria2.tellStatus");
    assert_eq!(request["params"][0], "abc123");
    assert!(request["params"][1]
        .as_array()
        .expect("fields should be array")
        .contains(&serde_json::json!("downloadSpeed")));
}

#[test]
fn apply_aria2_status_updates_progress_fields() {
    let mut task = DownloadTask {
        id: 1,
        url: "https://example.com/file.zip".to_string(),
        file_name: "file.zip".to_string(),
        save_dir: "/downloads".to_string(),
        category: "默认".to_string(),
        gid: Some("abc123".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        created_at: 1,
        updated_at: 1,
    };

    apply_aria2_status(
        &mut task,
        &Aria2TaskStatus {
            gid: None,
            status: "active".to_string(),
            total_length: "100".to_string(),
            completed_length: "40".to_string(),
            download_speed: "20".to_string(),
            error_code: None,
            error_message: None,
            dir: Some("/downloads".to_string()),
            files: Some(vec![Aria2FileStatus {
                path: "/downloads/file.zip".to_string(),
                uris: Vec::new(),
            }]),
        },
    );

    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.total_length, 100);
    assert_eq!(task.completed_length, 40);
    assert_eq!(task.download_speed, 20);
    assert_eq!(task.file_path.as_deref(), Some("/downloads/file.zip"));
}

#[test]
fn apply_aria2_status_keeps_active_progress_non_decreasing() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.total_length = 100;
    task.completed_length = 65;

    apply_aria2_status(
        &mut task,
        &Aria2TaskStatus {
            gid: None,
            status: "active".to_string(),
            total_length: "100".to_string(),
            completed_length: "63".to_string(),
            download_speed: "20".to_string(),
            error_code: None,
            error_message: None,
            dir: None,
            files: None,
        },
    );

    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.total_length, 100);
    assert_eq!(task.completed_length, 65);
    assert_eq!(task.download_speed, 20);
}

#[test]
fn apply_aria2_status_removes_completed_control_file() {
    let save_dir = PathBuf::from(temp_download_dir("complete-cleanup"));
    fs::create_dir_all(&save_dir).expect("save dir should create");
    let file_path = save_dir.join("file.zip");
    let control_path = save_dir.join("file.zip.aria2");
    fs::write(&file_path, b"complete").expect("downloaded file should write");
    fs::write(&control_path, b"control").expect("control file should write");

    let mut task = sample_task(None, save_dir.display().to_string());
    apply_aria2_status(
        &mut task,
        &Aria2TaskStatus {
            gid: None,
            status: "complete".to_string(),
            total_length: "8".to_string(),
            completed_length: "8".to_string(),
            download_speed: "0".to_string(),
            error_code: None,
            error_message: None,
            dir: Some(save_dir.display().to_string()),
            files: Some(vec![Aria2FileStatus {
                path: file_path.display().to_string(),
                uris: Vec::new(),
            }]),
        },
    );

    assert!(file_path.exists());
    assert!(!control_path.exists());
}

#[test]
fn pause_status_settles_only_after_paused_progress_is_stable() {
    let active = Aria2TaskStatus {
        gid: Some("abc123".to_string()),
        status: "active".to_string(),
        total_length: "100".to_string(),
        completed_length: "80".to_string(),
        download_speed: "50".to_string(),
        error_code: None,
        error_message: None,
        dir: None,
        files: None,
    };
    let mut paused = active.clone();
    paused.status = "paused".to_string();
    paused.download_speed = "0".to_string();

    assert!(!pause_status_is_settled(&active, Some(80)));
    assert!(!pause_status_is_settled(&paused, None));
    assert!(!pause_status_is_settled(&paused, Some(79)));
    assert!(pause_status_is_settled(&paused, Some(80)));
}

#[test]
fn apply_aria2_status_by_gid_updates_progress_before_pause_state() {
    let tasks = TaskMemoryState::new(vec![sample_task(None, "/downloads".to_string())]);
    let status = Aria2TaskStatus {
        gid: Some("abc123".to_string()),
        status: "active".to_string(),
        total_length: "100".to_string(),
        completed_length: "80".to_string(),
        download_speed: "50".to_string(),
        error_code: None,
        error_message: None,
        dir: Some("/downloads".to_string()),
        files: None,
    };

    let synced =
        apply_aria2_status_by_gid(&tasks, "abc123", &status).expect("task progress should sync");
    assert_eq!(synced.completed_length, 80);

    let paused = mark_task_paused(&tasks, 1).expect("task should pause");
    assert_eq!(paused.status, DownloadTaskStatus::Paused);
    assert_eq!(paused.completed_length, 80);
    assert_eq!(paused.total_length, 100);
    assert_eq!(paused.download_speed, 0);
}

#[test]
fn apply_aria2_status_ignores_empty_error_code_zero() {
    let mut task = DownloadTask {
        id: 1,
        url: "https://example.com/file.zip".to_string(),
        file_name: "file.zip".to_string(),
        save_dir: "/downloads".to_string(),
        category: "默认".to_string(),
        gid: Some("abc123".to_string()),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: Some("old".to_string()),
        error_message: Some("old".to_string()),
        file_path: None,
        created_at: 1,
        updated_at: 1,
    };

    let status = Aria2TaskStatus {
        gid: None,
        status: "complete".to_string(),
        total_length: "100".to_string(),
        completed_length: "100".to_string(),
        download_speed: "0".to_string(),
        error_code: Some("0".to_string()),
        error_message: Some("".to_string()),
        dir: None,
        files: None,
    };

    assert!(!is_aria2_status_error(&status));
    apply_aria2_status(&mut task, &status);

    assert_eq!(task.status, DownloadTaskStatus::Complete);
    assert_eq!(task.error_code, None);
    assert_eq!(task.error_message, None);
}

#[test]
fn non_zero_aria2_error_code_is_error() {
    let status = Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: Some("3".to_string()),
        error_message: Some("Resource not found".to_string()),
        dir: None,
        files: None,
    };

    assert!(is_aria2_status_error(&status));
    assert_eq!(
        normalize_aria2_error_code(status.error_code.as_deref()).as_deref(),
        Some("3")
    );
}

#[test]
fn aria2_error_16_gets_readable_hint() {
    let mut task = sample_task(None, "/downloads".to_string());
    let status = Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: Some("16".to_string()),
        error_message: Some("Download aborted.".to_string()),
        dir: None,
        files: None,
    };

    apply_aria2_status(&mut task, &status);

    assert_eq!(task.error_code.as_deref(), Some("16"));
    assert!(task
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("无法创建或写入目标文件"));
}

#[test]
fn apply_aria2_status_preserves_progress_when_active_status_is_temporarily_empty() {
    let mut task = sample_task(None, "/downloads".to_string());
    task.status = DownloadTaskStatus::Paused;
    let status = Aria2TaskStatus {
        gid: None,
        status: "active".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: None,
        error_message: None,
        dir: None,
        files: None,
    };

    apply_aria2_status(&mut task, &status);

    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.total_length, 100);
    assert_eq!(task.completed_length, 40);
    assert_eq!(task.download_speed, 0);
    assert!(task.error_code.is_none());
    assert!(task.error_message.is_none());
}

#[test]
fn apply_aria2_status_preserves_progress_when_error_has_no_lengths() {
    let mut task = sample_task(None, "/downloads".to_string());
    let status = Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: Some("1".to_string()),
        error_message: Some(
            "SSL/TLS handshake failure: unable to get local issuer certificate".to_string(),
        ),
        dir: None,
        files: None,
    };

    apply_aria2_status(&mut task, &status);

    assert_eq!(task.status, DownloadTaskStatus::Error);
    assert_eq!(task.total_length, 100);
    assert_eq!(task.completed_length, 40);
    assert_eq!(task.download_speed, 0);
    assert_eq!(task.error_code.as_deref(), Some("1"));
    assert_eq!(
        task.error_message.as_deref(),
        Some("SSL/TLS handshake failure: unable to get local issuer certificate")
    );
}

#[test]
fn task_status_error_keeps_readable_message() {
    let status = task_status_error("同步任务状态失败：无法连接 Aria2 RPC".to_string());

    assert_eq!(status.status, "error");
    assert_eq!(
        status.error_message.as_deref(),
        Some("同步任务状态失败：无法连接 Aria2 RPC")
    );
}

#[test]
fn tell_status_request_contains_error_and_file_fields() {
    let request = build_tell_status_request(&test_config(), "abc123");
    let fields = request["params"][1]
        .as_array()
        .expect("fields should be array");

    assert!(fields.contains(&serde_json::json!("errorCode")));
    assert!(fields.contains(&serde_json::json!("errorMessage")));
    assert!(fields.contains(&serde_json::json!("dir")));
    assert!(fields.contains(&serde_json::json!("files")));
}

#[test]
fn expand_home_dir_supports_tilde_paths() {
    let expanded = expand_home_dir("~/Downloads").expect("home path should expand");

    assert!(expanded.ends_with("Downloads"));
    assert!(expanded.is_absolute());
}

#[test]
fn resolve_save_dir_creates_missing_directory() {
    let dir = temp_download_dir("missing-dir");
    let resolved =
        resolve_save_dir_with_logs(Some(dir.clone()), None).expect("directory should be created");

    assert_eq!(resolved, dir);
    assert!(Path::new(&resolved).is_dir());
}

#[test]
fn resolve_save_dir_rejects_file_path() {
    let dir = PathBuf::from(temp_download_dir("file-path"));
    fs::create_dir_all(&dir).expect("temp dir should create");
    let file = dir.join("not-a-dir");
    fs::write(&file, b"content").expect("temp file should create");

    let error = resolve_save_dir_with_logs(Some(file.display().to_string()), None)
        .expect_err("file path should be rejected");

    assert!(error.contains("创建下载目录失败"));
}

#[test]
fn default_download_dir_uses_downloads_under_home() {
    let dir = default_download_dir().expect("default download dir should resolve");

    assert!(dir.ends_with("Downloads"));
}

#[test]
fn add_uri_request_contains_url_and_options() {
    let request = build_add_uri_request(
        &test_config(),
        &PreparedDownloadTask {
            url: "https://example.com/file.zip".to_string(),
            file_name: "custom.zip".to_string(),
            save_dir: "/downloads".to_string(),
            category: "默认".to_string(),
            source_type: DownloadTaskSourceType::Url,
            start_mode: DownloadTaskStartMode::Now,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::from_iter([
                (
                    "split".to_string(),
                    serde_json::Value::String("64".to_string()),
                ),
                (
                    "max-connection-per-server".to_string(),
                    serde_json::Value::String("64".to_string()),
                ),
            ]),
        },
    );

    assert_eq!(request["method"], "aria2.addUri");
    assert_eq!(request["params"][0][0], "https://example.com/file.zip");
    assert_eq!(request["params"][1]["dir"], "/downloads");
    assert_eq!(request["params"][1]["out"], "custom.zip");
    assert_eq!(request["params"][1]["split"], "64");
    assert_eq!(request["params"][1]["max-connection-per-server"], "64");
}

#[test]
fn add_uri_request_sets_pause_options_for_paused_magnet() {
    let request = build_add_uri_request(
        &test_config(),
        &PreparedDownloadTask {
            url: "magnet:?xt=urn:btih:test".to_string(),
            file_name: "磁力链接任务".to_string(),
            save_dir: "/downloads".to_string(),
            category: "默认".to_string(),
            source_type: DownloadTaskSourceType::Magnet,
            start_mode: DownloadTaskStartMode::Paused,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        },
    );

    assert_eq!(request["method"], "aria2.addUri");
    assert_eq!(request["params"][0][0], "magnet:?xt=urn:btih:test");
    assert_eq!(request["params"][1]["pause"], "true");
    assert_eq!(request["params"][1]["pause-metadata"], "true");
    assert!(request["params"][1].get("out").is_none());
}

#[test]
fn add_torrent_request_contains_base64_payload_and_options() {
    let request = build_add_torrent_request(
        &test_config(),
        &PreparedDownloadTask {
            url: "torrent:example.torrent".to_string(),
            file_name: "example".to_string(),
            save_dir: "/downloads".to_string(),
            category: "默认".to_string(),
            source_type: DownloadTaskSourceType::Url,
            start_mode: DownloadTaskStartMode::Paused,
            advanced_options: CreateTaskAdvancedOptions::default(),
            aria2_options: serde_json::Map::new(),
        },
        b"torrent-bytes",
    );

    assert_eq!(request["method"], "aria2.addTorrent");
    assert_eq!(request["params"][0], "dG9ycmVudC1ieXRlcw==");
    assert_eq!(request["params"][1], serde_json::json!([]));
    assert_eq!(request["params"][2]["dir"], "/downloads");
    assert_eq!(request["params"][2]["pause"], "true");
    assert_eq!(request["params"][2]["pause-metadata"], "true");
}

#[test]
fn gid_control_request_contains_method_and_gid() {
    let request = build_gid_control_request(&test_config(), "abc123", "aria2.pause", "pause-test");

    assert_eq!(request["method"], "aria2.pause");
    assert_eq!(request["id"], "pause-test");
    assert_eq!(request["params"][0], "abc123");
}

#[test]
fn gid_control_request_includes_token_when_configured() {
    let mut config = test_config();
    config.rpc_secret = "secret".to_string();

    let request = build_gid_control_request(&config, "abc123", "aria2.unpause", "unpause-test");

    assert_eq!(request["params"][0], "token:secret");
    assert_eq!(request["params"][1], "abc123");
}

#[test]
fn stale_aria2_gid_error_is_detected() {
    assert!(is_stale_aria2_gid_error(
        "同步 Aria2 任务状态失败：No URI available."
    ));
    assert!(is_stale_aria2_gid_status(&Aria2TaskStatus {
        gid: None,
        status: "error".to_string(),
        total_length: "0".to_string(),
        completed_length: "0".to_string(),
        download_speed: "0".to_string(),
        error_code: Some("1".to_string()),
        error_message: Some("No URI available.".to_string()),
        dir: None,
        files: None,
    }));
    assert!(!is_stale_aria2_gid_error("连接失败"));
}

#[test]
fn readded_gid_updates_task_without_clearing_progress() {
    let save_dir = temp_download_dir("readded-gid");
    let mut task = sample_task(None, save_dir.clone());
    task.status = DownloadTaskStatus::Error;
    task.gid = Some("old-gid".to_string());
    task.error_code = Some("1".to_string());
    task.error_message = Some("No URI available.".to_string());

    apply_readded_gid(&mut task, "new-gid");

    assert_eq!(task.gid.as_deref(), Some("new-gid"));
    assert_eq!(task.status, DownloadTaskStatus::Active);
    assert_eq!(task.completed_length, 40);
    assert_eq!(task.total_length, 100);
    assert!(task.error_code.is_none());
    assert!(task.error_message.is_none());
    let expected_file_path = Path::new(&save_dir).join("file.zip").display().to_string();
    assert_eq!(task.file_path.as_deref(), Some(expected_file_path.as_str()));
}
