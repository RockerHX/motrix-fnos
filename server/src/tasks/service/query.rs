use super::TaskService;
use crate::config::aria2::Aria2Config;
use crate::tasks::{refresh_tasks_from_aria2, DownloadTask, DownloadTaskStatus};

pub(super) async fn list_download_tasks(
    service: &TaskService<'_>,
    config: &Aria2Config,
) -> Result<Vec<DownloadTask>, String> {
    if service.runtime_guard.is_exiting() {
        service.debug_logs.info(
            "tasks.list",
            "应用正在退出，跳过 Aria2 刷新并返回内存任务快照",
        );
        return Ok(visible_tasks(crate::tasks::list_tasks(
            service.download_tasks,
        )?));
    }

    let tasks = refresh_tasks_from_aria2(
        service.download_tasks,
        service.app_data_dir,
        config,
        Some(service.debug_logs),
    )
    .await?;
    sync_tasks_to_database(service, &tasks).await?;

    Ok(visible_tasks(tasks))
}

pub(super) fn list_removed_download_tasks(
    service: &TaskService<'_>,
) -> Result<Vec<DownloadTask>, String> {
    let tasks = crate::tasks::list_tasks(service.download_tasks)?;
    Ok(removed_tasks(tasks))
}

pub(super) async fn sync_tasks_to_database(
    service: &TaskService<'_>,
    tasks: &[DownloadTask],
) -> Result<(), String> {
    service.repository.persist_task_states(tasks).await
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
