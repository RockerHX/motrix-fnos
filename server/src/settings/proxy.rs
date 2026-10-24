use crate::aria2::Aria2RpcClient;
use crate::config::aria2::Aria2Config;
use crate::database::settings::{
    delete_download_proxy_config_for_engine_if_unused, get_download_proxy_config,
    replace_download_proxy_config, BtRuntimeApplyStatus, DeleteDownloadProxyConfigResult,
    StoredDownloadProxyConfig,
};
use crate::debug_logs::DebugLogStore;
use crate::runtime::{Aria2LifecycleCoordinator, Aria2LifecyclePhase};
use crate::tasks::repository::{SqliteTaskRepository, TaskRepository};
use crate::tasks::service::TaskService;
use crate::tasks::{
    change_task_options, DownloadTaskStatus, TaskMemoryState, TaskProxyBinding, TaskProxySource,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use url::Url;

const MAX_PROXY_URL_BYTES: usize = 2048;
const SUPPORTED_PROXY_SCHEMES: &[&str] = &["http", "https", "socks4", "socks5"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProxyStatus {
    pub configured: bool,
    pub masked_proxy_url: Option<String>,
    pub revision: u64,
    pub bt_use_proxy: bool,
    pub bt_runtime_apply: BtRuntimeApplyStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProxyApplyFailure {
    pub task_id: u64,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProxyMutationResponse {
    pub status: DownloadProxyStatus,
    pub applied_task_ids: Vec<u64>,
    pub deferred_task_ids: Vec<u64>,
    pub failed: Vec<DownloadProxyApplyFailure>,
}

#[derive(Debug)]
pub enum DownloadProxyServiceError {
    InvalidUrl(&'static str),
    BtProxyNotConfigured,
    BtProxyProtocolUnsupported,
    BtProxyEngineUnsupported,
    InUse,
    Load(String),
    Save(String),
    State(String),
}

pub struct DownloadProxyServiceContext<'a> {
    pub pool: &'a SqlitePool,
    pub tasks: &'a TaskMemoryState,
    pub aria2_lifecycle: &'a Arc<Aria2LifecycleCoordinator>,
    pub aria2_rpc: &'a Aria2RpcClient,
    pub aria2_config: Option<Aria2Config>,
    pub debug_logs: &'a DebugLogStore,
    pub update_lock: &'a Mutex<()>,
    pub task_service: Option<&'a TaskService<'a>>,
}

pub async fn load_download_proxy_status(
    pool: &SqlitePool,
) -> Result<DownloadProxyStatus, DownloadProxyServiceError> {
    let config = get_download_proxy_config(pool)
        .await
        .map_err(DownloadProxyServiceError::Load)?;
    config
        .as_ref()
        .map(status_from_stored)
        .transpose()
        .map(|status| status.unwrap_or_else(unconfigured_status))
}

pub async fn update_download_proxy(
    context: DownloadProxyServiceContext<'_>,
    proxy_url: Option<&str>,
    bt_use_proxy: Option<bool>,
) -> Result<DownloadProxyMutationResponse, DownloadProxyServiceError> {
    let normalized = proxy_url.map(normalize_proxy_url).transpose()?;
    let _update_guard = context.update_lock.lock().await;
    let current = get_download_proxy_config(context.pool)
        .await
        .map_err(DownloadProxyServiceError::Load)?;
    let current_proxy = current.as_ref().and_then(|config| config.proxy_url.clone());
    let current_bt_use_proxy = current.as_ref().is_some_and(|config| config.bt_use_proxy);
    let target_proxy = normalized.clone().or(current_proxy.clone());
    let target_bt_use_proxy = bt_use_proxy.unwrap_or(current_bt_use_proxy);
    resolve_bt_proxy_url(target_proxy.as_deref(), target_bt_use_proxy)?;

    let proxy_changed = current_proxy != target_proxy;
    let bt_configuration_changed = current_bt_use_proxy != target_bt_use_proxy;
    let bt_proxy_changed = (current_bt_use_proxy || target_bt_use_proxy)
        && (proxy_changed || bt_configuration_changed);
    let phase = context
        .aria2_lifecycle
        .snapshot()
        .map_err(DownloadProxyServiceError::State)?
        .phase;
    if target_bt_use_proxy
        && phase == Aria2LifecyclePhase::Ready
        && context.aria2_config.as_ref().is_some_and(|config| {
            config.engine_generation() != crate::config::aria2::EngineGeneration::NativeV2
        })
    {
        return Err(DownloadProxyServiceError::BtProxyEngineUnsupported);
    }
    let mut previously_active_bt = Vec::new();
    let mut runtime_activity = None;
    let mut runtime_apply = if phase == Aria2LifecyclePhase::Ready {
        current
            .as_ref()
            .map_or(BtRuntimeApplyStatus::Applied, |config| {
                config.bt_runtime_apply
            })
    } else {
        BtRuntimeApplyStatus::Deferred
    };

    if bt_proxy_changed {
        match phase {
            Aria2LifecyclePhase::Stopped => runtime_apply = BtRuntimeApplyStatus::Deferred,
            Aria2LifecyclePhase::Ready => {
                runtime_activity = Some(
                    context
                        .aria2_lifecycle
                        .acquire_activity()
                        .map_err(DownloadProxyServiceError::State)?,
                );
                if context
                    .aria2_lifecycle
                    .snapshot()
                    .map_err(DownloadProxyServiceError::State)?
                    .phase
                    != Aria2LifecyclePhase::Ready
                {
                    return Err(DownloadProxyServiceError::State(
                        "Aria2 正在切换运行状态，请稍后重试".to_string(),
                    ));
                }
                let config = context.aria2_config.as_ref().ok_or_else(|| {
                    DownloadProxyServiceError::State("缺少 Aria2 运行配置".to_string())
                })?;
                let service = context.task_service.ok_or_else(|| {
                    DownloadProxyServiceError::State("BT 代理切换缺少任务服务".to_string())
                })?;
                previously_active_bt = service
                    .pause_active_bt_tasks_for_proxy_change(config)
                    .await
                    .map_err(DownloadProxyServiceError::State)?;
                runtime_apply = BtRuntimeApplyStatus::Deferred;
            }
            Aria2LifecyclePhase::Faulted => {
                return Err(DownloadProxyServiceError::State(
                    "Aria2 运行状态异常，无法安全切换 BT 代理".to_string(),
                ));
            }
            Aria2LifecyclePhase::Starting
            | Aria2LifecyclePhase::Quiescing
            | Aria2LifecyclePhase::Stopping => {
                return Err(DownloadProxyServiceError::State(
                    "Aria2 正在切换运行状态，请稍后重试".to_string(),
                ));
            }
        }
    }

    let replaced = replace_download_proxy_config(
        context.pool,
        normalized,
        bt_use_proxy,
        runtime_apply,
        current_timestamp_ms(),
    )
    .await
    .map_err(DownloadProxyServiceError::Save)?;
    let mut status = status_from_stored(&replaced.config)?;
    if !replaced.changed {
        return Ok(DownloadProxyMutationResponse {
            status,
            applied_task_ids: Vec::new(),
            deferred_task_ids: Vec::new(),
            failed: Vec::new(),
        });
    }

    let mut bt_blocked = false;
    if bt_proxy_changed && phase == Aria2LifecyclePhase::Ready {
        let config = context.aria2_config.as_ref().expect("ready config checked");
        runtime_apply = apply_bt_proxy_config(
            &SqliteTaskRepository::for_engine(context.pool, config.engine_generation()),
            context.aria2_rpc,
            config,
            Some(&replaced.config),
        )
        .await
        .map_err(DownloadProxyServiceError::Save)?;
        bt_blocked = runtime_apply == BtRuntimeApplyStatus::Blocked;
        if bt_blocked {
            context.debug_logs.error(
                "settings.proxy",
                "BT 全局代理应用或核验失败，BT 任务保持暂停",
            );
        }
        status.bt_runtime_apply = runtime_apply;
    }

    let candidates = if proxy_changed {
        context
            .tasks
            .with_tasks_mut(|tasks| {
                let mut candidates = Vec::new();
                for task in tasks {
                    if task.source_type != crate::tasks::DownloadTaskSourceType::Url
                        || task.proxy_binding.source() != TaskProxySource::Profile
                    {
                        continue;
                    }
                    task.proxy_binding = TaskProxyBinding::profile(target_proxy.clone());
                    if task.use_proxy {
                        candidates.push(ProxyApplyCandidate {
                            task_id: task.id,
                            gid: task.gid.clone(),
                            status: task.status.clone(),
                        });
                    }
                }
                candidates
            })
            .map_err(DownloadProxyServiceError::State)?
    } else {
        Vec::new()
    };
    let mut response = if let Some(target_proxy) = target_proxy.as_deref() {
        apply_proxy_to_candidates(&context, target_proxy, candidates).await
    } else {
        DownloadProxyMutationResponse {
            status: status.clone(),
            applied_task_ids: Vec::new(),
            deferred_task_ids: Vec::new(),
            failed: Vec::new(),
        }
    };
    response.status = status.clone();
    context
        .debug_logs
        .info("settings.proxy", "下载代理配置已更新");
    drop(runtime_activity);
    drop(_update_guard);
    if !bt_blocked {
        if let (Some(service), Some(config)) = (context.task_service, context.aria2_config.as_ref())
        {
            for task_id in previously_active_bt {
                if let Err(error) = service.resume_download_task(config, task_id).await {
                    response.failed.push(DownloadProxyApplyFailure {
                        task_id,
                        code: "bt_task_resume_failed".to_string(),
                        message: "代理已应用，但任务未能恢复；可手动继续".to_string(),
                    });
                    context.debug_logs.warn(
                        "settings.proxy",
                        format!("BT 代理切换后恢复任务失败，任务 ID {}：{}", task_id, error),
                    );
                }
            }
            response.failed.sort_by_key(|failure| failure.task_id);
        }
    }
    Ok(response)
}

pub async fn delete_download_proxy(
    pool: &SqlitePool,
    tasks: &TaskMemoryState,
    update_lock: &Mutex<()>,
    debug_logs: &DebugLogStore,
) -> Result<(), DownloadProxyServiceError> {
    delete_download_proxy_for_engine(pool, tasks, update_lock, debug_logs, None).await
}

pub(crate) async fn delete_download_proxy_for_engine(
    pool: &SqlitePool,
    tasks: &TaskMemoryState,
    update_lock: &Mutex<()>,
    debug_logs: &DebugLogStore,
    generation: Option<crate::config::aria2::EngineGeneration>,
) -> Result<(), DownloadProxyServiceError> {
    let _update_guard = update_lock.lock().await;
    match delete_download_proxy_config_for_engine_if_unused(pool, generation)
        .await
        .map_err(DownloadProxyServiceError::Save)?
    {
        DeleteDownloadProxyConfigResult::InUse => return Err(DownloadProxyServiceError::InUse),
        DeleteDownloadProxyConfigResult::Deleted => {}
    }
    tasks
        .with_tasks_mut(|tasks| {
            for task in tasks {
                if generation.is_none_or(|generation| task.engine_generation == generation)
                    && task.source_type == crate::tasks::DownloadTaskSourceType::Url
                    && task.proxy_binding.source() == TaskProxySource::Profile
                {
                    task.proxy_binding = TaskProxyBinding::profile(None);
                }
            }
        })
        .map_err(DownloadProxyServiceError::State)?;
    debug_logs.info("settings.proxy", "下载代理配置已清除");
    Ok(())
}

pub fn normalize_proxy_url(value: &str) -> Result<String, DownloadProxyServiceError> {
    if value.len() > MAX_PROXY_URL_BYTES {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理地址不能超过 2048 字节",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理地址不能包含控制字符",
        ));
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(DownloadProxyServiceError::InvalidUrl("代理地址不能为空"));
    }
    let mut parsed = Url::parse(value)
        .map_err(|_| DownloadProxyServiceError::InvalidUrl("代理地址必须是完整且合法的 URL"))?;
    if !SUPPORTED_PROXY_SCHEMES.contains(&parsed.scheme()) {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理协议只支持 HTTP、HTTPS、SOCKS4 或 SOCKS5",
        ));
    }
    let authority = value
        .split_once(':')
        .map(|(_, remainder)| remainder)
        .unwrap_or_default();
    if !authority.starts_with("//") || authority[2..].starts_with('/') {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理地址必须包含合法主机名",
        ));
    }
    let Some(host) = parsed.host_str().filter(|host| !host.is_empty()) else {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理地址必须包含主机名",
        ));
    };
    let normalized_host = host.to_ascii_lowercase();
    parsed
        .set_host(Some(&normalized_host))
        .map_err(|_| DownloadProxyServiceError::InvalidUrl("代理地址必须包含合法主机名"))?;
    if parsed.port() == Some(0) {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理端口必须在 1 到 65535 之间",
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(DownloadProxyServiceError::InvalidUrl(
            "代理地址不能包含 query 或 fragment",
        ));
    }
    Ok(parsed.to_string())
}

