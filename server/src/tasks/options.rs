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
    "stream-max-connections",
    "max-download-limit",
    "max-file-not-found",
    "max-tries",
    "referer",
    "retry-wait",
    "timeout",
    "user-agent",
    "all-proxy",
    "bt-tracker",
];

pub fn sanitize_create_task_options(
    advanced_options: &CreateTaskAdvancedOptions,
    aria2_options: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    if advanced_options.proxy.is_some() || aria2_options.contains_key("all-proxy") {
        return Err("代理选择尚未解析".to_string());
    }
    let mut options = sanitize_aria2_options(aria2_options)?;

    if let Some(connections) = advanced_options.connections {
        if !(1..=64).contains(&connections) {
            return Err("task_connections_invalid：连接数必须在 1 到 64 之间".to_string());
        }
        let value = Value::String(connections.to_string());
        options.insert("stream-max-connections".to_string(), value);
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

    Ok(options)
}

pub fn sanitize_aria2_options(options: &Map<String, Value>) -> Result<Map<String, Value>, String> {
    for key in options.keys() {
        if matches!(
            key.as_str(),
            "min-split-size" | "bt-save-metadata" | "dht-file-path"
        ) || (key == "media" && options[key].as_str() != Some("file"))
        {
            return Err(format!(
                "aria2_option_unsupported：选项 {key} 已失效或不支持"
            ));
        }
        if key.starts_with("rpc-")
            || matches!(
                key.as_str(),
                "state-dir"
                    | "input-file"
                    | "save-session"
                    | "save-session-interval"
                    | "pause"
                    | "pause-metadata"
                    | "pause-on-startup"
                    | "gid"
                    | "daemon"
                    | "enable-rpc"
                    | "stop"
                    | "stop-with-process"
                    | "log"
                    | "log-level"
                    | "conf-path"
                    | "no-conf"
                    | "dht-file-path6"
            )
        {
            return Err(format!("aria2_option_forbidden：选项 {key} 由服务管理"));
        }
    }
    if options.contains_key("stream-max-connections")
        && (options.contains_key("split") || options.contains_key("max-connection-per-server"))
    {
        return Err("aria2_option_conflict：不能同时指定新旧 HTTP 连接参数".to_string());
    }
    let mut connections = None;
    for key in [
        "stream-max-connections",
        "split",
        "max-connection-per-server",
    ] {
        if let Some(value) = options.get(key) {
            let value = value
                .as_u64()
                .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
                .filter(|value| (1..=64).contains(value))
                .ok_or("task_connections_invalid：连接数必须是 1 到 64 的整数")?;
            connections = Some(connections.map_or(value, |previous: u64| previous.min(value)));
        }
    }
    let mut sanitized: Map<String, Value> = options
        .iter()
        .filter(|(key, _)| PASSTHROUGH_OPTIONS.contains(&key.as_str()))
        .filter_map(|(key, value)| {
            normalize_aria2_option_value(value).map(|value| (key.clone(), value))
        })
        .collect();
    if let Some(connections) = connections {
        sanitized.insert(
            "stream-max-connections".to_string(),
            Value::String(connections.to_string()),
        );
    }
    Ok(sanitized)
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

#[cfg(test)]
mod tests;
