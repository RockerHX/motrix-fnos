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
            "/vol2/media".to_string(),
            "/vol1/downloads".to_string(),
        ]),
        Ok(vec![
            "/vol1/downloads".to_string(),
            "/vol2/media".to_string(),
        ])
    );
    assert_eq!(validate_official_paths(Vec::new()), Ok(Vec::new()));
}

#[test]
fn rejects_the_entire_official_result_when_any_path_is_unsafe() {
    for invalid in [
        "",
        "/",
        "relative/path",
        "/vol1/path with space",
        "/vol1/path\\child",
        "/vol1/path\0child",
        "/vol1/./child",
        "/vol1/../child",
    ] {
        assert_eq!(
            validate_official_paths(vec!["/vol1/valid".to_string(), invalid.to_string()]),
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
fn query_and_validation_failures_leave_the_previous_snapshot_unchanged() {
    let directory = test_dir("preserve");
    let snapshot = directory.join("accessible-paths.json");
    let previous = r#"{"paths":["/vol1/old"]}"#;
    std::fs::write(&snapshot, previous).expect("old snapshot should write");

    assert_eq!(
        validate_and_persist_query_result(&snapshot, Err(FnosApiError::Timeout)),
        Err(AccessiblePathsRefreshError::Fnos(FnosApiError::Timeout))
    );
    assert_eq!(
        validate_and_persist_query_result(&snapshot, Ok(folders(&["/vol1/../unsafe"]))),
        Err(AccessiblePathsRefreshError::InvalidPaths)
    );
    assert_eq!(
        std::fs::read_to_string(&snapshot).expect("snapshot should remain"),
        previous
    );
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