pub(crate) fn resolve_bt_proxy_url(
    proxy_url: Option<&str>,
    bt_use_proxy: bool,
) -> Result<Option<String>, DownloadProxyServiceError> {
    if !bt_use_proxy {
        return Ok(None);
    }
    let proxy_url =
        normalize_proxy_url(proxy_url.ok_or(DownloadProxyServiceError::BtProxyNotConfigured)?)?;
    if proxy_url.starts_with("https://") {
        return Err(DownloadProxyServiceError::BtProxyProtocolUnsupported);
    }
    Ok(Some(proxy_url))
}

pub(crate) async fn apply_bt_proxy_config(
    repository: &dyn TaskRepository,
    client: &Aria2RpcClient,
    config: &Aria2Config,
    proxy: Option<&StoredDownloadProxyConfig>,
) -> Result<BtRuntimeApplyStatus, String> {
    let applied = match resolve_bt_proxy_url(
        proxy.and_then(|proxy| proxy.proxy_url.as_deref()),
        proxy.is_some_and(|proxy| proxy.bt_use_proxy),
    ) {
        Ok(target) => crate::aria2::apply_bt_proxy(client, config, target.as_deref())
            .await
            .is_ok(),
        Err(_) => false,
    };
    let status = if applied {
        BtRuntimeApplyStatus::Applied
    } else {
        BtRuntimeApplyStatus::Blocked
    };
    repository.set_bt_proxy_runtime_apply_status(status).await?;
    Ok(status)
}

