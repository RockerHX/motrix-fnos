use crate::tasks::CreateTaskAdvancedOptions;
use serde_json::{Map, Value};

const PASSTHROUGH_OPTIONS: &[&str] = &[
    "allow-overwrite",
    "auto-file-renaming",
    "check-certificate",
    "connect-timeout",
    "continue",
    "header",
    "lowest-speed-limit",
    "max-connection-per-server",
    "max-download-limit",
    "max-file-not-found",
    "max-tries",
    "min-split-size",
    "referer",
    "retry-wait",
    "split",
    "timeout",
    "user-agent",
    "all-proxy",
];

pub fn sanitize_create_task_options(
    advanced_options: &CreateTaskAdvancedOptions,
    aria2_options: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut options = sanitize_aria2_options(aria2_options);

    if let Some(connections) = advanced_options.connections {
        if !(1..=64).contains(&connections) {
            return Err("连接数必须在 1 到 64 之间".to_string());
        }
        let value = Value::String(connections.to_string());
        options.insert("split".to_string(), value.clone());
        options.insert("max-connection-per-server".to_string(), value);
    }

    if let Some(download_limit_kb) = advanced_options.download_limit_kb {
        if download_limit_kb > 0 {
            let bytes = download_limit_kb
                .checked_mul(1024)
                .ok_or_else(|| "下载限速超出支持范围".to_string())?;
            options.insert(
                "max-download-limit".to_string(),
                Value::String(bytes.to_string()),
            );
        }
    }

    if let Some(proxy) = advanced_options.proxy.as_deref().map(str::trim) {
        if proxy.is_empty() {
            return Err("代理地址不能为空".to_string());
        }
        validate_proxy(proxy)?;
        options.insert("all-proxy".to_string(), Value::String(proxy.to_string()));
    }

    Ok(options)
}

pub fn sanitize_aria2_options(options: &Map<String, Value>) -> Map<String, Value> {
    options
        .iter()
        .filter(|(key, _)| PASSTHROUGH_OPTIONS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            normalize_aria2_option_value(value).map(|value| (key.clone(), value))
        })
        .collect()
}

fn normalize_aria2_option_value(value: &Value) -> Option<Value> {
    match value {
        Value::Null | Value::Object(_) => None,
        Value::String(value) => {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(Value::String(value.to_string()))
            }
        }
        Value::Array(items) => {
            let normalized = items
                .iter()
                .filter_map(normalize_aria2_option_value)
                .collect::<Vec<_>>();
            if normalized.is_empty() {
                None
            } else {
                Some(Value::Array(normalized))
            }
        }
        Value::Bool(_) | Value::Number(_) => Some(value.clone()),
    }
}

fn validate_proxy(proxy: &str) -> Result<(), String> {
    let lower = proxy.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("socks5://")
        || lower.starts_with("socks4://")
    {
        Ok(())
    } else {
        Err("代理地址必须以 http://、https://、socks5:// 或 socks4:// 开头".to_string())
    }
}
