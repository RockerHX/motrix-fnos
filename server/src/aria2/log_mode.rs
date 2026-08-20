use super::{ARIA2_DETAILED_LOG_LEVEL, ARIA2_LOG_LEVEL, ARIA2_LOG_MAX_BYTES, ARIA2_LOG_MAX_FILES};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tokio::time::Instant;

pub const ARIA2_DETAILED_LOG_DURATION: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Aria2LogLevel {
    Warn,
    Debug,
}

impl Aria2LogLevel {
    pub fn as_aria2_option(self) -> &'static str {
        match self {
            Self::Warn => ARIA2_LOG_LEVEL,
            Self::Debug => ARIA2_DETAILED_LOG_LEVEL,
        }
    }

    fn is_detailed(self) -> bool {
        self == Self::Debug
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2LogModeStatus {
    pub mode: Aria2LogLevel,
    pub detailed: bool,
    pub detailed_until_ms: Option<u64>,
    pub max_file_size_bytes: u64,
    pub max_file_count: usize,
    pub applies_on_next_start: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Aria2LogModeChange {
    generation: u64,
    level: Aria2LogLevel,
}

impl Aria2LogModeChange {
    pub(crate) fn level(self) -> Aria2LogLevel {
        self.level
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Aria2LogModeWorkerAction {
    WaitUntil(Instant),
    RetryRestore,
}

#[derive(Debug, Clone, Copy)]
struct DetailedDeadline {
    instant: Instant,
    until_ms: u64,
}

#[derive(Debug)]
struct Aria2LogModeState {
    level: Aria2LogLevel,
    detailed_deadline: Option<DetailedDeadline>,
    generation: u64,
    restore_pending: bool,
}

pub struct Aria2LogModeCoordinator {
    detailed_duration: Duration,
    state: Mutex<Aria2LogModeState>,
    changes: Notify,
    worker_running: AtomicBool,
}

impl Aria2LogModeCoordinator {
    pub fn new() -> Self {
        Self::with_detailed_duration(ARIA2_DETAILED_LOG_DURATION)
    }

    pub(crate) fn with_detailed_duration(detailed_duration: Duration) -> Self {
        Self {
            detailed_duration,
            state: Mutex::new(Aria2LogModeState {
                level: Aria2LogLevel::Warn,
                detailed_deadline: None,
                generation: 0,
                restore_pending: false,
            }),
            changes: Notify::new(),
            worker_running: AtomicBool::new(false),
        }
    }

    pub fn current_level(&self) -> Aria2LogLevel {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .level
    }

    pub fn status(&self, engine_running: bool) -> Aria2LogModeStatus {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Aria2LogModeStatus {
            mode: state.level,
            detailed: state.level.is_detailed(),
            detailed_until_ms: state.detailed_deadline.map(|deadline| deadline.until_ms),
            max_file_size_bytes: ARIA2_LOG_MAX_BYTES,
            max_file_count: ARIA2_LOG_MAX_FILES,
            applies_on_next_start: state.level.is_detailed() && !engine_running,
        }
    }

    pub(crate) fn enable_detailed(&self) -> Aria2LogModeChange {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.generation = state.generation.saturating_add(1);
        state.level = Aria2LogLevel::Debug;
        state.restore_pending = false;
        state.detailed_deadline = Some(DetailedDeadline {
            instant: Instant::now() + self.detailed_duration,
            until_ms: current_timestamp_ms()
                .saturating_add(self.detailed_duration.as_millis() as u64),
        });
        let change = Aria2LogModeChange {
            generation: state.generation,
            level: state.level,
        };
        drop(state);
        self.changes.notify_one();
        change
    }

    pub(crate) fn disable_detailed(&self) -> Aria2LogModeChange {
        self.set_warn(true)
    }

    pub(crate) fn expire_if_due(&self) -> Option<Aria2LogModeChange> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let deadline = state.detailed_deadline?;
        if deadline.instant > Instant::now() {
            return None;
        }
        state.generation = state.generation.saturating_add(1);
        state.level = Aria2LogLevel::Warn;
        state.detailed_deadline = None;
        state.restore_pending = true;
        let change = Aria2LogModeChange {
            generation: state.generation,
            level: state.level,
        };
        drop(state);
        self.changes.notify_one();
        Some(change)
    }

    pub(crate) fn mark_applied(&self, change: Aria2LogModeChange) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut changed = false;
        if state.generation == change.generation && state.level == change.level {
            state.restore_pending = false;
            changed = true;
        }
        drop(state);
        if changed {
            self.changes.notify_one();
        }
    }

    pub(crate) fn pending_restore(&self) -> Option<Aria2LogModeChange> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.restore_pending.then_some(Aria2LogModeChange {
            generation: state.generation,
            level: state.level,
        })
    }

    pub(crate) fn worker_action(&self) -> Option<Aria2LogModeWorkerAction> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(deadline) = state.detailed_deadline {
            return Some(Aria2LogModeWorkerAction::WaitUntil(deadline.instant));
        }
        state
            .restore_pending
            .then_some(Aria2LogModeWorkerAction::RetryRestore)
    }

    pub(crate) fn try_start_worker(&self) -> bool {
        self.worker_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn finish_worker(&self) {
        self.worker_running.store(false, Ordering::Release);
    }

    pub(crate) async fn wait_for_change(&self) {
        self.changes.notified().await;
    }

    fn set_warn(&self, restore_pending: bool) -> Aria2LogModeChange {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.generation = state.generation.saturating_add(1);
        state.level = Aria2LogLevel::Warn;
        state.detailed_deadline = None;
        state.restore_pending = restore_pending;
        let change = Aria2LogModeChange {
            generation: state.generation,
            level: state.level,
        };
        drop(state);
        self.changes.notify_one();
        change
    }
}

impl Default for Aria2LogModeCoordinator {
    fn default() -> Self {
        Self::new()
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
