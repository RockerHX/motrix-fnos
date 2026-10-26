use super::ensure_aria2_ready;
use crate::app::HttpAppState;
use crate::config::aria2::EngineGeneration;
use crate::database::task_operations::{
    list_unfinished_task_operations_for_engine, update_task_operation,
};
use crate::database::tasks::{
    delete_download_task_record_with_operation, persist_download_task_state_with_operation,
};
use crate::debug_logs::{emit_file_log, DebugLogLevel};
use crate::tasks::{
    find_aria2_task_for_request, is_stale_aria2_gid_error, reconcile_task_proxy_option,
    remove_task, remove_task_result, remove_task_with_request_id, task_exists, DownloadTask,
    DownloadTaskStatus, TaskOperation, TaskOperationStatus, TaskOperationType,
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
    FinalizeRemoved(String),
    FinalizeRestored(String),
    FinalizePermanentDelete(String),
    ReleaseReplacedAria2Task(String),
    RemoveUnpersistedAria2Task(String),
}

pub async fn reconcile_unfinished_task_operations(state: &HttpAppState) -> Result<(), String> {
    state.ensure_download_available()?;
    let operations = list_unfinished_task_operations_for_engine(
        &state.core.database.pool,
        crate::config::aria2::EngineGeneration::NativeV2,
    )
    .await?
    .into_iter()
    .filter(|operation| {
        operation.engine_generation() == crate::config::aria2::EngineGeneration::NativeV2
    })
    .filter(|operation| !operation.is_file_cleanup_pending())
    .collect::<Vec<_>>();
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

        let action = if operation.operation_type == TaskOperationType::Proxy {
            reconcile_proxy_operation(state, &operation, &tasks).await
        } else if operation.phase == "aria2_outcome_unknown" && operation.context.new_gid.is_none()
        {
            reconcile_unknown_aria2_request(state, &mut operation, &tasks).await
        } else {
            decide_reconcile_action(&operation, &tasks, &gid_presence)
        };
        let action = match action {
            ReconcileAction::ReleaseReplacedAria2Task(gid) => {
                match release_replaced_aria2_task(state, &operation, &tasks, &gid).await {
                    Ok(()) => ReconcileAction::Complete(
                        "启动时已释放替换任务遗留的旧 Aria2 GID".to_string(),
                    ),
                    Err(error) => ReconcileAction::ManualReview(format!(
                        "启动时释放替换任务旧 GID 失败：{}；已保留任务和用户文件",
                        error
                    )),
                }
            }
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

async fn reconcile_proxy_operation(
    state: &HttpAppState,
    operation: &TaskOperation,
    tasks: &[DownloadTask],
) -> ReconcileAction {
    let Some(task) = tasks.iter().find(|task| task.id == operation.task_id) else {
        return ReconcileAction::Fail("代理操作对应的任务已不存在".to_string());
    };
    if matches!(
        task.status,
        DownloadTaskStatus::Complete | DownloadTaskStatus::Removed
    ) || task.gid.as_deref().is_none_or(|gid| gid.trim().is_empty())
    {
        return ReconcileAction::Complete(
            "代理操作已按 SQLite 事实保留，将在任务下次运行时应用".to_string(),
        );
    }

    let config = match ensure_aria2_ready(state).await {
        Ok(config) => config,
        Err(error) => {
            return ReconcileAction::ManualReview(format!(
                "启动时无法连接 Aria2 对账任务代理：{}",
                error
            ));
        }
    };
    match reconcile_task_proxy_option(
        &state.aria2_rpc,
        &config,
        task,
        None,
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(_) => ReconcileAction::Complete("启动时已按 SQLite 事实对账任务代理".to_string()),
        Err(error) => ReconcileAction::ManualReview(format!(
            "启动时任务代理对账失败：{}；任务保持暂停",
            error
        )),
    }
}

async fn reconcile_unknown_aria2_request(
    state: &HttpAppState,
    operation: &mut TaskOperation,
    tasks: &[DownloadTask],
) -> ReconcileAction {
    let Some(request) = operation.context.aria2_request.as_ref() else {
        return ReconcileAction::ManualReview(
            "服务重启时缺少 Aria2 请求匹配信息，已保留用户文件，需要人工处理".to_string(),
        );
    };
    let config = match ensure_aria2_ready(state).await {
        Ok(config) => config,
        Err(error) => {
            return ReconcileAction::ManualReview(format!(
                "服务重启时无法连接 Aria2 核对未知请求结果：{}；已保留用户文件",
                error
            ));
        }
    };
    let mut excluded_gids = tasks
        .iter()
        .filter_map(|task| task.gid.clone())
        .collect::<BTreeSet<_>>();
    if let Some(old_gid) = operation.context.old_gid.as_ref() {
        excluded_gids.insert(old_gid.clone());
    }
    let gid = match find_aria2_task_for_request(
        &state.aria2_rpc,
        &config,
        request,
        &excluded_gids,
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(Some(gid)) => gid,
        Ok(None) => {
            return ReconcileAction::ManualReview(
                "服务重启时未能唯一确认 Aria2 请求结果，未自动重试或删除用户文件，需要人工处理"
                    .to_string(),
            );
        }
        Err(error) => {
            return ReconcileAction::ManualReview(format!(
                "服务重启时查询 Aria2 未知请求结果失败：{}；已保留用户文件",
                error
            ));
        }
    };

    let mut context = operation.context.clone();
    context.new_gid = Some(gid.clone());
    context
        .completed_side_effects
        .push("aria2_unknown_outcome_confirmed".to_string());
    operation.update_phase("aria2_outcome_confirmed", context);
    if let Err(error) = update_task_operation(&state.core.database.pool, operation).await {
        return ReconcileAction::ManualReview(format!(
            "服务重启时已确认 Aria2 请求结果，但无法记录 GID：{}；已保留用户文件",
            error
        ));
    }

    let presence = match task_exists(
        &state.aria2_rpc,
        &config,
        &gid,
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(true) => Aria2TaskPresence::Present,
        Ok(false) => Aria2TaskPresence::Missing,
        Err(error) => Aria2TaskPresence::Unknown(error),
    };
    let gid_presence = HashMap::from([(gid, presence)]);
    decide_reconcile_action(operation, tasks, &gid_presence)
}

async fn inspect_referenced_gids(
    state: &HttpAppState,
    operations: &[TaskOperation],
) -> HashMap<String, Aria2TaskPresence> {
    let gids = operations
        .iter()
        .filter(|operation| operation.status == TaskOperationStatus::InProgress)
        .filter(|operation| operation.operation_type != TaskOperationType::Proxy)
        .filter(|operation| !is_engine_free_history_operation(operation))
        .flat_map(operation_gids)
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

    let mut presence = HashMap::new();
    for gid in gids {
        let result = crate::tasks::aria2_rpc::task_exists(
            &state.aria2_rpc,
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
    let config = ensure_aria2_ready(state).await?;
    remove_task(&state.aria2_rpc, &config, gid, Some(&state.core.debug_logs))
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
    if let Some(action) = decide_replaced_gid_action(operation, task, gid_presence) {
        return action;
    }
    if let Some(action) = decide_permanent_delete_action(operation, task, gid_presence) {
        return action;
    }
    if let Some(action) = decide_retained_task_action(operation, task, gid_presence) {
        return action;
    }
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

fn decide_permanent_delete_action(
    operation: &TaskOperation,
    task: Option<&DownloadTask>,
    gid_presence: &HashMap<String, Aria2TaskPresence>,
) -> Option<ReconcileAction> {
    if operation.operation_type != TaskOperationType::PermanentDelete {
        return None;
    }
    let Some(task) = task else {
        return Some(ReconcileAction::ManualReview(
            "永久删除操作对应的任务记录不存在，需要人工核对".to_string(),
        ));
    };
    if task.status != DownloadTaskStatus::Removed {
        return Some(ReconcileAction::ManualReview(
            "永久删除操作对应的任务不在回收站，需要人工核对".to_string(),
        ));
    }
    if operation
        .context
        .completed_side_effects
        .iter()
        .any(|effect| effect == "delete_files_requested")
    {
        return Some(ReconcileAction::ManualReview(
            "永久删除的文件暂存尚未确认完成，已保留任务记录和文件，需要人工核对".to_string(),
        ));
    }
    let Some(old_gid) = operation
        .context
        .old_gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty())
    else {
        return Some(ReconcileAction::FinalizePermanentDelete(
            "永久删除任务没有原 GID，启动时补齐记录清理".to_string(),
        ));
    };

    match gid_presence.get(old_gid) {
        Some(Aria2TaskPresence::Missing) => Some(ReconcileAction::FinalizePermanentDelete(
            "原 GID 已确认不存在，启动时补齐记录清理".to_string(),
        )),
        Some(Aria2TaskPresence::Present) => Some(ReconcileAction::ManualReview(
            "永久删除操作的原 GID 仍存在，未自动删除任务记录".to_string(),
        )),
        Some(Aria2TaskPresence::Unknown(error)) => Some(ReconcileAction::ManualReview(format!(
            "服务重启时无法确认永久删除的原 GID：{}；已保留任务记录",
            error
        ))),
        None => Some(ReconcileAction::ManualReview(
            "服务重启时缺少永久删除的原 GID 核对结果，已保留任务记录".to_string(),
        )),
    }
}

fn decide_replaced_gid_action(
    operation: &TaskOperation,
    task: Option<&DownloadTask>,
    gid_presence: &HashMap<String, Aria2TaskPresence>,
) -> Option<ReconcileAction> {
    if !matches!(operation.operation_type, TaskOperationType::Restore) {
        return None;
    }
    let task = task?;
    let Some(new_gid) = operation
        .context
        .new_gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty())
    else {
        return None;
    };
    let Some(old_gid) = operation
        .context
        .old_gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty())
        .filter(|gid| *gid != new_gid)
    else {
        return None;
    };
    if !matches!(
        operation.phase.as_str(),
        "task_restored" | "engine_release_requested" | "engine_released"
    ) || !task_has_gid(Some(task), new_gid)
        || !matches!(gid_presence.get(new_gid), Some(Aria2TaskPresence::Present))
    {
        return None;
    }

    match gid_presence.get(old_gid) {
        Some(Aria2TaskPresence::Present) => Some(ReconcileAction::ReleaseReplacedAria2Task(
            old_gid.to_string(),
        )),
        Some(Aria2TaskPresence::Missing) => Some(ReconcileAction::Complete(
            "替换任务的旧 GID 已不存在，启动时完成操作对账".to_string(),
        )),
        Some(Aria2TaskPresence::Unknown(error)) => Some(ReconcileAction::ManualReview(format!(
            "服务重启时无法确认替换任务的旧 GID：{}；已保留任务和用户文件",
            error
        ))),
        None => None,
    }
}

fn decide_retained_task_action(
    operation: &TaskOperation,
    task: Option<&DownloadTask>,
    gid_presence: &HashMap<String, Aria2TaskPresence>,
) -> Option<ReconcileAction> {
    if !matches!(
        operation.operation_type,
        TaskOperationType::Delete | TaskOperationType::Restore
    ) {
        return None;
    }
    // Restore operations that created a replacement GID use the normal new-GID
    // reconciliation below; this branch is only for retained identities.
    if operation
        .context
        .new_gid
        .as_deref()
        .is_some_and(|gid| !gid.trim().is_empty())
    {
        return None;
    }
    let task = task?;
    let old_gid = operation
        .context
        .old_gid
        .as_deref()
        .filter(|gid| !gid.trim().is_empty());
    let history = is_engine_free_history_operation(operation);

    if history {
        return match operation.operation_type {
            TaskOperationType::Delete if task.status == DownloadTaskStatus::Removed => Some(
                ReconcileAction::Complete("完成历史回收状态已写入，未触碰引擎".to_string()),
            ),
            TaskOperationType::Delete => Some(ReconcileAction::FinalizeRemoved(
                "完成历史回收状态尚未写入，启动时补齐".to_string(),
            )),
            TaskOperationType::Restore if task.status == DownloadTaskStatus::Complete => Some(
                ReconcileAction::Complete("完成历史恢复状态已写入，未触碰引擎".to_string()),
            ),
            TaskOperationType::Restore => Some(ReconcileAction::FinalizeRestored(
                "完成历史恢复状态尚未写入，启动时补齐".to_string(),
            )),
            _ => None,
        };
    }

    let Some(old_gid) = old_gid else {
        return None;
    };
    match gid_presence.get(old_gid) {
        Some(Aria2TaskPresence::Present)
            if task.engine_generation == EngineGeneration::NativeV2
                && operation.operation_type == TaskOperationType::Delete
                && matches!(operation.phase.as_str(), "aria2_paused" | "aria2_retained")
                && task.status != DownloadTaskStatus::Removed =>
        {
            Some(ReconcileAction::FinalizeRemoved(
                "原生任务已暂停，启动时补齐回收站状态".to_string(),
            ))
        }
        Some(Aria2TaskPresence::Present)
            if task.engine_generation == EngineGeneration::NativeV2
                && operation.operation_type == TaskOperationType::Restore
                && task.status == DownloadTaskStatus::Removed =>
        {
            if task.files_deleted {
                Some(ReconcileAction::ManualReview(
                    "任务文件已删除，未自动绕过从头下载确认".to_string(),
                ))
            } else if operation.phase == "aria2_proxy_reconciled"
                && task.removed_from_status.is_some()
            {
                Some(ReconcileAction::FinalizeRestored(
                    "原生任务已暂停且代理已对账，启动时补齐恢复状态".to_string(),
                ))
            } else {
                Some(ReconcileAction::ManualReview(
                    "回收站任务缺少原始状态，未猜测恢复结果".to_string(),
                ))
            }
        }
        Some(Aria2TaskPresence::Present) => Some(ReconcileAction::Complete(
            "原 GID 与 SQLite 状态已一致".to_string(),
        )),
        Some(Aria2TaskPresence::Missing)
            if operation.operation_type == TaskOperationType::Delete
                && task.engine_generation != EngineGeneration::NativeV2
                && task.status == DownloadTaskStatus::Removed =>
        {
            Some(ReconcileAction::Complete(
                "旧版引擎任务已移除，回收站状态已写入".to_string(),
            ))
        }
        Some(Aria2TaskPresence::Unknown(error)) => Some(ReconcileAction::ManualReview(format!(
            "服务重启时无法确认原 GID：{}；已保留任务和文件",
            error
        ))),
        Some(Aria2TaskPresence::Missing) | None => Some(ReconcileAction::ManualReview(
            "服务重启时无法确认原 GID，未自动重建或删除用户文件".to_string(),
        )),
    }
}

fn is_engine_free_history_operation(operation: &TaskOperation) -> bool {
    operation.operation_type == TaskOperationType::Delete
        && operation
            .context
            .task_snapshot
            .as_ref()
            .is_some_and(|task| task.status == DownloadTaskStatus::Complete)
        || operation.operation_type == TaskOperationType::Restore
            && operation
                .context
                .task_snapshot
                .as_ref()
                .is_some_and(|task| {
                    task.status == DownloadTaskStatus::Removed
                        && !task.files_deleted
                        && task.removed_from_status == Some(DownloadTaskStatus::Complete)
                })
}

fn has_staged_user_files(operation: &TaskOperation) -> bool {
    operation.phase == "file_staging_in_progress"
        || operation
            .context
            .completed_side_effects
            .iter()
            .any(|effect| matches!(effect.as_str(), "old_files_staged" | "task_files_staged"))
}

async fn release_replaced_aria2_task(
    state: &HttpAppState,
    operation: &TaskOperation,
    tasks: &[DownloadTask],
    gid: &str,
) -> Result<(), String> {
    if tasks
        .iter()
        .any(|task| task.id != operation.task_id && task.gid.as_deref() == Some(gid))
    {
        return Ok(());
    }
    let config = ensure_aria2_ready(state).await?;
    match remove_task_with_request_id(
        &state.aria2_rpc,
        &config,
        gid,
        Some(&operation.id),
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(_) => {}
        Err(error) if is_stale_aria2_gid_error(&error) => {}
        Err(error) => return Err(error),
    }
    crate::aria2::save_session(&state.aria2_rpc, &config, Some(&state.core.debug_logs)).await?;
    if task_exists(&state.aria2_rpc, &config, gid, Some(&state.core.debug_logs)).await? {
        remove_task_result(
            &state.aria2_rpc,
            &config,
            gid,
            Some(&format!("{}:remove-result", operation.id)),
            Some(&state.core.debug_logs),
        )
        .await?;
        if task_exists(&state.aria2_rpc, &config, gid, Some(&state.core.debug_logs)).await? {
            return Err("task_outcome_unknown：替换任务旧 GID 尚未移除".to_string());
        }
    }
    Ok(())
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
    tasks: &mut Vec<DownloadTask>,
    operation: &mut TaskOperation,
    action: ReconcileAction,
) -> Result<(), String> {
    match action {
        ReconcileAction::Complete(message) => {
            operation.complete("startup_reconciled");
            update_task_operation(pool, operation).await?;
            emit_file_log(DebugLogLevel::Info, "runtime.operation_reconcile", &message);
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
        ReconcileAction::FinalizeRemoved(message) => {
            let Some(task) = tasks.iter_mut().find(|task| task.id == operation.task_id) else {
                operation.fail("startup_rolled_back", "回收操作对应的任务已不存在");
                update_task_operation(pool, operation).await?;
                return Ok(());
            };
            if task.status != DownloadTaskStatus::Removed {
                let source = task.status.clone();
                task.removed_from_status = Some(source);
                task.status = DownloadTaskStatus::Removed;
                task.selected_file_indexes = task
                    .files
                    .iter()
                    .filter(|file| file.selected)
                    .map(|file| file.index)
                    .collect();
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = None;
            }
            operation.complete("startup_reconciled");
            persist_download_task_state_with_operation(pool, task, operation).await?;
            emit_file_log(DebugLogLevel::Info, "runtime.operation_reconcile", &message);
        }
        ReconcileAction::FinalizeRestored(message) => {
            let Some(task) = tasks.iter_mut().find(|task| task.id == operation.task_id) else {
                operation.fail("startup_rolled_back", "恢复操作对应的任务已不存在");
                update_task_operation(pool, operation).await?;
                return Ok(());
            };
            if task.status == DownloadTaskStatus::Removed {
                let Some(source) = task.removed_from_status.clone().or_else(|| {
                    operation
                        .context
                        .task_snapshot
                        .as_ref()
                        .and_then(|snapshot| snapshot.removed_from_status.clone())
                }) else {
                    operation.require_manual_review(
                        "startup_manual_review",
                        "回收站任务缺少原始状态，未猜测恢复结果",
                    );
                    persist_download_task_state_with_operation(pool, task, operation).await?;
                    return Ok(());
                };
                task.status = if source == DownloadTaskStatus::Complete {
                    DownloadTaskStatus::Complete
                } else {
                    DownloadTaskStatus::Paused
                };
                task.removed_from_status = None;
                task.files_deleted = false;
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = None;
            }
            operation.complete("startup_reconciled");
            persist_download_task_state_with_operation(pool, task, operation).await?;
            emit_file_log(DebugLogLevel::Info, "runtime.operation_reconcile", &message);
        }
        ReconcileAction::FinalizePermanentDelete(message) => {
            operation.complete("record_deleted");
            if !delete_download_task_record_with_operation(pool, operation.task_id, operation)
                .await?
            {
                operation.require_manual_review(
                    "startup_manual_review",
                    "永久删除操作对应的数据库记录不存在，已保留操作记录",
                );
                update_task_operation(pool, operation).await?;
                return Ok(());
            }
            tasks.retain(|task| task.id != operation.task_id);
            emit_file_log(DebugLogLevel::Info, "runtime.operation_reconcile", &message);
        }
        ReconcileAction::RemoveUnpersistedAria2Task(_) => {
            return Err("未持久化 Aria2 任务尚未完成撤销".to_string());
        }
        ReconcileAction::ReleaseReplacedAria2Task(_) => {
            return Err("替换任务旧 GID 尚未完成释放".to_string());
        }
    }
    Ok(())
}

fn operation_gids(operation: &TaskOperation) -> impl Iterator<Item = &str> {
    [
        operation.context.old_gid.as_deref(),
        operation.context.new_gid.as_deref(),
    ]
    .into_iter()
    .flatten()
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
