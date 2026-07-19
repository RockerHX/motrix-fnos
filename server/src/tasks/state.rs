use crate::tasks::{
    should_pause_task_on_exit, DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, PreparedDownloadTask,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::current_timestamp_ms;
use crate::tasks::files::delete_task_file;

pub struct TaskMemoryState {
    tasks: Mutex<Vec<DownloadTask>>,
}

impl TaskMemoryState {
    pub fn new(tasks: Vec<DownloadTask>) -> Self {
        Self {
            tasks: Mutex::new(tasks),
        }
    }

    pub fn list(&self) -> Result<Vec<DownloadTask>, String> {
        self.tasks
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "无法读取下载任务列表".to_string())
    }

    pub fn with_tasks_mut<T>(
        &self,
        update: impl FnOnce(&mut Vec<DownloadTask>) -> T,
    ) -> Result<T, String> {
        let mut guard = self
            .tasks
            .lock()
            .map_err(|_| "无法写入下载任务列表".to_string())?;
        Ok(update(&mut guard))
    }

    fn lock(&self) -> Result<MutexGuard<'_, Vec<DownloadTask>>, String> {
        self.tasks
            .lock()
            .map_err(|_| "无法写入下载任务列表".to_string())
    }
}

pub fn store_created_task(
    tasks: &TaskMemoryState,
    next_id: &AtomicU64,
    prepared: PreparedDownloadTask,
    gid: String,
) -> Result<DownloadTask, String> {
    let task_id = next_id.fetch_add(1, Ordering::Relaxed);
    store_created_task_with_id(tasks, task_id, prepared, gid)
}

pub fn store_created_task_with_id(
    tasks: &TaskMemoryState,
    task_id: u64,
    prepared: PreparedDownloadTask,
    gid: String,
) -> Result<DownloadTask, String> {
    let file_path = if prepared.source_type == DownloadTaskSourceType::Magnet {
        None
    } else {
        Some(
            Path::new(&prepared.save_dir)
                .join(&prepared.file_name)
                .display()
                .to_string(),
        )
    };
    let status = initial_task_status(&prepared);
    let now = current_timestamp_ms();
    let task = DownloadTask {
        id: task_id,
        source_type: prepared.source_type,
        file_name: prepared.file_name,
        save_dir: prepared.save_dir,
        category: prepared.category,
        url: prepared.url,
        gid: Some(gid),
        status,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path,
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: now,
        updated_at: now,
    };

    let mut guard = tasks.lock()?;
    guard.push(task.clone());

    Ok(task)
}

fn initial_task_status(prepared: &PreparedDownloadTask) -> DownloadTaskStatus {
    if prepared.source_type == DownloadTaskSourceType::Magnet {
        return DownloadTaskStatus::Pending;
    }

    match prepared.start_mode {
        DownloadTaskStartMode::Now => DownloadTaskStatus::Pending,
        DownloadTaskStartMode::Paused => DownloadTaskStatus::Paused,
    }
}

pub(crate) fn should_refresh_task(task: &DownloadTask) -> bool {
    if task.confirmation_required {
        return true;
    }
    matches!(
        task.status,
        DownloadTaskStatus::Pending | DownloadTaskStatus::Active
    )
}

pub(crate) fn apply_readded_gid(task: &mut DownloadTask, new_gid: &str) {
    task.gid = Some(new_gid.to_string());
    task.status = DownloadTaskStatus::Active;
    task.download_speed = 0;
    task.error_code = None;
    task.error_message = None;
    task.file_path = Some(
        Path::new(&task.save_dir)
            .join(&task.file_name)
            .display()
            .to_string(),
    );
    task.updated_at = current_timestamp_ms();
}

pub fn list_tasks(tasks: &TaskMemoryState) -> Result<Vec<DownloadTask>, String> {
    tasks.list()
}

pub fn task_gid(tasks: &TaskMemoryState, task_id: u64) -> Result<String, String> {
    let task = task_snapshot(tasks, task_id)?;

    if task.status == DownloadTaskStatus::Removed {
        return Err("已删除任务不能继续操作".to_string());
    }

    task.gid
        .clone()
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "下载任务缺少 Aria2 GID，无法控制".to_string())
}

