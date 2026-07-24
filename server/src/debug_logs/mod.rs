use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing_subscriber::EnvFilter;

pub const DEFAULT_DEBUG_LOG_CAPACITY: usize = 500;
pub const LOG_FILTER_ENV: &str = "MOTRIX_FNOS_LOG";
const DEFAULT_LOG_FILTER: &str = "info";
const REDACTED_VALUE: &str = "[REDACTED]";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DebugLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DebugLogCategory {
    App,
    Task,
    Aria2,
    Settings,
    Storage,
    Api,
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogEntry {
    pub id: u64,
    pub timestamp_ms: u64,
    pub last_timestamp_ms: u64,
    pub level: DebugLogLevel,
    pub category: DebugLogCategory,
    pub module: String,
    pub message: String,
    pub repeat_count: u32,
}

#[derive(Debug)]
pub struct DebugLogStore {
    capacity: usize,
    next_id: AtomicU64,
    entries: Mutex<VecDeque<DebugLogEntry>>,
}

pub fn init_tracing() {
    let filter = std::env::var(LOG_FILTER_ENV).unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_string());
    let filter = EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

impl Default for DebugLogStore {
    fn default() -> Self {
        Self::new(DEFAULT_DEBUG_LOG_CAPACITY)
    }
}

impl DebugLogStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_id: AtomicU64::new(1),
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn info(&self, module: impl Into<String>, message: impl Into<String>) {
        self.push(DebugLogLevel::Info, module, message);
    }

    pub fn warn(&self, module: impl Into<String>, message: impl Into<String>) {
        self.push(DebugLogLevel::Warn, module, message);
    }

    pub fn error(&self, module: impl Into<String>, message: impl Into<String>) {
        self.push(DebugLogLevel::Error, module, message);
    }

    pub fn push(
        &self,
        level: DebugLogLevel,
        module: impl Into<String>,
        message: impl Into<String>,
    ) {
        let module = module.into();
        let message = redact_log_message(&message.into());
        self.emit_tracing_event(level, &module, &message);

        if self.capacity == 0 {
            return;
        }

        let timestamp_ms = current_timestamp_ms();

        if let Ok(mut entries) = self.entries.lock() {
            if let Some(last) = entries.back_mut() {
                if last.level == level && last.module == module && last.message == message {
                    last.last_timestamp_ms = timestamp_ms;
                    last.repeat_count = last.repeat_count.saturating_add(1);
                    return;
                }
            }

            let entry = DebugLogEntry {
                id: self.next_id.fetch_add(1, Ordering::Relaxed),
                timestamp_ms,
                last_timestamp_ms: timestamp_ms,
                level,
                category: infer_category(&module),
                module,
                message,
                repeat_count: 1,
            };

            while entries.len() >= self.capacity {
                entries.pop_front();
            }
            entries.push_back(entry);
        }
    }

    pub fn list(&self) -> Vec<DebugLogEntry> {
        self.entries
            .lock()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.clear();
        }
    }

    fn emit_tracing_event(&self, level: DebugLogLevel, module: &str, message: &str) {
        emit_redacted_file_log(level, module, message);
    }
}

pub fn emit_file_log(level: DebugLogLevel, module: &str, message: &str) {
    let message = redact_log_message(message);
    emit_redacted_file_log(level, module, &message);
}

pub(crate) fn emit_redacted_file_log(level: DebugLogLevel, module: &str, message: &str) {
    match level {
        DebugLogLevel::Info => tracing::info!(module = module, "{}", message),
        DebugLogLevel::Warn => tracing::warn!(module = module, "{}", message),
        DebugLogLevel::Error => tracing::error!(module = module, "{}", message),
    }
}

pub fn redact_log_message(message: &str) -> String {
    redact_sensitive_fields(&redact_urls(message))
}

pub(crate) fn redact_url_for_log(url: &str) -> String {
    let cutoff = url
        .find('?')
        .into_iter()
        .chain(url.find('#'))
        .min()
        .unwrap_or(url.len());
    url[..cutoff].to_string()
}

fn redact_urls(message: &str) -> String {
    let bytes = message.as_bytes();
    let mut output = String::with_capacity(message.len());
    let mut cursor = 0;
    while let Some(start) = find_http_scheme(bytes, cursor) {
        let end = find_url_end(bytes, start);
        output.push_str(&message[cursor..start]);
        output.push_str(&redact_url_for_log(&message[start..end]));
        cursor = end;
    }
    output.push_str(&message[cursor..]);
    output
}

fn find_http_scheme(bytes: &[u8], from: usize) -> Option<usize> {
    for index in from..bytes.len() {
        if index + 7 <= bytes.len() && bytes[index..index + 7].eq_ignore_ascii_case(b"http://") {
            return Some(index);
        }
        if index + 8 <= bytes.len() && bytes[index..index + 8].eq_ignore_ascii_case(b"https://") {
            return Some(index);
        }
    }
    None
}

