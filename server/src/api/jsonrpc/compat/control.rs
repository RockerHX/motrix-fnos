use super::super::types::RpcFault;
use super::params::ControlOperation;
use crate::api::tasks::task_service;
use crate::app::HttpAppState;
use crate::config::aria2::Aria2Config;
use crate::runtime::{
    broadcast_tasks_snapshot, ensure_aria2_ready, process_status, Aria2Lease, Aria2LifecyclePhase,
};
use crate::storage::load_accessible_paths;
use crate::tasks::service::{
    CompatAria2Requirement, CompatBatchOperation, CompatBatchResult, CompatTaskError,
    CompatTaskOperation,
};
use serde_json::Value;
use std::sync::Arc;

pub(super) async fn execute(
    state: &Arc<HttpAppState>,
    operation: ControlOperation,
    gid: &str,
) -> Result<Value, RpcFault> {
    let service = task_service(state);
    let operation = compat_operation(operation)
        .ok_or_else(|| RpcFault::server_error("批量兼容方法将在后续阶段接入"))?;
    let target = service
        .compat_task_target(operation, gid)
        .map_err(map_compat_error)?;

    let result = match target.aria2_requirement {
        CompatAria2Requirement::None => execute_with_config(&service, operation, None, gid).await,
        CompatAria2Requirement::Required => {
            let config = ensure_aria2_ready(state).await.map_err(map_runtime_error)?;
            execute_with_config(&service, operation, Some(&config), gid).await
        }
        CompatAria2Requirement::IfRunning => {
            let context = running_aria2_context(state)
                .await
                .map_err(map_runtime_error)?;
            execute_with_config(
                &service,
                operation,
                context.as_ref().map(|context| &context.config),
                gid,
            )
            .await
        }
    }?;

    broadcast_tasks_snapshot(state).map_err(RpcFault::server_error)?;
    Ok(result)
}

pub(super) async fn execute_batch(
    state: &Arc<HttpAppState>,
    operation: ControlOperation,
) -> Result<Value, RpcFault> {
    let operation =
        batch_operation(operation).ok_or_else(|| RpcFault::server_error("不是批量兼容方法"))?;
    let accessible_paths = load_accessible_paths(&state.runtime.accessible_paths_path)
        .map_err(RpcFault::server_error)?;
    let service = task_service(state);
    let plan = service
        .plan_compat_batch(operation, &accessible_paths)
        .map_err(map_compat_error)?;

    let result = match plan.aria2_requirement {
        CompatAria2Requirement::None | CompatAria2Requirement::IfRunning => {
            let context = if plan.aria2_requirement == CompatAria2Requirement::IfRunning {
                running_aria2_context(state)
                    .await
                    .map_err(map_runtime_error)?
            } else {
                None
            };
            service
                .execute_compat_batch(plan, context.as_ref().map(|context| &context.config))
                .await
        }
        CompatAria2Requirement::Required => {
            let config = ensure_aria2_ready(state).await.map_err(map_runtime_error)?;
            service.execute_compat_batch(plan, Some(&config)).await
        }
    };

    broadcast_tasks_snapshot(state).map_err(RpcFault::server_error)?;
    batch_result_value(result)
}

fn compat_operation(operation: ControlOperation) -> Option<CompatTaskOperation> {
    match operation {
        ControlOperation::Pause => Some(CompatTaskOperation::Pause),
        ControlOperation::Unpause => Some(CompatTaskOperation::Unpause),
        ControlOperation::Remove => Some(CompatTaskOperation::Remove),
        ControlOperation::RemoveDownloadResult => Some(CompatTaskOperation::RemoveDownloadResult),
        ControlOperation::PauseAll
        | ControlOperation::UnpauseAll
        | ControlOperation::PurgeDownloadResult => None,
    }
}