fn status_from_stored(
    config: &StoredDownloadProxyConfig,
) -> Result<DownloadProxyStatus, DownloadProxyServiceError> {
    let normalized = config
        .proxy_url
        .as_deref()
        .map(normalize_proxy_url)
        .transpose()
        .map_err(|_| DownloadProxyServiceError::Load("已保存的下载代理配置无效".to_string()))?;
    Ok(DownloadProxyStatus {
        configured: normalized.is_some(),
        masked_proxy_url: normalized.as_deref().map(mask_proxy_url).transpose()?,
        revision: config.revision,
        bt_use_proxy: config.bt_use_proxy,
        bt_runtime_apply: config.bt_runtime_apply,
    })
}

fn mask_proxy_url(value: &str) -> Result<String, DownloadProxyServiceError> {
    const USER_MASK: &str = "PROXYUSERMASK";
    const PASSWORD_MASK: &str = "PROXYPASSWORDMASK";
    let mut parsed = Url::parse(value)
        .map_err(|_| DownloadProxyServiceError::Load("已保存的下载代理配置无效".to_string()))?;
    let has_password = parsed.password().is_some();
    if !parsed.username().is_empty() || has_password {
        parsed
            .set_username(USER_MASK)
            .map_err(|_| DownloadProxyServiceError::Load("下载代理凭据脱敏失败".to_string()))?;
    }
    if has_password {
        parsed
            .set_password(Some(PASSWORD_MASK))
            .map_err(|_| DownloadProxyServiceError::Load("下载代理凭据脱敏失败".to_string()))?;
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed
        .to_string()
        .replacen(USER_MASK, "***", 1)
        .replacen(PASSWORD_MASK, "***", 1))
}

