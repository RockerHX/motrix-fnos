use super::TaskService;
use crate::settings::proxy::{normalize_proxy_url, DownloadProxyServiceError};
use crate::tasks::{CreateTaskAdvancedOptions, DownloadTask, TaskProxyBinding, TaskProxySource};
use serde_json::{Map, Value};

pub(super) const PROXY_CONFLICT_MESSAGE: &str = "代理选择冲突：useProxy 不能与原始代理字段同时使用";
pub(super) const PROXY_NOT_CONFIGURED_MESSAGE: &str = "未配置下载代理，请先在设置中保存代理地址";

pub(super) struct ResolvedTaskProxy {
    pub(super) use_proxy: bool,
    pub(super) binding: TaskProxyBinding,
}

impl TaskService<'_> {
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
            TaskProxySource::Profile => {
                let config = self
                    .repository
                    .get_download_proxy_config()
                    .await?
                    .ok_or_else(|| PROXY_NOT_CONFIGURED_MESSAGE.to_string())?;
                Ok(TaskProxyBinding::profile(Some(normalize_stored_proxy(
                    &config.proxy_url,
                )?)))
            }
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
