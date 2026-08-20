use super::TaskService;
use crate::config::aria2::Aria2Config;
use crate::tasks::{DownloadTask, DownloadTaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatTaskOperation {
    Pause,
    Unpause,
    Remove,
    RemoveDownloadResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatBatchOperation {
    PauseAll,
    UnpauseAll,
    PurgeDownloadResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatAria2Requirement {
    None,
    Required,
    IfRunning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatTaskError {
    GidNotFound,
    Conflict(&'static str),
    Aria2Required,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompatTaskTarget {
    pub aria2_requirement: CompatAria2Requirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatBatchPlan {
    pub aria2_requirement: CompatAria2Requirement,
    operation: CompatBatchOperation,
    gids: Vec<String>,
}

impl CompatBatchPlan {
    pub fn target_count(&self) -> usize {
        self.gids.len()
    }

    pub fn gids(&self) -> &[String] {
        &self.gids
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompatBatchResult {
    pub target_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
}

impl CompatBatchResult {
    pub fn is_complete(self) -> bool {
        self.failed_count == 0
    }
}

pub(super) fn get_download_task_by_gid(
    service: &TaskService<'_>,
    gid: &str,
) -> Result<Option<DownloadTask>, String> {
    find_download_task_by_gid(service.download_tasks.list()?, gid)
}

impl<'a> TaskService<'a> {
    pub fn compat_task_target(
        &self,
        operation: CompatTaskOperation,
        gid: &str,
    ) -> Result<CompatTaskTarget, CompatTaskError> {
        let task = self.find_compat_task(gid)?;
        Ok(CompatTaskTarget {
            aria2_requirement: compat_aria2_requirement(operation, &task)?,
        })
    }

    pub async fn pause_by_compat_gid(
        &self,
        config: Option<&Aria2Config>,
        gid: &str,
    ) -> Result<DownloadTask, CompatTaskError> {
        let task = self.find_compat_task(gid)?;
        match compat_aria2_requirement(CompatTaskOperation::Pause, &task)? {
            CompatAria2Requirement::None => Ok(task),
            CompatAria2Requirement::Required => {
                let config = config.ok_or(CompatTaskError::Aria2Required)?;
                self.pause_download_task(config, task.id)
                    .await
                    .map_err(map_task_operation_error)
            }
            CompatAria2Requirement::IfRunning => Err(CompatTaskError::Internal),
        }
    }

    pub async fn unpause_by_compat_gid(
        &self,
        config: Option<&Aria2Config>,
        gid: &str,
    ) -> Result<DownloadTask, CompatTaskError> {
        let task = self.find_compat_task(gid)?;
        match compat_aria2_requirement(CompatTaskOperation::Unpause, &task)? {
            CompatAria2Requirement::None => Ok(task),
            CompatAria2Requirement::Required => {
                let config = config.ok_or(CompatTaskError::Aria2Required)?;
                self.resume_download_task(config, task.id)
                    .await
                    .map_err(map_task_operation_error)
            }
            CompatAria2Requirement::IfRunning => Err(CompatTaskError::Internal),
        }
    }

    pub async fn remove_by_compat_gid(
        &self,
        config: Option<&Aria2Config>,
        gid: &str,
    ) -> Result<DownloadTask, CompatTaskError> {
        let task = self.find_compat_task(gid)?;
        match compat_aria2_requirement(CompatTaskOperation::Remove, &task)? {
            CompatAria2Requirement::None => Ok(task),
            CompatAria2Requirement::Required => {
                let config = config.ok_or(CompatTaskError::Aria2Required)?;
                self.delete_download_task(config, task.id, false)
                    .await
                    .map_err(map_task_operation_error)
            }
            CompatAria2Requirement::IfRunning => Err(CompatTaskError::Internal),
        }
    }

    pub async fn remove_download_result_by_compat_gid(
        &self,
        config: Option<&Aria2Config>,
        gid: &str,
    ) -> Result<DownloadTask, CompatTaskError> {
        let task = self.find_compat_task(gid)?;
        match compat_aria2_requirement(CompatTaskOperation::RemoveDownloadResult, &task)? {
            CompatAria2Requirement::None => Ok(task),
            CompatAria2Requirement::IfRunning => self
                .remove_download_result_task(config, task.id)
                .await
                .map_err(map_task_operation_error),
            CompatAria2Requirement::Required => Err(CompatTaskError::Internal),
        }
    }

    pub fn plan_compat_batch(
        &self,
        operation: CompatBatchOperation,
    ) -> Result<CompatBatchPlan, CompatTaskError> {
        let mut tasks = self
            .list_download_task_snapshot()
            .map_err(|_| CompatTaskError::Internal)?;
        tasks.retain(|task| compat_batch_includes(operation, task));
        tasks.sort_by(|left, right| compat_batch_order(operation, left, right));

        let gids = tasks
            .into_iter()
            .filter_map(|task| task.gid.map(|gid| gid.trim().to_string()))
            .filter(|gid| !gid.is_empty())
            .collect::<Vec<_>>();
        let aria2_requirement = if gids.is_empty() {
            CompatAria2Requirement::None
        } else {
            match operation {
                CompatBatchOperation::PauseAll | CompatBatchOperation::UnpauseAll => {
                    CompatAria2Requirement::Required
                }
                CompatBatchOperation::PurgeDownloadResult => CompatAria2Requirement::IfRunning,
            }
        };

        Ok(CompatBatchPlan {
            aria2_requirement,
            operation,
            gids,
        })
    }

    pub async fn execute_compat_batch(
        &self,
        plan: CompatBatchPlan,
        config: Option<&Aria2Config>,
    ) -> CompatBatchResult {
        let mut result = CompatBatchResult {
            target_count: plan.gids.len(),
            ..CompatBatchResult::default()
        };

        for gid in plan.gids {
            let outcome = match plan.operation {
                CompatBatchOperation::PauseAll => self.pause_by_compat_gid(config, &gid).await,
                CompatBatchOperation::UnpauseAll => self.unpause_by_compat_gid(config, &gid).await,
                CompatBatchOperation::PurgeDownloadResult => {
                    self.remove_download_result_by_compat_gid(config, &gid)
                        .await
                }
            };
            if outcome.is_ok() {
                result.completed_count += 1;
            } else {
                result.failed_count += 1;
            }
        }
        result
    }

    fn find_compat_task(&self, gid: &str) -> Result<DownloadTask, CompatTaskError> {
        self.get_download_task_by_gid(gid)
            .map_err(|_| CompatTaskError::Internal)?
            .ok_or(CompatTaskError::GidNotFound)
    }
}

fn compat_batch_includes(operation: CompatBatchOperation, task: &DownloadTask) -> bool {
    match operation {
        CompatBatchOperation::PauseAll => matches!(
            task.status,
            DownloadTaskStatus::Pending | DownloadTaskStatus::Active
        ),
        CompatBatchOperation::UnpauseAll => task.status == DownloadTaskStatus::Paused,
        CompatBatchOperation::PurgeDownloadResult => matches!(
            task.status,
            DownloadTaskStatus::Complete | DownloadTaskStatus::Error
        ),
    }
}

fn compat_batch_order(
    operation: CompatBatchOperation,
    left: &DownloadTask,
    right: &DownloadTask,
) -> std::cmp::Ordering {
    match operation {
        CompatBatchOperation::PauseAll | CompatBatchOperation::UnpauseAll => left
            .created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id)),
        CompatBatchOperation::PurgeDownloadResult => right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| left.id.cmp(&right.id)),
    }
}

fn compat_aria2_requirement(
    operation: CompatTaskOperation,
    task: &DownloadTask,
) -> Result<CompatAria2Requirement, CompatTaskError> {
    match operation {
        CompatTaskOperation::Pause => match task.status {
            DownloadTaskStatus::Paused => Ok(CompatAria2Requirement::None),
            DownloadTaskStatus::Pending | DownloadTaskStatus::Active => {
                Ok(CompatAria2Requirement::Required)
            }
            DownloadTaskStatus::Complete
            | DownloadTaskStatus::Error
            | DownloadTaskStatus::Removed => {
                Err(CompatTaskError::Conflict("当前任务状态不支持暂停"))
            }
        },
        CompatTaskOperation::Unpause => match task.status {
            DownloadTaskStatus::Pending | DownloadTaskStatus::Active => {
                Ok(CompatAria2Requirement::None)
            }
            DownloadTaskStatus::Paused => Ok(CompatAria2Requirement::Required),
            DownloadTaskStatus::Complete
            | DownloadTaskStatus::Error
            | DownloadTaskStatus::Removed => {
                Err(CompatTaskError::Conflict("当前任务状态不支持继续"))
            }
        },
        CompatTaskOperation::Remove => match task.status {
            DownloadTaskStatus::Removed => Ok(CompatAria2Requirement::None),
            DownloadTaskStatus::Pending
            | DownloadTaskStatus::Active
            | DownloadTaskStatus::Paused => Ok(CompatAria2Requirement::Required),
            DownloadTaskStatus::Complete | DownloadTaskStatus::Error => Err(
                CompatTaskError::Conflict("完成或错误任务请使用 removeDownloadResult 清理"),
            ),
        },
        CompatTaskOperation::RemoveDownloadResult => match task.status {
            DownloadTaskStatus::Removed => Ok(CompatAria2Requirement::None),
            DownloadTaskStatus::Complete | DownloadTaskStatus::Error => {
                Ok(CompatAria2Requirement::IfRunning)
            }
            DownloadTaskStatus::Pending
            | DownloadTaskStatus::Active
            | DownloadTaskStatus::Paused => Err(CompatTaskError::Conflict(
                "只有已完成或错误任务可以清理下载结果",
            )),
        },
    }
}

fn map_task_operation_error(error: String) -> CompatTaskError {
    if error.contains("已有操作正在进行")
        || error.contains("应用正在退出")
        || error.contains("文件仍在后台清理")
    {
        CompatTaskError::Conflict("任务当前不可操作，请刷新后重试")
    } else {
        CompatTaskError::Internal
    }
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
