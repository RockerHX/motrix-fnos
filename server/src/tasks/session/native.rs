use super::*;
use crate::config::aria2::EngineGeneration;
use crate::tasks::is_pending_magnet_metadata_task;
use std::path::PathBuf;

pub(super) fn sync_native_session(
    tasks: &TaskMemoryState,
    session: &[Aria2TaskStatus],
    app_data_dir: &Path,
) -> Result<Vec<DownloadTask>, String> {
    let mut snapshot = tasks.list()?;
    let mut seen = BTreeSet::new();
    for status in session {
        let gid = status
            .gid
            .as_deref()
            .filter(|gid| !gid.trim().is_empty())
            .ok_or_else(|| "task_resume_state_invalid：session 缺少 GID".to_string())?;
        if !seen.insert(gid) {
            return Err("task_resume_state_invalid：session GID 重复".to_string());
        }
        let matches = snapshot
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                task.engine_generation == EngineGeneration::NativeV2
                    && task.gid.as_deref() == Some(gid)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            return Err(
                "task_resume_state_invalid：session 身份未知或不唯一，已保留文件".to_string(),
            );
        };
        let task = &mut snapshot[*index];
        let expected_dir =
            if task.source_type == DownloadTaskSourceType::Magnet && task.file_path.is_none() {
                app_data_dir
                    .join("magnet-metadata")
                    .join(format!("task-{}", task.id))
            } else {
                PathBuf::from(task_download_dir(task))
            };
        *task = reconcile_native_task(task, status, &expected_dir)?;
    }
    for task in snapshot
        .iter()
        .filter(|task| task.engine_generation == EngineGeneration::NativeV2)
    {
        if task.status == DownloadTaskStatus::Complete
            || task.removed_from_status == Some(DownloadTaskStatus::Complete)
        {
            continue;
        }
        if !task.gid.as_deref().is_some_and(|gid| seen.contains(gid)) {
            return Err(format!(
                "task_resume_state_invalid：任务 {} 的原 GID 未加载，已保留文件",
                task.id
            ));
        }
    }
    tasks.with_tasks_mut(|stored| *stored = snapshot.clone())?;
    Ok(snapshot)
}

pub(crate) fn reconcile_native_task(
    task: &DownloadTask,
    status: &Aria2TaskStatus,
    expected_dir: &Path,
) -> Result<DownloadTask, String> {
    validate_native_task(task, status, expected_dir)?;
    let mut reconciled = task.clone();
    apply_aria2_status(&mut reconciled, status);
    if task.source_type == DownloadTaskSourceType::Magnet && task.file_path.is_none() {
        reconciled.files = super::super::progress::task_files(status);
    }
    reconciled.total_length = status
        .total_length
        .parse()
        .map_err(|_| "原生任务总大小无效".to_string())?;
    reconciled.completed_length = status
        .completed_length
        .parse()
        .map_err(|_| "原生任务有效进度无效".to_string())?;
    reconciled.save_dir = task.save_dir.clone();
    if task.confirmation_required || is_pending_magnet_metadata_task(task) {
        reconciled.file_path = task.file_path.clone();
    }
    // 回收站和完成历史属于 SQLite 事实，不能被暂停加载覆盖。
    if matches!(
        task.status,
        DownloadTaskStatus::Removed | DownloadTaskStatus::Complete
    ) {
        reconciled.status = task.status.clone();
    }
    if task.files_deleted {
        reconciled.completed_length = 0;
        for file in &mut reconciled.files {
            file.completed_length = 0;
        }
    }
    reconciled.download_speed = 0;
    Ok(reconciled)
}

fn validate_native_task(
    task: &DownloadTask,
    status: &Aria2TaskStatus,
    expected_dir: &Path,
) -> Result<(), String> {
    let invalid = || {
        format!(
            "task_resume_state_invalid：任务 {} 的状态、目录或文件选择无法核对，已保留文件",
            task.id
        )
    };
    if task.gid.as_deref() != status.gid.as_deref()
        || !matches!(status.status.as_str(), "paused" | "complete" | "error")
    {
        return Err(invalid());
    }
    if status
        .followed_by
        .as_ref()
        .is_some_and(|gids| !gids.is_empty())
    {
        return Err(invalid());
    }
    let dir = status.dir.as_deref().ok_or_else(invalid)?;
    if Path::new(dir) != expected_dir {
        return Err(invalid());
    }
    let mut indexes = BTreeSet::new();
    let files = status.files.as_deref().unwrap_or_default();
    if status.total_length.parse::<u64>().is_err()
        || status.completed_length.parse::<u64>().is_err()
        || status.completed_length.parse::<u64>().unwrap()
            > status.total_length.parse::<u64>().unwrap()
        || (task.confirmation_required && files.is_empty())
    {
        return Err(invalid());
    }
    for file in files {
        let relative = Path::new(&file.path)
            .strip_prefix(expected_dir)
            .map_err(|_| invalid())?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || file.index == 0
            || !indexes.insert(file.index)
            || !matches!(file.selected.as_str(), "true" | "false")
            || file.length.parse::<u64>().is_err()
            || file.completed_length.parse::<u64>().is_err()
        {
            return Err(invalid());
        }
        if let Some(saved) = task.files.iter().find(|saved| saved.index == file.index) {
            if Path::new(&saved.path) != Path::new(&file.path)
                || saved.length != file.length.parse::<u64>().map_err(|_| invalid())?
                || saved.selected != (file.selected == "true")
            {
                return Err(invalid());
            }
        }
    }
    if !task.files.is_empty() && task.files.len() != files.len() {
        return Err(invalid());
    }
    if !task.selected_file_indexes.is_empty() {
        let selected = files
            .iter()
            .filter(|file| file.selected == "true")
            .map(|file| file.index)
            .collect::<BTreeSet<_>>();
        if selected != task.selected_file_indexes.iter().copied().collect() {
            return Err(invalid());
        }
    }
    Ok(())
}
