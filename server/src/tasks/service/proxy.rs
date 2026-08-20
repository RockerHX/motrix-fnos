use super::TaskService;
use crate::config::aria2::Aria2Config;
use crate::runtime::Aria2LifecyclePhase;
use crate::settings::proxy::{normalize_proxy_url, DownloadProxyServiceError};
use crate::tasks::{
    change_task_options_with_request_id, replace_task_snapshot, task_snapshot,
    update_task_proxy_state, Aria2TaskOptionError, CreateTaskAdvancedOptions, DownloadTask,
    DownloadTaskStatus, TaskOperationType, TaskProxyBinding, TaskProxySource,
};
use serde_json::{Map, Value};

pub(super) const PROXY_CONFLICT_MESSAGE: &str = "代理选择冲突：useProxy 不能与原始代理字段同时使用";
pub(super) const PROXY_NOT_CONFIGURED_MESSAGE: &str = "未配置下载代理，请先在设置中保存代理地址";

pub(super) struct ResolvedTaskProxy {
    pub(super) use_proxy: bool,
    pub(super) binding: TaskProxyBinding,
}

impl TaskService<'_> {
    pub async fn update_download_task_proxy(
        &self,
        config: Option<&Aria2Config>,
        task_id: u64,
        enabled: bool,
    ) -> Result<DownloadTask, String> {
        self.ensure_not_exiting()?;
        let _task_operation_guard = self.download_tasks.begin_operation(task_id)?;
        let _proxy_update_guard = self.proxy_update_lock.lock().await;
        let snapshot = task_snapshot(self.download_tasks, task_id)?;
        if snapshot.use_proxy == enabled {
            return Ok(snapshot);
        }

        let proxy_binding = if enabled {
            self.resolve_profile_proxy_binding().await?
        } else {
            TaskProxyBinding::profile(None)
        };
        let should_sync_runtime = !matches!(
            snapshot.status,
            DownloadTaskStatus::Complete | DownloadTaskStatus::Removed
        ) && snapshot
            .gid
            .as_deref()
            .is_some_and(|gid| !gid.trim().is_empty());
        let mut runtime_activity = None;
        let runtime_config =
            if should_sync_runtime {
                match self.aria2_lifecycle.snapshot()?.phase {
                    Aria2LifecyclePhase::Stopped | Aria2LifecyclePhase::Faulted => None,
                    Aria2LifecyclePhase::Ready => {
                        runtime_activity = Some(self.aria2_lifecycle.acquire_activity()?);
                        if self.aria2_lifecycle.snapshot()?.phase != Aria2LifecyclePhase::Ready {
                            return Err("Aria2 正在切换运行状态，请稍后重试".to_string());
                        }
                        Some(config.ok_or_else(|| {
                            "Aria2 运行态正在切换，暂时无法应用任务代理".to_string()
                        })?)
                    }
                    Aria2LifecyclePhase::Starting
                    | Aria2LifecyclePhase::Quiescing
                    | Aria2LifecyclePhase::Stopping => {
                        return Err("Aria2 正在切换运行状态，请稍后重试".to_string());
                    }
                }
            } else {
                None
            };

        let mut context = super::task_operation_context(Some(snapshot.clone()), Vec::new());
        context.proxy_enabled = Some(enabled);
        let mut operation = self
            .begin_task_operation(task_id, TaskOperationType::Proxy, "prepared", context)
            .await?;

        if let Some(config) = runtime_config {
            let gid = snapshot
                .gid
                .as_deref()
                .expect("runtime proxy sync requires a checked GID");
            let target_options = proxy_options(enabled, &proxy_binding)?;
            let old_options = proxy_options(snapshot.use_proxy, &snapshot.proxy_binding)?;
            let request_id = operation.id.clone();
            match change_task_options_with_request_id(
                self.aria2_rpc,
                config,
                gid,
                target_options,
                Some(&request_id),
                Some(self.debug_logs),
            )
            .await
            {
                Ok(_) => {
                    let mut context = operation.context.clone();
                    context
                        .completed_side_effects
                        .push("proxy_option_applied".to_string());
                    operation.update_phase("proxy_applied", context);
                }
                Err(error) if error.is_outcome_unknown() => {
                    self.record_unknown_aria2_outcome(&mut operation, error.to_string())
                        .await?;
                    return Err(error.to_string());
                }
                Err(error) => {
                    self.fail_task_operation(
                        &mut operation,
                        "proxy_apply_failed",
                        error.to_string(),
                    )
                    .await;
                    return Err(error.to_string());
                }
            }

            let task =
                match update_task_proxy_state(self.download_tasks, task_id, enabled, proxy_binding)
                {
                    Ok(task) => task,
                    Err(error) => {
                        return Err(self
                            .compensate_proxy_after_persist_failure(
                                config,
                                &snapshot,
                                &mut operation,
                                old_options,
                                error,
                            )
                            .await);
                    }
                };
            if let Err(error) = self
                .persist_task_with_operation(&task, &mut operation, "proxy_state_persisted")
                .await
            {
                let memory_error =
                    replace_task_snapshot(self.download_tasks, snapshot.clone()).err();
                let reason = memory_error
                    .map(|memory_error| {
                        format!("{}；恢复内存任务状态失败：{}", error, memory_error)
                    })
                    .unwrap_or(error);
                return Err(self
                    .compensate_proxy_after_persist_failure(
                        config,
                        &snapshot,
                        &mut operation,
                        old_options,
                        reason,
                    )
                    .await);
            }
            drop(runtime_activity);
            self.complete_task_operation(&mut operation, "completed")
                .await;
            return Ok(task);
        }

        let task = update_task_proxy_state(self.download_tasks, task_id, enabled, proxy_binding)?;
        if let Err(error) = self
            .persist_task_with_operation(&task, &mut operation, "proxy_state_persisted")
            .await
        {
            return Err(self
                .rollback_task_operation_state(
                    snapshot,
                    &mut operation,
                    "task_persist_failed",
                    error,
                )
                .await);
        }
        self.complete_task_operation(&mut operation, "completed")
            .await;
        Ok(task)
    }

    pub async fn validate_create_task_proxy(
        &self,
        advanced_options: &CreateTaskAdvancedOptions,
        aria2_options: &Map<String, Value>,
    ) -> Result<(), String> {
        let mut advanced_options = advanced_options.clone();
        let mut aria2_options = aria2_options.clone();
        self.resolve_create_task_proxy(&mut advanced_options, &mut aria2_options)
            .await
            .map(|_| ())
    }

    pub(super) async fn resolve_create_task_proxy(
        &self,
        advanced_options: &mut CreateTaskAdvancedOptions,
        aria2_options: &mut Map<String, Value>,
    ) -> Result<ResolvedTaskProxy, String> {
        let advanced_proxy = take_advanced_proxy(advanced_options.proxy.take())?;
        let aria2_proxy = take_aria2_proxy(aria2_options.remove("all-proxy"))?;
        let legacy_proxy = match (advanced_proxy, aria2_proxy) {
            (Some(_), Some(_)) => return Err(PROXY_CONFLICT_MESSAGE.to_string()),
            (Some(proxy), None) | (None, Some(proxy)) => Some(proxy),
            (None, None) => None,
        };

        if advanced_options.use_proxy.is_some() && legacy_proxy.is_some() {
            return Err(PROXY_CONFLICT_MESSAGE.to_string());
        }
        if let Some(proxy_url) = legacy_proxy {
            return Ok(ResolvedTaskProxy {
                use_proxy: true,
                binding: TaskProxyBinding::override_url(proxy_url),
            });
        }
        if advanced_options.use_proxy != Some(true) {
            return Ok(ResolvedTaskProxy {
                use_proxy: false,
                binding: TaskProxyBinding::profile(None),
            });
        }

        let config = self
            .repository
            .get_download_proxy_config()
            .await?
            .ok_or_else(|| PROXY_NOT_CONFIGURED_MESSAGE.to_string())?;
        let proxy_url = normalize_stored_proxy(&config.proxy_url)?;
        Ok(ResolvedTaskProxy {
            use_proxy: true,
            binding: TaskProxyBinding::profile(Some(proxy_url)),
        })
    }

    pub(super) async fn resolve_existing_task_proxy(
        &self,
        task: &DownloadTask,
    ) -> Result<TaskProxyBinding, String> {
        if !task.use_proxy {
            return Ok(task.proxy_binding.clone());
        }
        match task.proxy_binding.source() {
            TaskProxySource::Profile => self.resolve_profile_proxy_binding().await,
            TaskProxySource::Override => {
                let proxy_url = task
                    .proxy_binding
                    .effective_proxy_url()
                    .ok_or_else(|| "兼容代理任务缺少私密代理覆盖".to_string())?;
                Ok(TaskProxyBinding::override_url(normalize_input_proxy(
                    proxy_url,
                )?))
            }
        }
    }

    pub(super) async fn resolve_recreated_task_proxy(
        &self,
        task: &DownloadTask,
        use_proxy_override: Option<bool>,
    ) -> Result<ResolvedTaskProxy, String> {
        let use_proxy = use_proxy_override.unwrap_or(task.use_proxy);
        let binding = if use_proxy == task.use_proxy {
            self.resolve_existing_task_proxy(task).await?
        } else if use_proxy {
            self.resolve_profile_proxy_binding().await?
        } else {
            TaskProxyBinding::profile(None)
        };
        Ok(ResolvedTaskProxy { use_proxy, binding })
    }

    async fn resolve_profile_proxy_binding(&self) -> Result<TaskProxyBinding, String> {
        let config = self
            .repository
            .get_download_proxy_config()
            .await?
            .ok_or_else(|| PROXY_NOT_CONFIGURED_MESSAGE.to_string())?;
        Ok(TaskProxyBinding::profile(Some(normalize_stored_proxy(
            &config.proxy_url,
        )?)))
    }

    async fn compensate_proxy_after_persist_failure(
        &self,
        config: &Aria2Config,
        snapshot: &DownloadTask,
        operation: &mut crate::tasks::TaskOperation,
        old_options: Map<String, Value>,
        persist_error: String,
    ) -> String {
        let gid = snapshot.gid.as_deref().unwrap_or_default();
        let request_id = format!("{}-compensate", operation.id);
        match change_task_options_with_request_id(
            self.aria2_rpc,
            config,
            gid,
            old_options,
            Some(&request_id),
            Some(self.debug_logs),
        )
        .await
        {
            Ok(_) => {
                let message = format!("{}；已恢复 Aria2 原代理选项", persist_error);
                self.fail_task_operation(operation, "task_persist_failed_compensated", &message)
                    .await;
                message
            }
            Err(Aria2TaskOptionError::OutcomeUnknown(error)) => {
                let message = format!(
                    "{}；恢复 Aria2 原代理选项的结果未知，等待启动对账",
                    persist_error
                );
                let mut context = operation.context.clone();
                context.proxy_enabled = Some(snapshot.use_proxy);
                context
                    .completed_side_effects
                    .push("proxy_compensation_outcome_unknown".to_string());
                operation.error_message = Some(message.clone());
                operation.update_phase("proxy_compensation_outcome_unknown", context);
                if let Err(update_error) = self.repository.update_operation(operation).await {
                    self.debug_logs.error(
                        "tasks.proxy",
                        format!(
                            "记录代理补偿结果未知失败，operationId {}：{}",
                            operation.id, update_error
                        ),
                    );
                }
                format!("{}：{}", message, error)
            }
            Err(Aria2TaskOptionError::Failed(error)) => {
                let message = format!("{}；恢复 Aria2 原代理选项失败，等待启动对账", persist_error);
                operation.require_manual_review("proxy_compensation_failed", message.clone());
                if let Err(update_error) = self.repository.update_operation(operation).await {
                    self.debug_logs.error(
                        "tasks.proxy",
                        format!(
                            "记录代理补偿失败，operationId {}：{}",
                            operation.id, update_error
                        ),
                    );
                }
                format!("{}：{}", message, error)
            }
        }
    }
}