fn batch_operation(operation: ControlOperation) -> Option<CompatBatchOperation> {
    match operation {
        ControlOperation::PauseAll => Some(CompatBatchOperation::PauseAll),
        ControlOperation::UnpauseAll => Some(CompatBatchOperation::UnpauseAll),
        ControlOperation::PurgeDownloadResult => Some(CompatBatchOperation::PurgeDownloadResult),
        ControlOperation::Pause
        | ControlOperation::Unpause
        | ControlOperation::Remove
        | ControlOperation::RemoveDownloadResult => None,
    }
}

fn batch_result_value(result: CompatBatchResult) -> Result<Value, RpcFault> {
    if !result.is_complete() {
        return Err(RpcFault::batch_failed(result.failed_count));
    }
    Ok(Value::String("OK".to_string()))
}

async fn execute_with_config(
    service: &crate::tasks::service::TaskService<'_>,
    operation: CompatTaskOperation,
    config: Option<&crate::config::aria2::Aria2Config>,
    gid: &str,
) -> Result<Value, RpcFault> {
    match operation {
        CompatTaskOperation::Pause => service
            .pause_by_compat_gid(config, gid)
            .await
            .map(|task| Value::String(task.gid.unwrap_or_else(|| gid.to_string())))
            .map_err(map_compat_error),
        CompatTaskOperation::Unpause => service
            .unpause_by_compat_gid(config, gid)
            .await
            .map(|task| Value::String(task.gid.unwrap_or_else(|| gid.to_string())))
            .map_err(map_compat_error),
        CompatTaskOperation::Remove => service
            .remove_by_compat_gid(config, gid)
            .await
            .map(|task| Value::String(task.gid.unwrap_or_else(|| gid.to_string())))
            .map_err(map_compat_error),
        CompatTaskOperation::RemoveDownloadResult => service
            .remove_download_result_by_compat_gid(config, gid)
            .await
            .map(|_| Value::String("OK".to_string()))
            .map_err(map_compat_error),
    }
}

struct RunningAria2Context {
    config: Aria2Config,
    _activity: Aria2Lease,
}

async fn running_aria2_context(
    state: &HttpAppState,
) -> Result<Option<RunningAria2Context>, String> {
    let activity = state.aria2_lifecycle.acquire_activity()?;
    let _operation = state
        .aria2_lifecycle
        .lock_lifecycle_operation_for_request()
        .await?;
    let status = process_status(&state.aria2_process)?;
    if !status.running {
        return Ok(None);
    }
    let Some(runtime) = state.aria2_runtime_snapshot() else {
        return Err("Aria2 进程已运行但运行态未记录，拒绝使用未知配置".to_string());
    };
    if status.pid != Some(runtime.pid) {
        return Err(format!(
            "Aria2 进程 PID {} 与运行态 PID {} 不一致",
            status.pid.unwrap_or_default(),
            runtime.pid
        ));
    }
    if state.aria2_lifecycle.snapshot()?.phase != Aria2LifecyclePhase::Ready {
        return Err("Aria2 正在切换运行状态，请稍后重试".to_string());
    }
    Ok(Some(RunningAria2Context {
        config: state.aria2_config(),
        _activity: activity,
    }))
}

fn map_runtime_error(error: String) -> RpcFault {
    if error.contains("生命周期转换超时")
        || error.contains("生命周期请求被拒绝")
        || error.contains("Aria2 正在停止")
        || error.contains("Aria2 正在切换运行状态")
    {
        RpcFault::aria2_busy(error)
    } else {
        RpcFault::server_error(error)
    }
}

fn map_compat_error(error: CompatTaskError) -> RpcFault {
    match error {
        CompatTaskError::GidNotFound => RpcFault::gid_not_found(),
        CompatTaskError::Conflict(message) => RpcFault::task_conflict(message),
        CompatTaskError::Aria2Required => RpcFault::server_error("当前任务操作需要 Aria2 运行态"),
        CompatTaskError::Internal => RpcFault::server_error("任务操作失败，请稍后重试"),
    }
}
