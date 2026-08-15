use super::TaskService;
use crate::tasks::{DownloadTask, DownloadTaskStatus};

pub(super) fn list_download_task_snapshot(
    service: &TaskService<'_>,
) -> Result<Vec<DownloadTask>, String> {
    Ok(visible_tasks(crate::tasks::list_tasks(
        service.download_tasks,
    )?))
}

pub(super) fn list_removed_download_tasks(
    service: &TaskService<'_>,
) -> Result<Vec<DownloadTask>, String> {
    let tasks = crate::tasks::list_tasks(service.download_tasks)?;
    Ok(removed_tasks(tasks))
}

pub(super) fn get_download_task(
    service: &TaskService<'_>,
    task_id: u64,
) -> Result<Option<DownloadTask>, String> {
    Ok(find_download_task(
        crate::tasks::list_tasks(service.download_tasks)?,
        task_id,
    ))
}

fn find_download_task(tasks: Vec<DownloadTask>, task_id: u64) -> Option<DownloadTask> {
    tasks.into_iter().find(|task| task.id == task_id)
}

fn visible_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status != DownloadTaskStatus::Removed)
        .collect()
}

fn removed_tasks(tasks: Vec<DownloadTask>) -> Vec<DownloadTask> {
    tasks
        .into_iter()
        .filter(|task| task.status == DownloadTaskStatus::Removed)
        .collect()
}

#[cfg(test)]
mod tests;
