use crate::app::HttpAppState;
use crate::database::task_operations::{
    list_unfinished_task_operations_for_engine, update_task_operation,
};
use crate::tasks::files::cleanup_staged_task_file_path;
use crate::tasks::TaskOperation;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

const FILE_CLEANUP_RETRY_DELAY: Duration = Duration::from_secs(30);

pub(crate) async fn spawn_file_cleanup_worker(state: Arc<HttpAppState>) {
    if state.ensure_download_available().is_err() {
        return;
    }
    let has_pending_cleanup = match has_pending_file_cleanup(&state).await {
        Ok(has_pending_cleanup) => has_pending_cleanup,
        Err(error) => {
            state.core.debug_logs.warn(
                "tasks.file_cleanup",
                format!(
                    "检查待清理任务文件失败，将在下次启动或操作时重试：{}",
                    error
                ),
            );
            return;
        }
    };
    if !has_pending_cleanup {
        return;
    }
    if !state.try_start_file_cleanup_worker() {
        state.notify_file_cleanup_worker();
        return;
    }

    tokio::spawn(async move {
        run_file_cleanup_worker(state).await;
    });
}

async fn run_file_cleanup_worker(state: Arc<HttpAppState>) {
    loop {
        match run_file_cleanup_once(&state).await {
            Ok(()) => {
                state.finish_file_cleanup_worker();
                match has_pending_file_cleanup(&state).await {
                    Ok(false) => return,
                    Ok(true) => {}
                    Err(error) => {
                        state.core.debug_logs.warn(
                            "tasks.file_cleanup",
                            format!("复查待清理任务文件失败，将在后台重试：{}", error),
                        );
                    }
                }

                if state.try_start_file_cleanup_worker() {
                    continue;
                }
                state.notify_file_cleanup_worker();
                return;
            }
            Err(error) => {
                state.core.debug_logs.warn(
                    "tasks.file_cleanup",
                    format!("后台任务文件清理未完成，将自动重试：{}", error),
                );
                tokio::select! {
                    _ = tokio::time::sleep(FILE_CLEANUP_RETRY_DELAY) => {}
                    _ = state.wait_for_file_cleanup_worker_notification() => {}
                }
            }
        }
    }
}

async fn has_pending_file_cleanup(state: &HttpAppState) -> Result<bool, String> {
    Ok(list_unfinished_task_operations_for_engine(
        &state.core.database.pool,
        state.aria2_config().engine_generation(),
    )
    .await?
    .into_iter()
    .filter(|operation| operation.engine_generation() == state.aria2_config().engine_generation())
    .any(|operation| operation.is_file_cleanup_pending()))
}

pub(crate) async fn run_file_cleanup_once(state: &HttpAppState) -> Result<(), String> {
    state.ensure_download_available()?;
    let mut attempted = HashSet::new();
    let mut first_error = None;

    loop {
        let operations = list_unfinished_task_operations_for_engine(
            &state.core.database.pool,
            state.aria2_config().engine_generation(),
        )
        .await?;
        let pending = operations
            .into_iter()
            .filter(|operation| {
                operation.engine_generation() == state.aria2_config().engine_generation()
            })
            .filter(|operation| operation.is_file_cleanup_pending())
            .filter(|operation| !attempted.contains(&operation.id))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }

        for operation in pending {
            attempted.insert(operation.id.clone());
            if let Err(error) = process_file_cleanup_operation(state, operation).await {
                state.core.debug_logs.warn(
                    "tasks.file_cleanup",
                    format!("任务文件清理暂未完成，将在后台重试：{}", error),
                );
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

async fn process_file_cleanup_operation(
    state: &HttpAppState,
    mut operation: TaskOperation,
) -> Result<(), String> {
    let _task_operation_guard = state
        .core
        .download_tasks
        .begin_operation(operation.task_id)?;
    let paths = cleanup_paths(&operation);
    let mut errors = Vec::new();

    for path in paths {
        let cleanup_path = path.clone();
        let task_id = operation.task_id;
        let result = tokio::task::spawn_blocking(move || {
            cleanup_staged_task_file_path(task_id, &cleanup_path)
        })
        .await
        .map_err(|error| format!("任务文件清理线程异常退出：{}", error))?;
        if let Err(error) = result {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        let mut context = operation.context.clone();
        if !context
            .completed_side_effects
            .iter()
            .any(|effect| effect == "task_files_deleted")
        {
            context
                .completed_side_effects
                .push("task_files_deleted".to_string());
        }
        operation.context = context;
        if operation.operation_type == crate::tasks::TaskOperationType::PermanentDelete {
            let task = operation
                .context
                .task_snapshot
                .clone()
                .ok_or_else(|| "永久删除清理缺少任务快照，已保留任务记录".to_string())?;
            if let Err(error) = crate::api::build_task_service(state)
                .finish_permanent_delete(&task, &mut operation)
                .await
            {
                operation.retain_file_cleanup_pending(error.clone());
                update_task_operation(&state.core.database.pool, &operation).await?;
                return Err(error);
            }
        } else {
            operation.complete("completed");
            update_task_operation(&state.core.database.pool, &operation).await?;
        }
        return Ok(());
    }

    let message = errors.join("；");
    operation.retain_file_cleanup_pending(message.clone());
    update_task_operation(&state.core.database.pool, &operation).await?;
    Err(message)
}

fn cleanup_paths(operation: &TaskOperation) -> Vec<String> {
    if !operation.context.file_cleanup_paths.is_empty() {
        return operation.context.file_cleanup_paths.clone();
    }

    // 1.8.x 曾把暂存目录只写入 criticalPaths；兼容这类未完成记录，仍按严格目录名校验。
    operation
        .context
        .critical_paths
        .iter()
        .filter(|path| {
            std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with(".motrix-redownload-backup-"))
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