pub fn task_snapshot(tasks: &TaskMemoryState, task_id: u64) -> Result<DownloadTask, String> {
    let guard = tasks
        .tasks
        .lock()
        .map_err(|_| "无法读取下载任务列表".to_string())?;
    guard
        .iter()
        .find(|task| task.id == task_id)
        .cloned()
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))
}

pub fn remove_task_record(tasks: &TaskMemoryState, task_id: u64) -> Result<(), String> {
    let mut guard = tasks.lock()?;
    let index = guard
        .iter()
        .position(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;
    guard.remove(index);
    Ok(())
}

pub fn mark_task_paused(tasks: &TaskMemoryState, task_id: u64) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        apply_paused_state(task);
        Ok(())
    })
}

pub fn mark_task_paused_by_gid(tasks: &TaskMemoryState, gid: &str) -> Result<DownloadTask, String> {
    let mut guard = tasks.lock()?;
    let task = guard
        .iter_mut()
        .find(|task| task.gid.as_deref() == Some(gid))
        .ok_or_else(|| format!("下载任务不存在，GID {}", gid))?;
    apply_paused_state(task);
    Ok(task.clone())
}

pub fn mark_unfinished_tasks_paused(tasks: &TaskMemoryState) -> Result<Vec<DownloadTask>, String> {
    let mut guard = tasks.lock()?;
    let mut updated = Vec::new();
    for task in guard
        .iter_mut()
        .filter(|task| should_pause_task_on_exit(task))
    {
        apply_paused_state(task);
        task.updated_at = current_timestamp_ms();
        updated.push(task.clone());
    }
    Ok(updated)
}

pub(crate) fn apply_paused_state(task: &mut DownloadTask) {
    task.status = DownloadTaskStatus::Paused;
    task.download_speed = 0;
    task.error_code = None;
    task.error_message = None;
}

pub fn mark_task_resumed(tasks: &TaskMemoryState, task_id: u64) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if task.confirmation_required {
            return Err("请先确认要下载的文件".to_string());
        }
        task.status = DownloadTaskStatus::Active;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

pub fn mark_task_files_confirmed(
    tasks: &TaskMemoryState,
    task_id: u64,
    gid: String,
    save_dir: String,
    selected_indexes: &[u32],
    metadata_torrent_path: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.gid = Some(gid);
        task.save_dir = save_dir;
        task.confirmation_required = false;
        task.status = DownloadTaskStatus::Active;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.metadata_torrent_path = Some(metadata_torrent_path);
        task.files_deleted = false;
        task.selected_file_indexes = selected_indexes.to_vec();
        task.file_path = Some(
            Path::new(&task.save_dir)
                .join(&task.file_name)
                .display()
                .to_string(),
        );
        for file in &mut task.files {
            file.selected = selected_indexes.contains(&file.index);
        }
        Ok(())
    })
}

pub fn set_task_metadata_torrent_path(
    tasks: &TaskMemoryState,
    task_id: u64,
    metadata_torrent_path: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.metadata_torrent_path = Some(metadata_torrent_path);
        Ok(())
    })
}

pub fn mark_task_removed(
    tasks: &TaskMemoryState,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if delete_files {
            delete_task_file(task)?;
        }
        task.status = DownloadTaskStatus::Removed;
        task.files_deleted = delete_files;
        task.selected_file_indexes = task
            .files
            .iter()
            .filter(|file| file.selected)
            .map(|file| file.index)
            .collect();
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

pub fn mark_task_redownloaded(
    tasks: &TaskMemoryState,
    task_id: u64,
    new_gid: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if task.status != DownloadTaskStatus::Complete {
            return Err("只有已完成任务可以重新下载".to_string());
        }

        task.gid = Some(new_gid);
        task.status = DownloadTaskStatus::Pending;
        task.total_length = 0;
        task.completed_length = 0;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.confirmation_required = false;
        task.file_path = Some(
            Path::new(&task.save_dir)
                .join(&task.file_name)
                .display()
                .to_string(),
        );
        Ok(())
    })
}

fn update_task(
    tasks: &TaskMemoryState,
    task_id: u64,
    update: impl FnOnce(&mut DownloadTask) -> Result<(), String>,
) -> Result<DownloadTask, String> {
    let mut guard = tasks.lock()?;
    let task = guard
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;

    update(task)?;
    task.updated_at = current_timestamp_ms();
    Ok(task.clone())
}
