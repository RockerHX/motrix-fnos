use crate::app::HttpAppState;
use crate::database::task_operations::{list_unfinished_task_operations, update_task_operation};
use crate::tasks::files::cleanup_staged_task_file_path;
use crate::tasks::TaskOperation;
use std::collections::HashSet;
use std::sync::Arc;

pub(crate) fn spawn_file_cleanup_worker(state: Arc<HttpAppState>) {
    if !state.try_start_file_cleanup_worker() {
        return;
    }

    tokio::spawn(async move {
        if let Err(error) = run_file_cleanup_once(&state).await {
            state.core.debug_logs.warn(
                "tasks.file_cleanup",
                format!("后台任务文件清理未完成：{}", error),
            );
        }
        state.finish_file_cleanup_worker();
    });
}

pub(crate) async fn run_file_cleanup_once(state: &HttpAppState) -> Result<(), String> {
    let mut attempted = HashSet::new();
    let mut first_error = None;

    loop {
        let operations = list_unfinished_task_operations(&state.core.database.pool).await?;
        let pending = operations
            .into_iter()
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
                    format!("任务文件清理暂未完成，将在下次启动或操作时重试：{}", error),
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
        operation.complete("completed");
        update_task_operation(&state.core.database.pool, &operation).await?;
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
