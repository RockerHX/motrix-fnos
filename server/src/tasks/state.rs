use crate::tasks::{
    should_pause_task_on_exit, DownloadTask, DownloadTaskStatus, PreparedDownloadTask,
};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use super::{current_timestamp_ms, delete_task_file};

pub fn store_created_task(
    tasks: &Mutex<Vec<DownloadTask>>,
    next_id: &AtomicU64,
    prepared: PreparedDownloadTask,
    gid: String,
) -> Result<DownloadTask, String> {
    let file_path = Path::new(&prepared.save_dir)
        .join(&prepared.file_name)
        .display()
        .to_string();
    let now = current_timestamp_ms();
    let task = DownloadTask {
        id: next_id.fetch_add(1, Ordering::Relaxed),
        file_name: prepared.file_name,
        save_dir: prepared.save_dir,
        url: prepared.url,
        gid: Some(gid),
        status: DownloadTaskStatus::Pending,
        total_length: 0,
        completed_length: 0,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some(file_path),
        created_at: now,
        updated_at: now,
    };

    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    guard.push(task.clone());

    Ok(task)
}

pub(crate) fn should_refresh_task(task: &DownloadTask) -> bool {
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

pub fn list_tasks(tasks: &Mutex<Vec<DownloadTask>>) -> Result<Vec<DownloadTask>, String> {
    tasks
        .lock()
        .map(|guard| guard.clone())
        .map_err(|_| "无法读取下载任务列表".to_string())
}

pub fn task_gid(tasks: &Mutex<Vec<DownloadTask>>, task_id: u64) -> Result<String, String> {
    let task = task_snapshot(tasks, task_id)?;

    if task.status == DownloadTaskStatus::Removed {
        return Err("已删除任务不能继续操作".to_string());
    }

    task.gid
        .clone()
        .filter(|gid| !gid.trim().is_empty())
        .ok_or_else(|| "下载任务缺少 Aria2 GID，无法控制".to_string())
}

pub fn task_snapshot(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    let guard = tasks
        .lock()
        .map_err(|_| "无法读取下载任务列表".to_string())?;
    guard
        .iter()
        .find(|task| task.id == task_id)
        .cloned()
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))
}

pub fn remove_task_record(tasks: &Mutex<Vec<DownloadTask>>, task_id: u64) -> Result<(), String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let index = guard
        .iter()
        .position(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;
    guard.remove(index);
    Ok(())
}

pub fn mark_task_paused(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        apply_paused_state(task);
        Ok(())
    })
}

pub fn mark_task_paused_by_gid(
    tasks: &Mutex<Vec<DownloadTask>>,
    gid: &str,
) -> Result<DownloadTask, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.gid.as_deref() == Some(gid))
        .ok_or_else(|| format!("下载任务不存在，GID {}", gid))?;
    apply_paused_state(task);
    Ok(task.clone())
}

pub fn mark_unfinished_tasks_paused(
    tasks: &Mutex<Vec<DownloadTask>>,
) -> Result<Vec<DownloadTask>, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
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

pub fn mark_task_resumed(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        task.status = DownloadTaskStatus::Active;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

pub fn mark_task_removed(
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
    delete_files: bool,
) -> Result<DownloadTask, String> {
    update_task(tasks, task_id, |task| {
        if delete_files {
            delete_task_file(task)?;
        }
        task.status = DownloadTaskStatus::Removed;
        task.download_speed = 0;
        task.error_code = None;
        task.error_message = None;
        Ok(())
    })
}

pub fn mark_task_redownloaded(
    tasks: &Mutex<Vec<DownloadTask>>,
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
    tasks: &Mutex<Vec<DownloadTask>>,
    task_id: u64,
    update: impl FnOnce(&mut DownloadTask) -> Result<(), String>,
) -> Result<DownloadTask, String> {
    let mut guard = tasks
        .lock()
        .map_err(|_| "无法写入下载任务列表".to_string())?;
    let task = guard
        .iter_mut()
        .find(|task| task.id == task_id)
        .ok_or_else(|| format!("下载任务不存在：{}", task_id))?;

    update(task)?;
    task.updated_at = current_timestamp_ms();
    Ok(task.clone())
}

