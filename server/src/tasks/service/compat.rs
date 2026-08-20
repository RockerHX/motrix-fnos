use super::TaskService;
use crate::tasks::DownloadTask;

pub(super) fn get_download_task_by_gid(
    service: &TaskService<'_>,
    gid: &str,
) -> Result<Option<DownloadTask>, String> {
    find_download_task_by_gid(service.download_tasks.list()?, gid)
}

fn find_download_task_by_gid(
    tasks: Vec<DownloadTask>,
    gid: &str,
) -> Result<Option<DownloadTask>, String> {
    let gid = gid.trim();
    if gid.is_empty() {
        return Err("Aria2 GID 不能为空".to_string());
    }

    let mut matches = tasks
        .into_iter()
        .filter(|task| task.gid.as_deref().map(str::trim) == Some(gid));
    let result = matches.next();
    if matches.next().is_some() {
        return Err(format!("存在多个使用同一 Aria2 GID 的任务：{gid}"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