fn find_url_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| {
            byte.is_ascii_whitespace()
                || matches!(
                    *byte,
                    b'"' | b'\'' | b'<' | b'>' | b')' | b']' | b'}' | b',' | b';'
                )
        })
        .map(|offset| start + offset)
        .unwrap_or(bytes.len())
}

fn redact_sensitive_fields(message: &str) -> String {
    let bytes = message.as_bytes();
    let mut output = String::with_capacity(message.len());
    let mut cursor = 0;
    let mut index = 0;

    while index < bytes.len() {
        if !is_key_start(bytes[index]) || (index > 0 && is_key_char(bytes[index - 1])) {
            index += 1;
            continue;
        }

        let key_end = next_key_end(bytes, index);
        let key = &message[index..key_end];
        let normalized_key = normalize_key(key);
        if !is_sensitive_key(&normalized_key) {
            index = key_end;
            continue;
        }

        let mut separator = key_end;
        while separator < bytes.len() && bytes[separator].is_ascii_whitespace() {
            separator += 1;
        }
        if separator >= bytes.len() || !matches!(bytes[separator], b':' | b'=') {
            index = key_end;
            continue;
        }

        let mut value_start = separator + 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        let quote = bytes
            .get(value_start)
            .copied()
            .filter(|byte| matches!(byte, b'"' | b'\''));
        if quote.is_some() {
            value_start += 1;
        }
        if value_start >= bytes.len() {
            index = key_end;
            continue;
        }

        let value_end = if let Some(quote) = quote {
            bytes[value_start..]
                .iter()
                .position(|byte| *byte == quote)
                .map(|offset| value_start + offset)
                .unwrap_or(bytes.len())
        } else {
            find_unquoted_value_end(bytes, value_start, &normalized_key)
        };
        if value_end <= value_start {
            index = key_end;
            continue;
        }

        let replacement_start =
            if normalized_key == "authorization" && bytes[value_start..].starts_with(b"Bearer ") {
                value_start + b"Bearer ".len()
            } else {
                value_start
            };
        output.push_str(&message[cursor..replacement_start]);
        output.push_str(REDACTED_VALUE);
        cursor = value_end;
        index = value_end;
    }

    output.push_str(&message[cursor..]);
    output
}

fn find_unquoted_value_end(bytes: &[u8], start: usize, key: &str) -> usize {
    if key == "authorization" && bytes[start..].starts_with(b"Bearer ") {
        let token_start = start + b"Bearer ".len();
        return bytes[token_start..]
            .iter()
            .position(|byte| {
                byte.is_ascii_whitespace()
                    || matches!(*byte, b',' | b';' | b')' | b']' | b'}' | b'>')
            })
            .map(|offset| token_start + offset)
            .unwrap_or(bytes.len());
    }

    let keep_spaces = matches!(key, "password" | "passwd" | "passphrase");
    bytes[start..]
        .iter()
        .position(|byte| {
            (!keep_spaces && byte.is_ascii_whitespace())
                || matches!(
                    *byte,
                    b',' | b';' | b')' | b']' | b'}' | b'>' | b'\n' | b'\r'
                )
        })
        .map(|offset| start + offset)
        .unwrap_or(bytes.len())
}

fn next_key_end(bytes: &[u8], start: usize) -> usize {
    bytes[start..]
        .iter()
        .position(|byte| !is_key_char(*byte))
        .map(|offset| start + offset)
        .unwrap_or(bytes.len())
}

fn is_key_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn normalize_key(key: &str) -> String {
    key.bytes()
        .filter(|byte| byte.is_ascii_alphanumeric())
        .map(|byte| byte.to_ascii_lowercase() as char)
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key,
        "token"
            | "password"
            | "passwd"
            | "passphrase"
            | "secret"
            | "rpcsecret"
            | "signature"
            | "signed"
            | "sig"
            | "csrf"
            | "csrftoken"
            | "xcsrftoken"
            | "session"
            | "sessionid"
            | "cookie"
            | "setcookie"
            | "authorization"
            | "apikey"
            | "accesstoken"
            | "refreshtoken"
            | "jsonrpctoken"
            | "sessiontoken"
            | "xsessionid"
            | "credential"
            | "credentials"
            | "privatekey"
    )
}

fn infer_category(module: &str) -> DebugLogCategory {
    match module.split('.').next().unwrap_or(module) {
        "tasks" => DebugLogCategory::Task,
        "aria2" => DebugLogCategory::Aria2,
        "settings" => DebugLogCategory::Settings,
        "storage" => DebugLogCategory::Storage,
        "api" => DebugLogCategory::Api,
        "runtime" => DebugLogCategory::Runtime,
        _ => DebugLogCategory::App,
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
