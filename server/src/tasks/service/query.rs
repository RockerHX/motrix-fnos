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
