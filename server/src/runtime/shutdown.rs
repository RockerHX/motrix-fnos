use crate::app::HttpAppState;
use crate::aria2::save_session;
use crate::database::tasks::persist_download_task_states;
use crate::tasks::{
    list_tasks, mark_unfinished_tasks_paused, pause_task, refresh_tasks_from_aria2,
    should_pause_task_on_exit,
};
use std::sync::Arc;

pub async fn run_shutdown_cleanup(state: &Arc<HttpAppState>) {
    state
        .core
        .debug_logs
        .info("runtime.exit", "开始执行统一退出流程");

    // 顺序不可互换：先同步最新进度，再暂停并持久化未完成任务，随后保存 session，最后才停止 Aria2；各阶段失败均按最后已知状态降级继续退出。
    sync_tasks_before_exit(state).await;
    pause_unfinished_tasks_before_exit(state).await;
    save_aria2_session_before_exit(state).await;

    // 只有确认 Aria2 已停止后才能清除运行态；失败时保留 PID/端口/secret，供下次启动识别并定向清理。
    let should_clear_runtime =
        match super::aria2_process::stop_process(&state.aria2_process, &state.core.debug_logs) {
            Ok(status) => {
                state.core.debug_logs.info(
                    "runtime.exit",
                    format!("退出流程已停止 Aria2：{}", status.message),
                );
                true
            }
            Err(error) => {
                state.core.debug_logs.warn(
                    "runtime.exit",
                    format!(
                        "退出流程停止 Aria2 失败，将保留运行态记录供下次启动清理：{}",
                        error
                    ),
                );
                false
            }
        };

    if should_clear_runtime {
        state.clear_aria2_runtime();
    }
}

async fn sync_tasks_before_exit(state: &Arc<HttpAppState>) {
    if state.aria2_runtime_snapshot().is_none() {
        persist_last_known_tasks(
            state,
            "退出前未发现 Aria2 运行态，已保存应用内最后任务快照",
            "退出前保存最后已知任务状态失败",
        )
        .await;
        return;
    }

    let config = state.aria2_config();
    match refresh_tasks_from_aria2(
        &state.core.download_tasks,
        &state.runtime.app_data_dir,
        &config,
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(tasks) => {
            if let Err(error) =
                persist_download_task_states(&state.core.database.pool, &tasks).await
            {
                state.core.debug_logs.error(
                    "runtime.exit",
                    format!("退出前保存最新任务状态失败：{}", error),
                );
            } else {
                state.core.debug_logs.info(
                    "runtime.exit",
                    format!("退出前已同步并保存 {} 个任务状态", tasks.len()),
                );
            }
        }
        Err(error) => {
            state.core.debug_logs.warn(
                "runtime.exit",
                format!("退出前同步 Aria2 状态失败，将保存应用内最后状态：{}", error),
            );
            persist_last_known_tasks(
                state,
                "退出前已回退保存应用内最后任务快照",
                "退出前保存最后已知任务状态失败",
            )
            .await;
        }
    }
}

async fn pause_unfinished_tasks_before_exit(state: &Arc<HttpAppState>) {
    let candidates = match list_tasks(&state.core.download_tasks) {
        Ok(tasks) => tasks
            .into_iter()
            .filter(should_pause_task_on_exit)
            .filter_map(|task| task.gid.map(|gid| (task.id, gid)))
            .collect::<Vec<_>>(),
        Err(error) => {
            state.core.debug_logs.error(
                "runtime.exit",
                format!("退出前读取待暂停任务失败：{}", error),
            );
            return;
        }
    };

    if candidates.is_empty() {
        state
            .core
            .debug_logs
            .info("runtime.exit", "退出前没有可通过 RPC 暂停的未完成任务");
    }

    let config = state.aria2_config();
    let has_runtime = state.aria2_runtime_snapshot().is_some();
    let mut rpc_paused_count = 0;
    for (task_id, gid) in &candidates {
        if !has_runtime {
            break;
        }

        match pause_task(&config, gid, Some(&state.core.debug_logs)).await {
            Ok(_) => rpc_paused_count += 1,
            Err(error) => state.core.debug_logs.warn(
                "runtime.exit",
                format!(
                    "退出前 RPC 暂停任务失败，仍会把任务保存为暂停态，ID {}，GID {}：{}",
                    task_id, gid, error
                ),
            ),
        }
    }

    let paused_tasks = match mark_unfinished_tasks_paused(&state.core.download_tasks) {
        Ok(tasks) => tasks,
        Err(error) => {
            state.core.debug_logs.error(
                "runtime.exit",
                format!("退出前标记未完成任务暂停失败：{}", error),
            );
            return;
        }
    };

    let tasks = match list_tasks(&state.core.download_tasks) {
        Ok(tasks) => tasks,
        Err(error) => {
            state.core.debug_logs.error(
                "runtime.exit",
                format!("退出前读取暂停后任务状态失败：{}", error),
            );
            return;
        }
    };

    if let Err(error) = persist_download_task_states(&state.core.database.pool, &tasks).await {
        state.core.debug_logs.error(
            "runtime.exit",
            format!("退出前保存暂停任务状态失败：{}", error),
        );
        return;
    }

    state.core.debug_logs.info(
        "runtime.exit",
        format!(
            "退出前已保存 {} 个未完成任务为暂停态，RPC 成功暂停 {} 个",
            paused_tasks.len(),
            rpc_paused_count
        ),
    );

    if !paused_tasks.is_empty() {
        let _ = super::task_monitor::broadcast_tasks_snapshot(state);
    }
}

async fn save_aria2_session_before_exit(state: &Arc<HttpAppState>) {
    if state.aria2_runtime_snapshot().is_none() {
        state.core.debug_logs.info(
            "runtime.exit",
            "退出前未发现 Aria2 运行态，跳过 session 保存",
        );
        return;
    }

    let config = state.aria2_config();
    match save_session(&config, Some(&state.core.debug_logs)).await {
        Ok(()) => state
            .core
            .debug_logs
            .info("runtime.exit", "退出前已请求 Aria2 保存 session"),
        Err(error) => state.core.debug_logs.warn(
            "runtime.exit",
            format!("退出前保存 Aria2 session 失败，继续退出：{}", error),
        ),
    }
}

async fn persist_last_known_tasks(
    state: &Arc<HttpAppState>,
    success_message: &str,
    failure_prefix: &str,
) {
    match list_tasks(&state.core.download_tasks) {
        Ok(tasks) => {
            if let Err(error) =
                persist_download_task_states(&state.core.database.pool, &tasks).await
            {
                state
                    .core
                    .debug_logs
                    .error("runtime.exit", format!("{}：{}", failure_prefix, error));
            } else {
                state.core.debug_logs.info("runtime.exit", success_message);
            }
        }
        Err(error) => state
            .core
            .debug_logs
            .error("runtime.exit", format!("退出前读取任务快照失败：{}", error)),
    }
}

#[cfg(test)]
mod tests;
