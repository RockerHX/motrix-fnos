use crate::tasks::{
    should_pause_task_on_exit, DownloadTask, DownloadTaskSourceType, DownloadTaskStartMode,
    DownloadTaskStatus, PreparedDownloadTask,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use super::current_timestamp_ms;
use crate::tasks::files::delete_task_file;
use std::collections::HashSet;

pub struct TaskMemoryState {
    tasks: Mutex<Vec<DownloadTask>>,
    active_operations: Mutex<HashSet<u64>>,
}

impl TaskMemoryState {
    pub fn new(tasks: Vec<DownloadTask>) -> Self {
        Self {
            tasks: Mutex::new(tasks),
            active_operations: Mutex::new(HashSet::new()),
        }
    }

    pub fn begin_operation(&self, task_id: u64) -> Result<TaskOperationGuard<'_>, String> {
        let mut operations = self
            .active_operations
            .lock()
            .map_err(|_| "无法锁定任务操作状态".to_string())?;
        if !operations.insert(task_id) {
            return Err("该任务已有操作正在进行，请稍后重试".to_string());
        }
        Ok(TaskOperationGuard {
            state: self,
            task_id,
        })
    }

    pub fn list(&self) -> Result<Vec<DownloadTask>, String> {
        self.tasks
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "无法读取下载任务列表".to_string())
    }

    pub fn active_operation_count(&self) -> Result<usize, String> {
        self.active_operations
            .lock()
            .map(|operations| operations.len())
            .map_err(|_| "无法读取任务操作状态".to_string())
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

pub struct TaskOperationGuard<'a> {
    state: &'a TaskMemoryState,
    task_id: u64,
}

impl Drop for TaskOperationGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut operations) = self.state.active_operations.lock() {
            operations.remove(&self.task_id);
        }
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
        save_dir: prepared.save_dir.clone(),
        owned_task_dir: (prepared.source_type == DownloadTaskSourceType::Torrent)
            .then_some(prepared.save_dir),
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
        use_proxy: prepared.use_proxy,
        proxy_binding: prepared.proxy_binding,
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
        Path::new(task.owned_task_dir.as_deref().unwrap_or(&task.save_dir))
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
    proxy_binding: crate::tasks::TaskProxyBinding,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.gid = Some(gid);
        task.save_dir = save_dir.clone();
        task.owned_task_dir = Some(save_dir);
        task.confirmation_required = false;
        task.status = DownloadTaskStatus::Active;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.metadata_torrent_path = Some(metadata_torrent_path);
        task.proxy_binding = proxy_binding;
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

pub fn mark_task_restored(
    tasks: &TaskMemoryState,
    task_id: u64,
    gid: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if task.status != DownloadTaskStatus::Removed {
            return Err("只有回收站任务可以恢复".to_string());
        }
        task.gid = Some(gid);
        task.status = DownloadTaskStatus::Paused;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.confirmation_required = false;
        if task.files_deleted {
            task.total_length = 0;
            task.completed_length = 0;
            for file in &mut task.files {
                file.completed_length = 0;
            }
        }
        task.files_deleted = false;
        Ok(())
    })
}

pub fn mark_magnet_task_reparsing(
    tasks: &TaskMemoryState,
    task_id: u64,
    gid: String,
    base_save_dir: String,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if task.status != DownloadTaskStatus::Removed {
            return Err("只有回收站任务可以恢复".to_string());
        }
        task.gid = Some(gid);
        task.save_dir = base_save_dir;
        task.owned_task_dir = None;
        task.status = DownloadTaskStatus::Pending;
        task.total_length = 0;
        task.completed_length = 0;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        task.file_path = None;
        task.metadata_torrent_path = None;
        task.confirmation_required = false;
        task.files.clear();
        task.files_deleted = false;
        Ok(())
    })
}

pub fn replace_task_snapshot(
    tasks: &TaskMemoryState,
    snapshot: DownloadTask,
) -> Result<(), String> {
    let mut guard = tasks.lock()?;
    let task = guard
        .iter_mut()
        .find(|task| task.id == snapshot.id)
        .ok_or_else(|| format!("下载任务不存在：{}", snapshot.id))?;
    *task = snapshot;
    Ok(())
}

pub fn update_task_proxy_state(
    tasks: &TaskMemoryState,
    task_id: u64,
    use_proxy: bool,
    proxy_binding: crate::tasks::TaskProxyBinding,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.use_proxy = use_proxy;
        task.proxy_binding = proxy_binding;
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
            Path::new(task.owned_task_dir.as_deref().unwrap_or(&task.save_dir))
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
