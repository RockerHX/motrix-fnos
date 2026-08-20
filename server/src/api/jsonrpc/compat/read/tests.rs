use super::page_range;
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus, TaskProxyBinding};

#[test]
fn page_range_supports_positive_and_negative_offsets() {
    assert_eq!(
        page_range(5, 1, 2).expect("positive offset should work"),
        1..3
    );
    assert_eq!(
        page_range(5, -2, 2).expect("negative offset should work"),
        3..5
    );
    assert_eq!(
        page_range(5, -20, 2).expect("large negative offset should clamp"),
        0..2
    );
}

#[test]
fn lane_filter_and_order_match_extension_semantics() {
    let mut waiting_old = sample_task(
        1,
        DownloadTaskStatus::Pending,
        "waiting-old",
        "/downloads".to_string(),
    );
    waiting_old.created_at = 10;
    let mut waiting_new = sample_task(
        2,
        DownloadTaskStatus::Paused,
        "waiting-new",
        "/downloads".to_string(),
    );
    waiting_new.created_at = 20;
    let mut stopped_old = sample_task(
        3,
        DownloadTaskStatus::Complete,
        "stopped-old",
        "/downloads".to_string(),
    );
    stopped_old.updated_at = 10;
    let mut stopped_new = sample_task(
        4,
        DownloadTaskStatus::Error,
        "stopped-new",
        "/downloads".to_string(),
    );
    stopped_new.updated_at = 20;

    let mut waiting = vec![waiting_new.clone(), waiting_old.clone()];
    waiting.retain(|task| super::TaskLane::Waiting.includes(task));
    waiting.sort_by(|left, right| super::TaskLane::Waiting.compare(left, right));
    assert_eq!(waiting, vec![waiting_old, waiting_new]);

    let mut stopped = vec![stopped_old.clone(), stopped_new.clone()];
    stopped.retain(|task| super::TaskLane::Stopped.includes(task));
    stopped.sort_by(|left, right| super::TaskLane::Stopped.compare(left, right));
    assert_eq!(stopped, vec![stopped_new, stopped_old]);
}

fn sample_task(id: u64, status: DownloadTaskStatus, gid: &str, save_dir: String) -> DownloadTask {
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
        file_path: Some(format!("{save_dir}/archive.zip")),
        use_proxy: false,
        proxy_binding: TaskProxyBinding::default(),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 1,
    }
}
