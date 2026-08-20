use super::find_download_task_by_gid;
use crate::tasks::service::tests::sample_task;
use crate::tasks::DownloadTaskStatus;

#[test]
fn gid_lookup_returns_the_unique_task() {
    let expected = sample_task(
        1,
        DownloadTaskStatus::Active,
        "gid-1",
        "/downloads".to_string(),
    );
    let other = sample_task(
        2,
        DownloadTaskStatus::Paused,
        "gid-2",
        "/downloads".to_string(),
    );

    assert_eq!(
        find_download_task_by_gid(vec![expected.clone(), other], " gid-1 ")
            .expect("GID lookup should succeed"),
        Some(expected)
    );
}

#[test]
fn gid_lookup_rejects_empty_and_duplicate_values() {
    let first = sample_task(
        1,
        DownloadTaskStatus::Active,
        "duplicate",
        "/downloads".to_string(),
    );
    let second = sample_task(
        2,
        DownloadTaskStatus::Paused,
        "duplicate",
        "/downloads".to_string(),
    );

    assert_eq!(
        find_download_task_by_gid(Vec::new(), " ").expect_err("empty GID must fail"),
        "Aria2 GID 不能为空"
    );
    assert!(find_download_task_by_gid(vec![first, second], "duplicate")
        .expect_err("duplicate GIDs must fail")
        .contains("存在多个使用同一 Aria2 GID 的任务"));
}
