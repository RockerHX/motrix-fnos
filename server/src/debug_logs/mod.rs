use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing_subscriber::EnvFilter;

pub const DEFAULT_DEBUG_LOG_CAPACITY: usize = 500;
pub const LOG_FILTER_ENV: &str = "MOTRIX_FNOS_LOG";
const DEFAULT_LOG_FILTER: &str = "info";

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
        let message = message.into();
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
        emit_file_log(level, module, message);
    }
}

pub fn emit_file_log(level: DebugLogLevel, module: &str, message: &str) {
    match level {
        DebugLogLevel::Info => tracing::info!(module = module, "{}", message),
        DebugLogLevel::Warn => tracing::warn!(module = module, "{}", message),
        DebugLogLevel::Error => tracing::error!(module = module, "{}", message),
    }
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
mod tests {
    use super::*;

    #[test]
    fn store_keeps_recent_entries_with_capacity_limit() {
        let store = DebugLogStore::new(2);

        store.info("test", "first");
        store.warn("test", "second");
        store.error("test", "third");

        let entries = store.list();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "second");
        assert_eq!(entries[1].message, "third");
    }

    #[test]
    fn clear_removes_all_entries() {
        let store = DebugLogStore::new(2);
        store.info("test", "message");

        store.clear();

        assert!(store.list().is_empty());
    }

    #[test]
    fn list_returns_entries_in_time_order() {
        let store = DebugLogStore::new(3);
        store.info("test", "first");
        store.warn("test", "second");
        store.error("test", "third");

        let entries = store.list();
        assert_eq!(
            entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(entries
            .windows(2)
            .all(|window| window[0].timestamp_ms <= window[1].timestamp_ms));
    }

    #[test]
    fn store_collapses_consecutive_duplicate_entries() {
        let store = DebugLogStore::new(3);

        store.warn("aria2.rpc", "same");
        store.warn("aria2.rpc", "same");
        store.warn("aria2.rpc", "same");

        let entries = store.list();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].repeat_count, 3);
        assert_eq!(entries[0].category, DebugLogCategory::Aria2);
        assert!(entries[0].last_timestamp_ms >= entries[0].timestamp_ms);
    }

    #[test]
    fn store_serializes_category_and_repeat_fields() {
        let store = DebugLogStore::new(2);
        store.error("tasks.create", "failed");

        let value = serde_json::to_value(store.list().remove(0)).expect("log should serialize");

        assert_eq!(value["category"], "task");
        assert_eq!(value["repeatCount"], 1);
        assert!(value["lastTimestampMs"].as_u64().is_some());
    }
}
