use super::model::{serialize_tasks, Aria2CompatTask, Aria2GlobalStat};
use super::params::{TaskLane, MAX_PAGE_SIZE};
use crate::api::jsonrpc::types::RpcFault;
use crate::api::tasks::task_service;
use crate::app::HttpAppState;
use crate::storage::load_accessible_paths;
use crate::tasks::{DownloadTask, DownloadTaskStatus};
use serde_json::Value;
use std::sync::Arc;

pub(super) fn global_stat(state: &Arc<HttpAppState>) -> Result<Value, RpcFault> {
    let tasks = visible_compat_tasks(state)?;
    let mut stat = Aria2GlobalStat::empty();
    for task in tasks {
        match task.status {
            DownloadTaskStatus::Active => {
                stat.num_active += 1;
                stat.download_speed += task.download_speed;
            }
            DownloadTaskStatus::Pending | DownloadTaskStatus::Paused => stat.num_waiting += 1,
            DownloadTaskStatus::Complete | DownloadTaskStatus::Error => stat.num_stopped += 1,
            _ => {}
        }
    }
    Ok(stat.to_value())
}

pub(super) fn tell(
    state: &Arc<HttpAppState>,
    lane: TaskLane,
    offset: i64,
    num: u64,
    keys: &[String],
) -> Result<Value, RpcFault> {
    let mut tasks = visible_compat_tasks(state)?
        .into_iter()
        .filter(|task| lane.includes(task))
        .collect::<Vec<_>>();
    tasks.sort_by(|left, right| lane.compare(left, right));

    let range = page_range(tasks.len(), offset, num)?;
    Ok(serialize_tasks(&tasks[range], keys))
}

fn visible_compat_tasks(state: &Arc<HttpAppState>) -> Result<Vec<DownloadTask>, RpcFault> {
    let accessible_paths = load_accessible_paths(&state.runtime.accessible_paths_path)
        .map_err(RpcFault::server_error)?;
    task_service(state)
        .list_download_task_snapshot()
        .map_err(RpcFault::server_error)
        .map(|tasks| {
            tasks
                .into_iter()
                .filter(|task| {
                    let save_dir = task.save_dir.trim();
                    !save_dir.is_empty() && accessible_paths.iter().any(|path| path == save_dir)
                })
                .filter(|task| Aria2CompatTask::from_download_task(task).is_some())
                .collect()
        })
}

fn page_range(len: usize, offset: i64, num: u64) -> Result<std::ops::Range<usize>, RpcFault> {
    if num > MAX_PAGE_SIZE {
        return Err(RpcFault::invalid_params(format!(
            "num exceeds the server limit of {MAX_PAGE_SIZE}"
        )));
    }
    let start = if offset >= 0 {
        usize::try_from(offset).map_err(|_| RpcFault::invalid_params("offset is too large"))?
    } else {
        let distance = usize::try_from(offset.unsigned_abs()).unwrap_or(usize::MAX);
        len.saturating_sub(distance)
    }
    .min(len);
    let end = start.saturating_add(num as usize).min(len);
    Ok(start..end)
}

impl TaskLane {
    fn includes(self, task: &DownloadTask) -> bool {
        match self {
            Self::Active => task.status == DownloadTaskStatus::Active,
            Self::Waiting => matches!(
                task.status,
                DownloadTaskStatus::Pending | DownloadTaskStatus::Paused
            ),
            Self::Stopped => matches!(
                task.status,
                DownloadTaskStatus::Complete | DownloadTaskStatus::Error
            ),
        }
    }

    fn compare(self, left: &DownloadTask, right: &DownloadTask) -> std::cmp::Ordering {
        match self {
            Self::Stopped => right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id)),
            Self::Active | Self::Waiting => left
                .created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id)),
        }
    }
}

#[cfg(test)]
mod tests;
