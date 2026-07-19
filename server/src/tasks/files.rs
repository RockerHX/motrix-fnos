use crate::tasks::{DownloadTask, PreparedDownloadTask};
use std::fs;
use std::path::{Path, PathBuf};

const RESTORE_METADATA_ROOT: &str = "task-metadata";
const RESTORE_TORRENT_FILE: &str = "source.torrent";

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

pub(crate) fn save_restore_torrent_metadata(
    app_data_dir: &Path,
    task_id: u64,
    torrent_data: &[u8],
) -> Result<PathBuf, String> {
    if torrent_data.is_empty() {
        return Err("种子 metadata 不能为空".to_string());
    }
    let metadata_dir = restore_metadata_dir(app_data_dir, task_id);
    fs::create_dir_all(&metadata_dir).map_err(|error| {
        format!(
            "创建任务恢复 metadata 目录失败：{}（{}）",
            metadata_dir.display(),
            error
        )
    })?;
    let path = metadata_dir.join(RESTORE_TORRENT_FILE);
    fs::write(&path, torrent_data).map_err(|error| {
        format!(
            "保存任务恢复 metadata 失败：{}（{}）",
            path.display(),
            error
        )
    })?;
    Ok(path)
}

pub(crate) fn archive_task_torrent_metadata(
    app_data_dir: &Path,
    task: &DownloadTask,
) -> Result<PathBuf, String> {
    let target = restore_torrent_path(app_data_dir, task.id);
    if target.is_file() {
        return Ok(target);
    }
    let source = find_saved_torrent_metadata_path(task)?;
    let data = fs::read(&source).map_err(|error| {
        format!(
            "读取待归档种子 metadata 失败：{}（{}）",
            source.display(),
            error
        )
    })?;
    save_restore_torrent_metadata(app_data_dir, task.id, &data)
}

pub(crate) fn remove_restore_metadata(app_data_dir: &Path, task_id: u64) {
    let metadata_dir = restore_metadata_dir(app_data_dir, task_id);
    if metadata_dir.is_dir() {
        let _ = fs::remove_dir_all(metadata_dir);
    }
}

pub(crate) fn restore_torrent_path(app_data_dir: &Path, task_id: u64) -> PathBuf {
    restore_metadata_dir(app_data_dir, task_id).join(RESTORE_TORRENT_FILE)
}

fn restore_metadata_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join(RESTORE_METADATA_ROOT)
        .join(format!("task-{task_id}"))
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
        return Err(format!("磁链 metadata 种子文件不存在：{}", path.display()));
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

pub(crate) fn bt_task_path_component(name: &str) -> String {
    let safe_name = safe_task_path_component(name);
    Path::new(&safe_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(&safe_name)
        .to_string()
}

fn delete_bt_task_dir(task: &DownloadTask, allow_magnet_default_name: bool) -> Result<(), String> {
    // BT 删除只能作用于任务创建时分配的专属目录；符号链接、根目录和名称不匹配的目录一律拒绝递归删除。
    let task_dir = Path::new(&task.save_dir);
    let metadata = match fs::symlink_metadata(task_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "读取 BT 任务目录元数据失败：{}（{}）",
                task_dir.display(),
                error
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "拒绝删除符号链接形式的 BT 任务目录：{}",
            task_dir.display()
        ));
    }
    if !metadata.is_dir() {
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
    let task_name = bt_task_path_component(&task.file_name);
    let magnet_default_name = bt_task_path_component("磁力链接任务");
    // 升级前创建的 BT 专属目录可能保留完整任务名及扩展名，删除时需要兼容这类既有任务。
    let legacy_task_name = safe_task_path_component(&task.file_name);
    let matches_task_name = matches_bt_task_dir_name(dir_name, &task_name)
        || matches_bt_task_dir_name(dir_name, &legacy_task_name);
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

fn matches_bt_task_dir_name(dir_name: &str, task_name: &str) -> bool {
    dir_name == task_name || dir_name.starts_with(&format!("{} (", task_name))
}

fn delete_non_torrent_task_files(task: &DownloadTask) -> Result<(), String> {
    let Some(file_path) = task
        .file_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Ok(());
    };

    // 单文件删除同样要在 canonicalize 后确认仍位于任务保存目录内，不能信任数据库中的原始路径文本。
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
