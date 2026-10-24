use crate::app::HttpAppState;
use crate::aria2::save_session;
use crate::database::tasks::persist_download_task_states;
use crate::tasks::{
    list_tasks, mark_unfinished_tasks_paused, pause_task, refresh_tasks_from_aria2,
    should_pause_task_on_exit,
};
use std::sync::Arc;
use tokio::time::{timeout_at, Duration, Instant};

pub(crate) const SHUTDOWN_TOTAL_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn run_shutdown_cleanup(state: &Arc<HttpAppState>) {
    let deadline = Instant::now() + SHUTDOWN_TOTAL_TIMEOUT;
    let _ = run_shutdown_cleanup_until(state, deadline).await;
}

pub(crate) async fn run_shutdown_cleanup_until(
    state: &Arc<HttpAppState>,
    deadline: Instant,
) -> bool {
    if state.ensure_download_available().is_err() {
        return run_shutdown_stage(state, deadline, "停止受限管理引擎", async {
            super::aria2_process::stop_aria2_after_shutdown(state, deadline)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .await
            == Some(true);
    }
    state
        .core
        .debug_logs
        .info("runtime.exit", "开始执行统一退出流程");
    let mut succeeded = true;
    // 失败仍尝试有界收尾，但总结果不能把保存失败当成成功。
    let Some(result) = run_shutdown_stage(
        state,
        deadline,
        "同步任务状态",
        sync_tasks_before_exit(state),
    )
    .await
    else {
        return false;
    };
    succeeded &= result;
    let Some(result) = run_shutdown_stage(
        state,
        deadline,
        "暂停并持久化未完成任务",
        pause_unfinished_tasks_before_exit(state),
    )
    .await
    else {
        return false;
    };
    succeeded &= result;
    let Some(result) = run_shutdown_stage(
        state,
        deadline,
        "保存 Aria2 session",
        save_aria2_session_before_exit(state),
    )
    .await
    else {
        return false;
    };
    succeeded &= result;
    let stopped = run_shutdown_stage(state, deadline, "停止 Aria2", async {
        super::aria2_process::stop_aria2_after_shutdown(state, deadline)
            .await
            .map(|status| {
                state.core.debug_logs.info(
                    "runtime.exit",
                    format!("退出流程已停止 Aria2：{}", status.message),
                )
            })
            .map_err(|error| {
                format!("退出流程停止 Aria2 失败，将保留运行态记录供下次启动清理：{error}")
            })
    })
    .await;
    succeeded && stopped == Some(true)
}

async fn run_shutdown_stage<F>(
    state: &Arc<HttpAppState>,
    deadline: Instant,
    stage: &str,
    operation: F,
) -> Option<bool>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    match timeout_at(deadline, operation).await {
        Ok(Ok(())) => Some(true),
        Ok(Err(error)) => {
            state.core.debug_logs.warn("runtime.exit", error);
            Some(false)
        }
        Err(_) => {
            state.core.debug_logs.warn(
                "runtime.exit",
                format!("退出总预算耗尽，未完成阶段：{stage}"),
            );
            None
        }
    }
}

async fn sync_tasks_before_exit(state: &Arc<HttpAppState>) -> Result<(), String> {
    if state.aria2_runtime_snapshot().is_none() {
        return persist_last_known_tasks(state).await;
    }
    let config = state.aria2_config();
    match refresh_tasks_from_aria2(
        &state.core.download_tasks,
        &state.runtime.app_data_dir,
        &state.runtime.accessible_paths_path,
        &state.aria2_rpc,
        &config,
        Some(&state.core.debug_logs),
    )
    .await
    {
        Ok(tasks) => persist_compatible_tasks(state, &tasks).await,
        Err(error) => {
            persist_last_known_tasks(state).await?;
            Err(format!(
                "退出前同步 Aria2 状态失败，已保存最后快照：{error}"
            ))
        }
    }
}

async fn pause_unfinished_tasks_before_exit(state: &Arc<HttpAppState>) -> Result<(), String> {
    let tasks = list_tasks(&state.core.download_tasks)?;
    let config = state.aria2_config();
    let has_runtime = state.aria2_runtime_snapshot().is_some();
    let native = config.state_dir.is_some();
    let mut errors = Vec::new();
    if has_runtime && native {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": "motrix-fnos-exit-pause-all", "method": "aria2.forcePauseAll",
            "params": [format!("token:{}", config.rpc_secret)],
        });
        let paused = state
            .aria2_rpc
            .request::<serde_json::Value>(&config, &body)
            .await
            .and_then(|response| response.into_result())
            .map_err(|error| format!("退出前暂停所有引擎活动失败，保留最后状态：{error}"))?;
        if paused != "OK" {
            return Err("退出前无法确认引擎已暂停，保留最后状态".to_string());
        }
    } else if has_runtime {
        for task in tasks.iter().filter(|task| should_pause_task_on_exit(task)) {
            if let Some(gid) = task.gid.as_deref() {
                if let Err(error) =
                    pause_task(&state.aria2_rpc, &config, gid, Some(&state.core.debug_logs)).await
                {
                    errors.push(format!(
                        "退出前 RPC 暂停任务失败，仍会把任务保存为暂停态，ID {}：{error}",
                        task.id
                    ));
                }
            }
        }
    }
    mark_unfinished_tasks_paused(&state.core.download_tasks)?;
    persist_last_known_tasks(state).await?;
    let _ = super::task_monitor::broadcast_tasks_snapshot(state);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

async fn save_aria2_session_before_exit(state: &Arc<HttpAppState>) -> Result<(), String> {
    if state.aria2_runtime_snapshot().is_none() {
        return Ok(());
    }
    save_session(
        &state.aria2_rpc,
        &state.aria2_config(),
        Some(&state.core.debug_logs),
    )
    .await
    .map_err(|error| format!("退出前保存 Aria2 session 失败，继续退出：{error}"))
}

async fn persist_last_known_tasks(state: &Arc<HttpAppState>) -> Result<(), String> {
    let tasks = list_tasks(&state.core.download_tasks)?;
    persist_compatible_tasks(state, &tasks).await
}

async fn persist_compatible_tasks(
    state: &Arc<HttpAppState>,
    tasks: &[crate::tasks::DownloadTask],
) -> Result<(), String> {
    let generation = state.aria2_config().engine_generation();
    let compatible = tasks
        .iter()
        .filter(|task| task.engine_generation == generation)
        .cloned()
        .collect::<Vec<_>>();
    persist_download_task_states(&state.core.database.pool, &compatible)
        .await
        .map_err(|error| format!("退出前保存最后已知任务状态失败：{error}"))
}

#[cfg(test)]
mod tests;
