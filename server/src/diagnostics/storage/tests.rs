use super::{collect_storage_usage, StorageFileUsage};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn collects_disk_session_and_nested_magnet_metadata_usage() {
    let root = temp_dir("usage");
    fs::create_dir_all(root.join("aria2")).expect("aria2 directory should create");
    fs::create_dir_all(root.join("magnet-metadata/task-1/nested"))
        .expect("metadata directories should create");
    fs::write(root.join("aria2/aria2.session"), b"session-data").expect("session should write");
    fs::write(
        root.join("magnet-metadata/task-1/metadata.torrent"),
        b"torrent",
    )
    .expect("metadata should write");
    fs::write(
        root.join("magnet-metadata/task-1/nested/piece"),
        b"piece-data",
    )
    .expect("nested metadata should write");

    let usage = collect_storage_usage(&root).expect("storage usage should collect");

    assert_eq!(usage.aria2_session_bytes, 12);
    assert_eq!(
        usage.magnet_metadata,
        StorageFileUsage {
            total_bytes: 17,
            file_count: 2,
        }
    );
    assert!(usage.disk.total_bytes > 0);
    assert!(usage.disk.available_bytes <= usage.disk.total_bytes);
    remove_temp_dir(root);
}

#[test]
fn ignores_missing_and_symbolic_linked_private_files() {
    let root = temp_dir("links");
    fs::create_dir_all(root.join("magnet-metadata/task-1"))
        .expect("metadata directory should create");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        fs::write(root.join("outside"), b"outside-data").expect("outside file should write");
        symlink(
            root.join("outside"),
            root.join("magnet-metadata/task-1/file-link"),
        )
        .expect("file symlink should create");
        symlink(
            root.join("outside"),
            root.join("magnet-metadata/task-1/dir-link"),
        )
        .expect("directory symlink should create");
    }

    let usage = collect_storage_usage(&root).expect("storage usage should collect");
    assert_eq!(usage.aria2_session_bytes, 0);
    assert_eq!(usage.magnet_metadata, StorageFileUsage::default());
    remove_temp_dir(root);
}

#[test]
fn rejects_symbolic_linked_application_data_directory() {
    let parent = temp_dir("root-link");
    let target = parent.join("target");
    let link = parent.join("link");
    fs::create_dir(&target).expect("target directory should create");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, &link).expect("application data symlink should create");
        let error = collect_storage_usage(&link).expect_err("symlink should reject");
        assert!(error.contains("应用数据目录"));
    }

    remove_temp_dir(parent);
}

fn temp_dir(label: &str) -> PathBuf {
    let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-diagnostics-storage-{label}-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("temporary directory should create");
    path
}

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
