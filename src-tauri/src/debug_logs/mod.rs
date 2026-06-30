use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_DEBUG_LOG_CAPACITY: usize = 500;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DebugLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DebugLogEntry {
    pub id: u64,
    pub timestamp_ms: u64,
    pub level: DebugLogLevel,
    pub module: String,
    pub message: String,
}

#[derive(Debug)]
pub struct DebugLogStore {
    capacity: usize,
    next_id: AtomicU64,
    entries: Mutex<VecDeque<DebugLogEntry>>,
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

        let entry = DebugLogEntry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            timestamp_ms: current_timestamp_ms(),
            level,
            module,
            message,
        };

        if let Ok(mut entries) = self.entries.lock() {
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
        match level {
            DebugLogLevel::Info => tracing::info!(module = module, "{}", message),
            DebugLogLevel::Warn => tracing::warn!(module = module, "{}", message),
            DebugLogLevel::Error => tracing::error!(module = module, "{}", message),
        }
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
