use super::*;

#[test]
fn default_download_dir_prefers_data_authorized_path() {
    let paths = vec![
        "/vol1/tmp".to_string(),
        "/应用文件/motrix_fnos/data".to_string(),
        "/vol1/downloads".to_string(),
    ];

    assert_eq!(
        default_download_dir(&paths, Path::new("/fallback")),
        PathBuf::from("/应用文件/motrix_fnos/data")
    );
}

#[test]
fn default_download_dir_uses_first_authorized_path_when_data_missing() {
    let paths = vec!["/vol1/downloads".to_string(), "/vol1/tmp".to_string()];

    assert_eq!(
        default_download_dir(&paths, Path::new("/fallback")),
        PathBuf::from("/vol1/downloads")
    );
}

#[test]
fn default_download_dir_falls_back_to_app_data_dir_when_authorized_paths_empty() {
    assert_eq!(
        default_download_dir(&[], Path::new("/app/data")),
        PathBuf::from("/app/data")
    );
}

#[test]
fn load_accessible_paths_normalizes_file_values() {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-accessible-paths-test-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{"paths":[" /app/data ","/app/data","","/vol1/tmp"]}"#,
    )
    .expect("accessible paths should write");

    let paths = load_accessible_paths(&path).expect("paths should load");

    assert_eq!(paths, vec!["/app/data", "/vol1/tmp"]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn validate_default_download_dir_accepts_authorized_path() {
    let paths = vec!["/app/data".to_string()];

    assert!(validate_default_download_dir("/app/data", &paths, Path::new("/fallback")).is_ok());
}

#[test]
fn validate_default_download_dir_rejects_unauthorized_path() {
    let paths = vec!["/app/data".to_string()];

    let error = validate_default_download_dir("/tmp", &paths, Path::new("/fallback"))
        .expect_err("unauthorized path should fail");

    assert_eq!(error, "默认下载目录不在已授权目录列表中");
}

#[test]
fn validate_default_download_dir_allows_app_data_dir_without_authorized_paths() {
    assert!(validate_default_download_dir("/app/data", &[], Path::new("/app/data")).is_ok());
}

#[test]
fn validate_task_save_dir_requires_non_empty_path() {
    assert_eq!(
        validate_task_save_dir(Some("  "), &["/downloads".to_string()]),
        Err(TaskSaveDirError::Required)
    );
}

#[test]
fn validate_task_save_dir_requires_authorized_paths() {
    assert_eq!(
        validate_task_save_dir(Some("/downloads"), &[]),
        Err(TaskSaveDirError::NoAccessiblePaths)
    );
}

#[test]
fn validate_task_save_dir_accepts_authorized_root_and_descendants_only() {
    let paths = vec!["/downloads".to_string()];
    assert!(validate_task_save_dir(Some("/downloads"), &paths).is_ok());
    assert!(validate_task_save_dir(Some("/downloads/movies"), &paths).is_ok());
    assert_eq!(
        validate_task_save_dir(Some("/downloads-other/movies"), &paths),
        Err(TaskSaveDirError::Unauthorized)
    );
}

#[test]
fn prepare_task_save_dir_creates_missing_descendants_and_rejects_unsafe_paths() {
    let root = std::env::temp_dir().join(format!(
        "motrix-fnos-save-dir-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ));
    std::fs::create_dir(&root).expect("authorized root should create");
    let save_dir = root.join("火影").join("xxxx");
    let paths = vec![root.display().to_string()];

    prepare_task_save_dir(
        Some(save_dir.to_str().expect("path should be UTF-8")),
        &paths,
    )
    .expect("authorized descendants should be created");
    assert!(save_dir.is_dir());

    let escape = format!("{}/../outside", root.display());
    assert_eq!(
        prepare_task_save_dir(Some(&escape), &paths),
        Err(TaskSaveDirError::Unauthorized)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn prepare_task_save_dir_rejects_symlink_descendants() {
    use std::os::unix::fs::symlink;

    let base = std::env::temp_dir().join(format!(
        "motrix-fnos-save-dir-symlink-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ));
    let root = base.join("root");
    let outside = base.join("outside");
    std::fs::create_dir_all(&root).expect("authorized root should create");
    std::fs::create_dir(&outside).expect("outside directory should create");
    symlink(&outside, root.join("linked")).expect("symlink should create");
    let paths = vec![root.display().to_string()];

    let error = prepare_task_save_dir(
        Some(
            root.join("linked")
                .join("child")
                .to_str()
                .expect("path should be UTF-8"),
        ),
        &paths,
    )
    .expect_err("symlink descendant must be rejected");
    assert!(matches!(error, TaskSaveDirError::PrepareFailed(_)));
    assert!(!outside.join("child").exists());
    let _ = std::fs::remove_dir_all(base);
}
