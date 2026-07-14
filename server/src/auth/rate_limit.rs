use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const FAILURE_WINDOW_MS: u64 = 5 * 60 * 1_000;
const LOCK_DURATION_MS: u64 = 30 * 1_000;
const FAILURE_LIMIT: usize = 5;

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRateLimitError {
    StoreUnavailable,
}

#[derive(Clone)]
pub struct LoginRateLimiter {
    state: Arc<Mutex<LoginRateLimitState>>,
    clock: Clock,
}

#[derive(Default)]
struct LoginRateLimitState {
    failures: VecDeque<u64>,
    locked_until_ms: Option<u64>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(current_timestamp_ms))
    }

    pub fn retry_after_seconds(&self) -> Result<Option<u64>, LoginRateLimitError> {
        let now = (self.clock)();
        let mut state = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        Ok(active_lock(&mut state, now))
    }

    pub fn record_failure(&self) -> Result<Option<u64>, LoginRateLimitError> {
        let now = (self.clock)();
        let mut state = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        if let Some(retry_after) = active_lock(&mut state, now) {
            return Ok(Some(retry_after));
        }
        while state
            .failures
            .front()
            .is_some_and(|timestamp| now.saturating_sub(*timestamp) >= FAILURE_WINDOW_MS)
        {
            state.failures.pop_front();
        }
        state.failures.push_back(now);
        if state.failures.len() >= FAILURE_LIMIT {
            state.locked_until_ms = Some(now.saturating_add(LOCK_DURATION_MS));
            return Ok(Some(LOCK_DURATION_MS / 1_000));
        }
        Ok(None)
    }

    pub fn record_success(&self) -> Result<(), LoginRateLimitError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        state.failures.clear();
        state.locked_until_ms = None;
        Ok(())
    }

    fn with_clock(clock: Clock) -> Self {
        Self {
            state: Arc::new(Mutex::new(LoginRateLimitState::default())),
            clock,
        }
    }
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

fn active_lock(state: &mut LoginRateLimitState, now: u64) -> Option<u64> {
    let locked_until = state.locked_until_ms?;
    if now >= locked_until {
        state.locked_until_ms = None;
        state.failures.clear();
        return None;
    }
    Some(locked_until.saturating_sub(now).div_ceil(1_000))
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
