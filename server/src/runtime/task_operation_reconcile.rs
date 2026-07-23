use super::ensure_aria2_ready;
use crate::app::HttpAppState;
use crate::database::task_operations::{list_unfinished_task_operations, update_task_operation};
use crate::database::tasks::persist_download_task_state_with_operation;
use crate::tasks::{
    remove_task, DownloadTask, DownloadTaskStatus, TaskOperation, TaskOperationStatus,
    TaskOperationType,
};
use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Aria2TaskPresence {
    Present,
    Missing,
    Unknown(String),
}

enum ReconcileAction {
    Complete(String),
    Fail(String),
    ManualReview(String),
    RemoveUnpersistedAria2Task(String),
}

pub async fn reconcile_unfinished_task_operations(state: &HttpAppState) -> Result<(), String> {
    let operations = list_unfinished_task_operations(&state.core.database.pool).await?;
    if operations.is_empty() {
        return Ok(());
    }

    let gid_presence = inspect_referenced_gids(state, &operations).await;
    let mut tasks = state.core.download_tasks.list()?;

    for mut operation in operations {
        // 已经进入人工处理的记录不能在后续重启中被自动覆盖；保留当时的证据和用户文件。
        if operation.status == TaskOperationStatus::ManualReview {
            continue;
        }

        let action = decide_reconcile_action(&operation, &tasks, &gid_presence);
        let action = match action {
            ReconcileAction::RemoveUnpersistedAria2Task(gid) => {
                match remove_unpersisted_aria2_task(state, &gid).await {
                    Ok(()) => ReconcileAction::Fail(
                        "服务重启时已撤销尚未写入任务记录的 Aria2 任务".to_string(),
                    ),
                    Err(error) => ReconcileAction::ManualReview(format!(
                        "服务重启时发现未持久化的 Aria2 任务，但撤销失败：{}；已保留用户文件",
                        error
                    )),
                }
            }
            action => action,
        };

        apply_reconcile_action(
            &state.core.database.pool,
            &mut tasks,
            &mut operation,
            action,
        )
        .await?;
    }

    state
        .core
        .download_tasks
        .with_tasks_mut(|stored| *stored = tasks)?;
    Ok(())
}

async fn inspect_referenced_gids(
    state: &HttpAppState,
    operations: &[TaskOperation],
) -> HashMap<String, Aria2TaskPresence> {
    let gids = operations
        .iter()
        .filter(|operation| operation.status == TaskOperationStatus::InProgress)
        .filter_map(operation_gid)
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    if gids.is_empty() {
        return HashMap::new();
    }

    let config = match ensure_aria2_ready(state).await {
        Ok(config) => config,
        Err(error) => {
            state.core.debug_logs.warn(
                "runtime.operation_reconcile",
                format!("启动时无法连接 Aria2，未完成操作将等待人工处理：{}", error),
            );
            return gids
                .into_iter()
                .map(|gid| (gid, Aria2TaskPresence::Unknown(error.clone())))
                .collect();
        }
    };

    let client = reqwest::Client::new();
    let mut presence = HashMap::new();
    for gid in gids {
        let result = crate::tasks::aria2_rpc::task_exists(
            &client,
            &config,
            &gid,
            Some(&state.core.debug_logs),
        )
        .await;
        let value = match result {
            Ok(true) => Aria2TaskPresence::Present,
            Ok(false) => Aria2TaskPresence::Missing,
            Err(error) => Aria2TaskPresence::Unknown(error),
        };
        presence.insert(gid, value);
    }
    presence
}

async fn remove_unpersisted_aria2_task(state: &HttpAppState, gid: &str) -> Result<(), String> {
    let config = state.aria2_config();
    remove_task(&config, gid, Some(&state.core.debug_logs))
        .await
        .map(|_| ())
}

