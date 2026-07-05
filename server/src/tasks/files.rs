use crate::tasks::DownloadTask;
use std::path::{Path, PathBuf};
use std::fs;

pub fn delete_task_files(task: &DownloadTask) -> Result<(), String> {
    delete_task_file(task)
}

pub(crate) fn delete_task_file(task: &DownloadTask) -> Result<(), String> {
    let Some(file_path) = task
        .file_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(());
    };

    let save_dir = Path::new(&task.save_dir)
        .canonicalize()
        .map_err(|error| format!("校验保存目录失败：{}（{}）", task.save_dir, error))?;
    let candidates = delete_file_candidates(Path::new(file_path));

    for path in candidates {
        if !path.exists() {
            continue;
        }
        if !path.is_file() {
            return Err(format!("当前仅支持删除单文件：{}", path.display()));
        }

        let file = path
            .canonicalize()
            .map_err(|error| format!("校验本地文件失败：{}（{}）", path.display(), error))?;
        if !file.starts_with(&save_dir) {
            return Err("拒绝删除保存目录外的文件".to_string());
        }

        delete_local_file(&file)?;
    }

    Ok(())
}

fn delete_local_file(file: &Path) -> Result<(), String> {
    fs::remove_file(file).map_err(|error| format!("删除本地文件失败：{}（{}）", file.display(), error))
}

pub(crate) fn cleanup_aria2_control_file(task: &DownloadTask) {
    let Some(file_path) = task
        .file_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return;
    };

    let control_file = PathBuf::from(format!("{}.aria2", file_path));
    if !control_file.is_file() || !control_file_is_under_save_dir(&control_file, &task.save_dir) {
        return;
    }

    let _ = fs::remove_file(control_file);
}

fn control_file_is_under_save_dir(control_file: &Path, save_dir: &str) -> bool {
    let Ok(save_dir) = Path::new(save_dir).canonicalize() else {
        return false;
    };
    let Some(parent) = control_file.parent() else {
        return false;
    };
    parent
        .canonicalize()
        .map(|parent| parent.starts_with(save_dir))
        .unwrap_or(false)
}

pub(crate) fn delete_file_candidates(path: &Path) -> Vec<PathBuf> {
    vec![
        path.to_path_buf(),
        PathBuf::from(format!("{}.aria2", path.display())),
    ]
}

