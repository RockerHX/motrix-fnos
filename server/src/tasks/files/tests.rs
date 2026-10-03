use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn failed_first_stage_removes_empty_backup_dir_on_rollback() {
    let root = temp_dir("first-stage-failure");
    fs::create_dir_all(&root).expect("root should create");
    let file = root.join("archive.zip");
    fs::write(&file, b"original").expect("file should write");

    let mut staged = prepare_stage_paths(1, Some(&root), vec![file.clone()])
        .expect("staging plan should prepare")
        .expect("existing file should be staged");
    let backup_dir = staged.backup_dir().to_path_buf();
    fs::remove_file(&file).expect("source should disappear before staging");

    assert!(staged.stage().is_err());
    staged
        .restore()
        .expect("rollback should remove empty backup directory");
    assert!(!backup_dir.exists());
    fs::remove_dir_all(root).expect("test root should remove");
}

#[test]
fn failed_staging_can_restore_files_already_moved() {
    let root = temp_dir("stage-failure");
    fs::create_dir_all(&root).expect("root should create");
    let file = root.join("archive.zip");
    let control_file = root.join("archive.zip.aria2");
    fs::write(&file, b"original").expect("file should write");
    fs::write(&control_file, b"control").expect("control file should write");

    let mut staged = prepare_stage_paths(1, Some(&root), vec![file.clone(), control_file.clone()])
        .expect("staging plan should prepare")
        .expect("existing files should be staged");
    let backup_dir = staged.backup_dir().to_path_buf();
    fs::remove_file(&control_file).expect("second file should disappear");

    let error = staged.stage().expect_err("second move should fail");
    assert!(error.contains("暂存重新下载文件失败"));
    assert_eq!(
        fs::read(backup_dir.join("archive.zip")).expect("moved file should remain backed up"),
        b"original"
    );
    fs::write(&file, b"concurrent").expect("restore target should become occupied");
    let restore_error = staged
        .restore()
        .expect_err("occupied rollback target should preserve the backup");
    assert!(restore_error.contains(&backup_dir.display().to_string()));
    assert_eq!(
        fs::read(&file).expect("concurrent file should remain"),
        b"concurrent"
    );
    assert_eq!(
        fs::read(backup_dir.join("archive.zip")).expect("original should remain backed up"),
        b"original"
    );

    fs::remove_file(&file).expect("restore target conflict should remove");
    staged.restore().expect("first file should roll back");
    assert_eq!(fs::read(&file).expect("file should restore"), b"original");
    assert!(!control_file.exists());
    assert!(!backup_dir.exists());
    fs::remove_dir_all(root).expect("test root should remove");
}

#[test]
fn restore_keeps_conflicting_backup_and_can_retry_after_partial_restore() {
    let root = temp_dir("restore-conflict");
    fs::create_dir_all(&root).expect("root should create");
    let file = root.join("archive.zip");
    let control_file = root.join("archive.zip.aria2");
    fs::write(&file, b"original").expect("file should write");
    fs::write(&control_file, b"control").expect("control file should write");

    let mut staged = prepare_stage_paths(1, Some(&root), vec![file.clone(), control_file.clone()])
        .expect("staging plan should prepare")
        .expect("existing files should be staged");
    let backup_dir = staged.backup_dir().to_path_buf();
    staged.stage().expect("files should stage");
    fs::write(&file, b"concurrent").expect("concurrent target should create");

    let error = staged
        .restore()
        .expect_err("existing restore target should block restoration");
    assert!(error.contains(&backup_dir.display().to_string()));
    assert_eq!(
        fs::read(&file).expect("concurrent file should remain"),
        b"concurrent"
    );
    assert_eq!(
        fs::read(&control_file).expect("reverse-order partial restore should remain"),
        b"control"
    );
    assert_eq!(
        fs::read(backup_dir.join("archive.zip")).expect("original should remain backed up"),
        b"original"
    );

    fs::remove_file(&file).expect("conflict should remove");
    staged.restore().expect("remaining restore should retry");
    assert_eq!(
        fs::read(&file).expect("original file should restore"),
        b"original"
    );
    assert_eq!(
        fs::read(&control_file).expect("control file should remain restored"),
        b"control"
    );
    assert!(!backup_dir.exists());
    fs::remove_dir_all(root).expect("test root should remove");
}

fn temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "motrix-task-files-{label}-{}-{counter}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    path
}