fn unconfigured_status() -> DownloadProxyStatus {
    DownloadProxyStatus {
        configured: false,
        masked_proxy_url: None,
        revision: 0,
        bt_use_proxy: false,
        bt_runtime_apply: BtRuntimeApplyStatus::Deferred,
    }
}

#[derive(Clone)]
struct ProxyApplyCandidate {
    task_id: u64,
    gid: Option<String>,
    status: DownloadTaskStatus,
}

async fn apply_proxy_to_candidates(
    context: &DownloadProxyServiceContext<'_>,
    proxy_url: &str,
    candidates: Vec<ProxyApplyCandidate>,
) -> DownloadProxyMutationResponse {
    let mut response = DownloadProxyMutationResponse {
        status: unconfigured_status(),
        applied_task_ids: Vec::new(),
        deferred_task_ids: Vec::new(),
        failed: Vec::new(),
    };
    for candidate in candidates {
        let active_gid = matches!(
            candidate.status,
            DownloadTaskStatus::Pending | DownloadTaskStatus::Active
        )
        .then(|| candidate.gid.as_deref())
        .flatten()
        .filter(|gid| !gid.trim().is_empty());
        let Some(gid) = active_gid else {
            response.deferred_task_ids.push(candidate.task_id);
            continue;
        };
        let phase = context
            .aria2_lifecycle
            .snapshot()
            .map(|snapshot| snapshot.phase);
        match phase {
            Ok(Aria2LifecyclePhase::Stopped | Aria2LifecyclePhase::Faulted) => {
                response.deferred_task_ids.push(candidate.task_id);
            }
            Ok(Aria2LifecyclePhase::Ready) if context.aria2_config.is_some() => {
                let mut options = serde_json::Map::new();
                options.insert(
                    "all-proxy".to_string(),
                    serde_json::Value::String(proxy_url.to_string()),
                );
                let result = change_task_options(
                    context.aria2_rpc,
                    context.aria2_config.as_ref().expect("checked aria2 config"),
                    gid,
                    options,
                    None,
                )
                .await;
                if result.is_ok() {
                    response.applied_task_ids.push(candidate.task_id);
                } else {
                    let code = context
                        .aria2_lifecycle
                        .snapshot()
                        .ok()
                        .filter(|snapshot| snapshot.phase != Aria2LifecyclePhase::Ready)
                        .map(|_| "runtime_transition")
                        .unwrap_or("proxy_apply_failed");
                    response.failed.push(DownloadProxyApplyFailure {
                        task_id: candidate.task_id,
                        code: code.to_string(),
                        message: if code == "runtime_transition" {
                            "Aria2 正在切换运行状态，任务代理将在下次运行时重试".to_string()
                        } else {
                            "未能即时应用任务代理，任务将在下次运行时重试".to_string()
                        },
                    });
                    context.debug_logs.warn(
                        "settings.proxy",
                        format!(
                            "即时应用任务代理失败，任务 ID {}，错误码 {}",
                            candidate.task_id, code
                        ),
                    );
                }
            }
            Ok(Aria2LifecyclePhase::Ready) => {
                response.deferred_task_ids.push(candidate.task_id);
            }
            _ => response.failed.push(DownloadProxyApplyFailure {
                task_id: candidate.task_id,
                code: "runtime_transition".to_string(),
                message: "Aria2 正在切换运行状态，任务代理将在下次运行时重试".to_string(),
            }),
        }
    }
    response.applied_task_ids.sort_unstable();
    response.deferred_task_ids.sort_unstable();
    response.failed.sort_by_key(|failure| failure.task_id);
    response
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
