use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_DIR_ID: AtomicU64 = AtomicU64::new(1);

fn test_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-shared-access-{label}-{}-{}",
        std::process::id(),
        TEST_DIR_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).expect("test directory should exist");
    path
}

fn folders(paths: &[&str]) -> SharedAccessibleFolders {
    SharedAccessibleFolders {
        paths: paths.iter().map(|path| (*path).to_string()).collect(),
        http_status: 200,
        business_code: 0,
    }
}

#[test]
fn validates_all_paths_and_preserves_order_while_deduplicating() {
    assert_eq!(
        validate_official_paths(vec![
            "/vol1/downloads".to_string(),
            "/vol2/My Downloads".to_string(),
            "/vol2/My  Downloads".to_string(),
            "/vol2/My Downloads".to_string(),
            "/vol1/downloads".to_string(),
        ]),
        Ok(vec![
            "/vol1/downloads".to_string(),
            "/vol2/My Downloads".to_string(),
            "/vol2/My  Downloads".to_string(),
        ])
    );
    assert_eq!(validate_official_paths(Vec::new()), Ok(Vec::new()));
}

#[test]
fn accepts_internal_spaces_without_rewriting_paths() {
    for path in [
        "/vol1/My Downloads",
        "/vol1/My  Downloads/nested folder",
        "/vol1/我的 下载/电影 文件",
        "/vol1/全角\u{3000}空格",
        "/vol1/不换行\u{00a0}空格",
        "/vol1/ spaced segment /downloads",
        "/vol1/WDC WD3200BEVT-22ZEST0/downloads",
    ] {
        assert_eq!(
            validate_official_paths(vec![path.to_string()]),
            Ok(vec![path.to_string()]),
            "path should be preserved: {path:?}"
        );
    }
}

#[test]
fn rejects_the_entire_official_result_when_any_path_is_unsafe() {
    for invalid in [
        "",
        "/",
        "///",
        "relative/path",
        " /vol1/downloads",
        "/vol1/downloads ",
        "\u{3000}/vol1/downloads",
        "/vol1/downloads\u{00a0}",
        "/vol1/path\tchild",
        "/vol1/path\nchild",
        "/vol1/path\rchild",
        "/vol1/path\u{000b}child",
        "/vol1/path\u{000c}child",
        "/vol1/path\u{001b}child",
        "/vol1/path\u{007f}child",
        "/vol1/path\u{0085}child",
        "/vol1/path\\child",
        "/vol1/path\0child",
        "/vol1/./child",
        "/vol1/../child",
    ] {
        assert_eq!(
            validate_official_paths(vec!["/vol1/valid path".to_string(), invalid.to_string()]),
            Err(AccessiblePathsRefreshError::InvalidPaths),
            "path should be rejected: {invalid:?}"
        );
    }
}

#[test]
fn atomically_replaces_snapshot_including_with_an_empty_list() {
    let directory = test_dir("replace");
    let snapshot = directory.join("accessible-paths.json");
    std::fs::write(&snapshot, r#"{"paths":["/vol1/old"]}"#).expect("old snapshot should write");

    let updated =
        validate_and_persist_query_result(&snapshot, Ok(folders(&["/vol1/new", "/vol2/media"])))
            .expect("new snapshot should persist");
    assert_eq!(updated.paths, vec!["/vol1/new", "/vol2/media"]);
    assert_eq!(
        crate::storage::load_accessible_paths(&snapshot).expect("snapshot should load"),
        updated.paths
    );

    validate_and_persist_query_result(&snapshot, Ok(folders(&[])))
        .expect("empty snapshot should persist");
    assert!(crate::storage::load_accessible_paths(&snapshot)
        .expect("empty snapshot should load")
        .is_empty());
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn spaced_paths_survive_snapshot_reload_and_prepare_the_download_directory() {
    let directory = test_dir("spaces");
    let download_root = directory.join("WDC WD3200BEVT-22ZEST0");
    std::fs::create_dir(&download_root).expect("download root should exist");
    let root_path = download_root.to_str().expect("path should be UTF-8");
    let snapshot = directory.join("accessible-paths.json");
    let updated = validate_and_persist_query_result(
        &snapshot,
        Ok(folders(&["/vol1/downloads", root_path, root_path])),
    )
    .expect("paths with spaces should persist");
    let paths = crate::storage::load_accessible_paths(&snapshot).expect("snapshot should load");
    assert_eq!(paths, vec!["/vol1/downloads", root_path]);
    assert_eq!(paths, updated.paths);
    assert!(crate::storage::validate_default_download_dir(root_path, &paths, &directory).is_ok());

    let save_dir = download_root.join("我的 下载").join("episode 01");
    let save_path = save_dir.to_str().expect("path should be UTF-8");
    assert!(crate::storage::validate_task_save_dir(Some(save_path), &paths).is_ok());
    crate::storage::prepare_task_save_dir(Some(save_path), &paths)
        .expect("download directory with spaces should be prepared");
    assert!(save_dir.is_dir());
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn query_and_validation_failures_leave_the_previous_snapshot_unchanged() {
    let directory = test_dir("preserve");
    let snapshot = directory.join("accessible-paths.json");
    let previous = r#"{"paths":["/vol1/old path"]}"#;
    std::fs::write(&snapshot, previous).expect("old snapshot should write");

    assert_eq!(
        validate_and_persist_query_result(&snapshot, Err(FnosApiError::Timeout)),
        Err(AccessiblePathsRefreshError::Fnos(FnosApiError::Timeout))
    );
    assert_eq!(
        std::fs::read_to_string(&snapshot).expect("snapshot should remain"),
        previous
    );
    for invalid in [
        "/vol1/../unsafe",
        "/vol1/trailing space ",
        "/vol1/line\nbreak",
    ] {
        assert_eq!(
            validate_and_persist_query_result(&snapshot, Ok(folders(&["/vol1/new path", invalid]))),
            Err(AccessiblePathsRefreshError::InvalidPaths),
            "invalid path should reject the whole refresh: {invalid:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&snapshot).expect("snapshot should remain"),
            previous
        );
    }
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn persistence_failure_cleans_up_temporary_files_without_replacing_target() {
    let directory = test_dir("persist-failure");
    let snapshot = directory.join("accessible-paths.json");
    std::fs::create_dir(&snapshot).expect("target directory should exist");

    assert_eq!(
        persist_accessible_paths_atomic(&snapshot, &["/vol1/new".to_string()]),
        Err(AccessiblePathsRefreshError::Persist)
    );
    assert!(snapshot.is_dir());
    assert_eq!(
        std::fs::read_dir(&directory)
            .expect("test directory should read")
            .count(),
        1
    );
    let _ = std::fs::remove_dir_all(directory);
}