fn decide_reconcile_action(
    operation: &TaskOperation,
    tasks: &[DownloadTask],
    gid_presence: &HashMap<String, Aria2TaskPresence>,
) -> ReconcileAction {
    if operation.phase == "prepared" {
        return ReconcileAction::Fail("服务重启前操作尚未产生可确认的外部副作用".to_string());
    }

    if has_staged_user_files(operation) {
        return ReconcileAction::ManualReview(
            "服务重启时发现用户文件暂存记录，未自动移动或删除文件，需要人工确认".to_string(),
        );
    }

    let task = tasks.iter().find(|task| task.id == operation.task_id);
    let Some(new_gid) = operation
        .context
        .new_gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty())
    else {
        return ReconcileAction::ManualReview(
            "服务重启时无法确认外部下载引擎状态，已保留任务和用户文件，需要人工处理".to_string(),
        );
    };

    match gid_presence.get(new_gid) {
        Some(Aria2TaskPresence::Present) if task_has_gid(task, new_gid) => {
            if can_complete_persisted_operation(operation) {
                ReconcileAction::Complete("启动时已确认任务记录与 Aria2 状态一致".to_string())
            } else {
                ReconcileAction::ManualReview(
                    "服务重启时发现已持久化但未完成的任务操作，未自动重复执行，需要人工处理"
                        .to_string(),
                )
            }
        }
        Some(Aria2TaskPresence::Present) => {
            ReconcileAction::RemoveUnpersistedAria2Task(new_gid.to_string())
        }
        Some(Aria2TaskPresence::Missing) if task_has_gid(task, new_gid) => {
            ReconcileAction::ManualReview(
                "任务记录引用的 Aria2 GID 不存在，未自动重建或删除用户文件，需要人工处理"
                    .to_string(),
            )
        }
        Some(Aria2TaskPresence::Missing) => {
            ReconcileAction::Fail("服务重启时未发现尚未写入任务记录的 Aria2 任务".to_string())
        }
        Some(Aria2TaskPresence::Unknown(error)) => ReconcileAction::ManualReview(format!(
            "服务重启时无法确认 Aria2 任务结果：{}；已保留用户文件",
            error
        )),
        None => ReconcileAction::ManualReview(
            "服务重启时缺少 Aria2 任务核对结果，已保留用户文件，需要人工处理".to_string(),
        ),
    }
}

fn has_staged_user_files(operation: &TaskOperation) -> bool {
    operation
        .context
        .completed_side_effects
        .iter()
        .any(|effect| effect == "old_files_staged" || effect == "task_files_staged")
}

fn task_has_gid(task: Option<&DownloadTask>, gid: &str) -> bool {
    task.and_then(|task| task.gid.as_deref()) == Some(gid)
}

fn can_complete_persisted_operation(operation: &TaskOperation) -> bool {
    matches!(
        (operation.operation_type, operation.phase.as_str()),
        (TaskOperationType::Create, "task_persisted")
            | (TaskOperationType::Confirm, "task_confirmed")
            | (TaskOperationType::Restore, "task_restored")
    )
}

async fn apply_reconcile_action(
    pool: &sqlx::SqlitePool,
    tasks: &mut [DownloadTask],
    operation: &mut TaskOperation,
    action: ReconcileAction,
) -> Result<(), String> {
    match action {
        ReconcileAction::Complete(message) => {
            operation.complete("startup_reconciled");
            update_task_operation(pool, operation).await?;
            tracing::info!(module = "runtime.operation_reconcile", "{}", message);
        }
        ReconcileAction::Fail(message) => {
            operation.fail("startup_rolled_back", message);
            update_task_operation(pool, operation).await?;
        }
        ReconcileAction::ManualReview(message) => {
            operation.require_manual_review("startup_manual_review", message.clone());
            if let Some(task) = tasks.iter_mut().find(|task| task.id == operation.task_id) {
                if task.status != DownloadTaskStatus::Removed {
                    task.status = DownloadTaskStatus::Error;
                    task.download_speed = 0;
                    task.error_code = None;
                }
                task.error_message = Some(message);
                task.updated_at = current_timestamp_ms();
                persist_download_task_state_with_operation(pool, task, operation).await?;
            } else {
                update_task_operation(pool, operation).await?;
            }
        }
        ReconcileAction::RemoveUnpersistedAria2Task(_) => {
            return Err("未持久化 Aria2 任务尚未完成撤销".to_string());
        }
    }
    Ok(())
}

fn operation_gid(operation: &TaskOperation) -> Option<&str> {
    operation
        .context
        .new_gid
        .as_deref()
        .or(operation.context.old_gid.as_deref())
        .filter(|gid| !gid.trim().is_empty())
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
