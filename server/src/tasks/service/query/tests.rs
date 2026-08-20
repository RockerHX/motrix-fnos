use super::{find_download_task, removed_tasks, visible_tasks};
use crate::tasks::service::tests::sample_task;
use crate::tasks::DownloadTaskStatus;

#[test]
fn task_filters_split_visible_and_removed_tasks() {
    let visible = sample_task(1, DownloadTaskStatus::Active, "gid-1", "/tmp".to_string());
    let removed = sample_task(2, DownloadTaskStatus::Removed, "gid-2", "/tmp".to_string());

    assert_eq!(
        visible_tasks(vec![visible.clone(), removed.clone()]),
        vec![visible]
    );
    assert_eq!(removed_tasks(vec![removed.clone()]), vec![removed]);
}

#[test]
fn task_lookup_includes_visible_and_removed_tasks() {
    let visible = sample_task(1, DownloadTaskStatus::Complete, "gid-1", "/tmp".to_string());
    let removed = sample_task(2, DownloadTaskStatus::Removed, "gid-2", "/tmp".to_string());
    let tasks = vec![visible.clone(), removed.clone()];

    assert_eq!(find_download_task(tasks.clone(), visible.id), Some(visible));
    assert_eq!(find_download_task(tasks.clone(), removed.id), Some(removed));
    assert_eq!(find_download_task(tasks, 999), None);
}
