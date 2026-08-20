use super::*;
use crate::tasks::{DownloadTaskFile, TaskProxyBinding};
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_ID: AtomicU64 = AtomicU64::new(1);

fn test_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "motrix-file-context-{label}-{}-{}",
        std::process::id(),
        TEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory should exist");
    path
}

fn task(source_type: DownloadTaskSourceType, save_dir: &Path) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/file.bin".to_string(),
        source_type,
        file_name: "file.bin".to_string(),
        save_dir: save_dir.display().to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status: DownloadTaskStatus::Complete,
        total_length: 1,
        completed_length: 1,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: None,
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::<DownloadTaskFile>::new(),
        created_at: 1,
        updated_at: 1,
    }
}

#[test]
fn completed_url_task_exposes_only_a_real_authorized_regular_file() {
    let root = test_dir("url");
    let file = root.join("file.bin");
    std::fs::write(&file, b"data").expect("file should write");
    let mut task = task(DownloadTaskSourceType::Url, &root);
    task.file_path = Some(file.display().to_string());

    let actions = task_file_actions(&task, &[root.display().to_string()]);

    assert_eq!(actions.availability, TaskFileAvailability::Available);
    assert_eq!(actions.open_file_path, Some(file.display().to_string()));
    assert_eq!(actions.file_manager_path, actions.open_file_path);
    assert_eq!(actions.detail_paths, vec![file.display().to_string()]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn completed_bt_tasks_require_an_explicit_owned_non_symlink_directory() {
    let root = test_dir("bt");
    let owned = root.join("task-owned");
    std::fs::create_dir(&owned).expect("owned directory should exist");
    for source_type in [
        DownloadTaskSourceType::Torrent,
        DownloadTaskSourceType::Magnet,
    ] {
        let mut task = task(source_type, &owned);
        assert_eq!(
            task_file_actions(&task, &[root.display().to_string()]).availability,
            TaskFileAvailability::UnsupportedLayout
        );
        task.owned_task_dir = Some(owned.display().to_string());
        let actions = task_file_actions(&task, &[root.display().to_string()]);
        assert_eq!(actions.availability, TaskFileAvailability::Available);
        assert_eq!(actions.file_manager_path, Some(owned.display().to_string()));
        assert_eq!(actions.open_file_path, None);
        assert_eq!(actions.detail_paths, vec![owned.display().to_string()]);
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn task_state_and_files_deleted_fail_before_path_checks() {
    let root = test_dir("state");
    let mut task = task(DownloadTaskSourceType::Url, &root);
    task.status = DownloadTaskStatus::Paused;
    assert_eq!(
        task_file_actions(&task, &[]).availability,
        TaskFileAvailability::TaskNotComplete
    );
    task.status = DownloadTaskStatus::Complete;
    task.files_deleted = true;
    let actions = task_file_actions(&task, &[]);
    assert_eq!(actions.availability, TaskFileAvailability::FilesDeleted);
    assert!(actions.file_manager_path.is_none());
    assert!(actions.open_file_path.is_none());
    assert!(actions.detail_paths.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_revoked_and_wrong_layout_targets_fail_closed() {
    let root = test_dir("fail-closed");
    let file = root.join("file.bin");
    std::fs::write(&file, b"data").expect("file should write");
    let mut task = task(DownloadTaskSourceType::Url, &root);
    task.file_path = Some(file.display().to_string());

    assert_eq!(
        task_file_actions(&task, &[]).availability,
        TaskFileAvailability::PathUnauthorized
    );
    std::fs::remove_file(&file).expect("file should remove");
    assert_eq!(
        task_file_actions(&task, &[root.display().to_string()]).availability,
        TaskFileAvailability::PathMissing
    );
    task.file_path = Some(root.display().to_string());
    assert_eq!(
        task_file_actions(&task, &[root.display().to_string()]).availability,
        TaskFileAvailability::UnsupportedLayout
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn symlinks_and_canonical_directory_escape_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = test_dir("escape-root");
    let outside = test_dir("escape-outside");
    let outside_file = outside.join("outside.bin");
    std::fs::write(&outside_file, b"data").expect("outside file should write");
    let direct_link = root.join("direct-link.bin");
    symlink(&outside_file, &direct_link).expect("file symlink should create");
    let mut url_task = task(DownloadTaskSourceType::Url, &root);
    url_task.file_path = Some(direct_link.display().to_string());
    assert_eq!(
        task_file_actions(&url_task, &[root.display().to_string()]).availability,
        TaskFileAvailability::UnsupportedLayout
    );

    let directory_link = root.join("linked-directory");
    symlink(&outside, &directory_link).expect("directory symlink should create");
    url_task.file_path = Some(directory_link.join("outside.bin").display().to_string());
    assert_eq!(
        task_file_actions(&url_task, &[root.display().to_string()]).availability,
        TaskFileAvailability::PathUnauthorized
    );

    let mut bt_task = task(DownloadTaskSourceType::Torrent, &root);
    bt_task.owned_task_dir = Some(directory_link.display().to_string());
    assert_eq!(
        task_file_actions(&bt_task, &[root.display().to_string()]).availability,
        TaskFileAvailability::UnsupportedLayout
    );
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}