fn proxy_options(enabled: bool, binding: &TaskProxyBinding) -> Result<Map<String, Value>, String> {
    let proxy_url = if enabled {
        binding
            .effective_proxy_url()
            .ok_or_else(|| "任务要求使用代理，但没有可用的代理配置".to_string())?
    } else {
        ""
    };
    Ok(Map::from_iter([(
        "all-proxy".to_string(),
        Value::String(proxy_url.to_string()),
    )]))
}

fn take_advanced_proxy(value: Option<String>) -> Result<Option<String>, String> {
    value.map(|proxy| normalize_input_proxy(&proxy)).transpose()
}

fn take_aria2_proxy(value: Option<Value>) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(Value::String(proxy)) if proxy.trim().is_empty() => Ok(None),
        Some(Value::String(proxy)) => normalize_input_proxy(&proxy).map(Some),
        Some(_) => Err("代理地址必须是字符串".to_string()),
    }
}

fn normalize_input_proxy(value: &str) -> Result<String, String> {
    normalize_proxy_url(value).map_err(|error| match error {
        DownloadProxyServiceError::InvalidUrl(message) => message.to_string(),
        _ => "代理地址校验失败".to_string(),
    })
}

fn normalize_stored_proxy(value: &str) -> Result<String, String> {
    normalize_proxy_url(value).map_err(|_| "已保存的下载代理配置无效，请重新保存".to_string())
}
