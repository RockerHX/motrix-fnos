use crate::tasks::{DownloadTask, PreparedDownloadTask};
use std::fs;
use std::path::{Path, PathBuf};

pub fn delete_task_files(task: &DownloadTask) -> Result<(), String> {
    delete_task_file(task)
}

pub(crate) fn delete_task_file(task: &DownloadTask) -> Result<(), String> {
    let lower_url = task.url.to_ascii_lowercase();
    if lower_url.starts_with("torrent:") || lower_url.starts_with("magnet:?") {
        return delete_bt_task_dir(task, lower_url.starts_with("magnet:?"));
    }

    delete_non_torrent_task_files(task)
}

pub(crate) fn cleanup_empty_torrent_task_dir(task: &PreparedDownloadTask) {
    let path = Path::new(&task.save_dir);
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    }
}

pub(crate) fn read_saved_torrent_metadata(task: &DownloadTask) -> Result<Vec<u8>, String> {
    let torrent_path = find_saved_torrent_metadata_path(task)?;
    fs::read(&torrent_path).map_err(|error| {
        format!(
            "读取磁链 metadata 种子失败：{}（{}）",
            torrent_path.display(),
            error
        )
    })
}

fn find_saved_torrent_metadata_path(task: &DownloadTask) -> Result<PathBuf, String> {
    if let Some(path) = task
        .metadata_torrent_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "磁链 metadata 种子文件不存在：{}",
            path.display()
        ));
    }

    let task_dir = Path::new(&task.save_dir);
    find_single_torrent_file(task_dir)
}

pub(crate) fn find_single_torrent_file(task_dir: &Path) -> Result<PathBuf, String> {
    if !task_dir.is_dir() {
        return Err(format!("磁链 metadata 目录不存在：{}", task_dir.display()));
    }

    let mut candidates = fs::read_dir(task_dir)
        .map_err(|error| {
            format!(
                "读取磁链 metadata 目录失败：{}（{}）",
                task_dir.display(),
                error
            )
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| extension.eq_ignore_ascii_case("torrent"))
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    candidates.sort();

    match candidates.len() {
        0 => Err(format!(
            "磁链 metadata 已解析但未找到 .torrent 文件：{}",
            task_dir.display()
        )),
        1 => Ok(candidates.remove(0)),
        _ => Err(format!(
            "磁链 metadata 目录存在多个 .torrent 文件，无法确定要使用哪一个：{}",
            task_dir.display()
        )),
    }
}

pub(crate) fn safe_task_path_component(name: &str) -> String {
    let sanitized = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                ch
            }
        })
        .collect::<String>()
        .trim_matches([' ', '.'])
        .chars()
        .take(120)
        .collect::<String>();

    if sanitized.is_empty() {
        "未命名种子任务".to_string()
    } else {
        sanitized
    }
}

fn delete_bt_task_dir(task: &DownloadTask, allow_magnet_default_name: bool) -> Result<(), String> {
    let task_dir = Path::new(&task.save_dir);
    if !task_dir.exists() {
        return Ok(());
    }
    if !task_dir.is_dir() {
        return delete_non_torrent_task_files(task);
    }

    let canonical_task_dir = task_dir
        .canonicalize()
        .map_err(|error| format!("校验种子任务目录失败：{}（{}）", task_dir.display(), error))?;
    let Some(parent) = canonical_task_dir.parent() else {
        return Err("拒绝删除根目录".to_string());
    };
    if parent == canonical_task_dir {
        return Err("拒绝删除根目录".to_string());
    }

    let dir_name = canonical_task_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let task_name = safe_task_path_component(&task.file_name);
    let magnet_default_name = safe_task_path_component("磁力链接任务");
    let matches_task_name =
        dir_name == task_name || dir_name.starts_with(&format!("{} (", task_name));
    let matches_magnet_default_name = allow_magnet_default_name
        && (dir_name == magnet_default_name
            || dir_name.starts_with(&format!("{} (", magnet_default_name)));
    if !matches_task_name && !matches_magnet_default_name {
        return Err(format!(
            "拒绝删除非 BT 任务专属目录：{}",
            canonical_task_dir.display()
        ));
    }

    fs::remove_dir_all(&canonical_task_dir).map_err(|error| {
        format!(
            "删除 BT 任务目录失败：{}（{}）",
            canonical_task_dir.display(),
            error
        )
    })
}

fn delete_non_torrent_task_files(task: &DownloadTask) -> Result<(), String> {
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
    fs::remove_file(file)
        .map_err(|error| format!("删除本地文件失败：{}（{}）", file.display(), error))
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
