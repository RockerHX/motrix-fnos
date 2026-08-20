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

    fn find_compat_task(&self, gid: &str) -> Result<DownloadTask, CompatTaskError> {
        self.get_download_task_by_gid(gid)
            .map_err(|_| CompatTaskError::Internal)?
            .ok_or(CompatTaskError::GidNotFound)
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
