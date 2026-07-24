use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const FAILURE_WINDOW_MS: u64 = 5 * 60 * 1_000;
const LOCK_DURATION_MS: u64 = 30 * 1_000;
const FAILURE_LIMIT: usize = 5;
const GLOBAL_FAILURE_LIMIT: usize = 100;
const MAX_SOURCE_BUCKETS: usize = 1_024;
const OVERFLOW_SOURCE: &str = "__overflow__";
pub const UNKNOWN_LOGIN_SOURCE: &str = "unknown";

type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRateLimitError {
    StoreUnavailable,
}

#[derive(Clone)]
pub struct LoginRateLimiter {
    state: Arc<Mutex<LoginRateLimiterState>>,
    clock: Clock,
}

#[derive(Default)]
struct LoginRateLimitState {
    failures: VecDeque<u64>,
    locked_until_ms: Option<u64>,
}

#[derive(Default)]
struct LoginRateLimiterState {
    sources: HashMap<String, LoginRateLimitState>,
    global: LoginRateLimitState,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self::with_clock(Arc::new(current_timestamp_ms))
    }

    pub fn retry_after_seconds(&self, source: &str) -> Result<Option<u64>, LoginRateLimitError> {
        let now = (self.clock)();
        let mut limiter = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        prune_sources(&mut limiter.sources, now);
        if let Some(retry_after) = active_lock(&mut limiter.global, now) {
            return Ok(Some(retry_after));
        }
        let state = source_state(&mut limiter, source, now);
        Ok(active_lock(state, now))
    }

    pub fn record_failure(&self, source: &str) -> Result<Option<u64>, LoginRateLimitError> {
        let now = (self.clock)();
        let mut limiter = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        prune_sources(&mut limiter.sources, now);
        if let Some(retry_after) = active_lock(&mut limiter.global, now) {
            return Ok(Some(retry_after));
        }
        let source_locked = {
            let state = source_state(&mut limiter, source, now);
            if let Some(retry_after) = active_lock(state, now) {
                return Ok(Some(retry_after));
            }
            prune_failures(state, now);
            state.failures.push_back(now);
            if state.failures.len() >= FAILURE_LIMIT {
                state.locked_until_ms = Some(now.saturating_add(LOCK_DURATION_MS));
                true
            } else {
                false
            }
        };
        prune_failures(&mut limiter.global, now);
        limiter.global.failures.push_back(now);
        if limiter.global.failures.len() >= GLOBAL_FAILURE_LIMIT {
            limiter.global.locked_until_ms = Some(now.saturating_add(LOCK_DURATION_MS));
            return Ok(Some(LOCK_DURATION_MS / 1_000));
        }
        if source_locked {
            return Ok(Some(LOCK_DURATION_MS / 1_000));
        }
        Ok(None)
    }

    pub fn record_success(&self, source: &str) -> Result<(), LoginRateLimitError> {
        let now = (self.clock)();
        let mut limiter = self
            .state
            .lock()
            .map_err(|_| LoginRateLimitError::StoreUnavailable)?;
        prune_sources(&mut limiter.sources, now);
        let state = source_state(&mut limiter, source, now);
        state.failures.clear();
        state.locked_until_ms = None;
        limiter.global.failures.clear();
        limiter.global.locked_until_ms = None;
        Ok(())
    }

    #[cfg(test)]
    fn source_bucket_count(&self) -> usize {
        self.state
            .lock()
            .expect("rate limiter state should lock")
            .sources
            .len()
    }

    fn with_clock(clock: Clock) -> Self {
        Self {
            state: Arc::new(Mutex::new(LoginRateLimiterState::default())),
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

fn source_state<'a>(
    limiter: &'a mut LoginRateLimiterState,
    source: &str,
    now: u64,
) -> &'a mut LoginRateLimitState {
    let source = if source.trim().is_empty() {
        UNKNOWN_LOGIN_SOURCE
    } else {
        source
    };
    if !limiter.sources.contains_key(source) && limiter.sources.len() >= MAX_SOURCE_BUCKETS {
        prune_sources(&mut limiter.sources, now);
    }
    let source =
        if !limiter.sources.contains_key(source) && limiter.sources.len() >= MAX_SOURCE_BUCKETS {
            if !limiter.sources.contains_key(OVERFLOW_SOURCE) {
                if let Some(evicted) = limiter.sources.keys().next().cloned() {
                    limiter.sources.remove(&evicted);
                }
            }
            OVERFLOW_SOURCE
        } else {
            source
        };
    limiter.sources.entry(source.to_string()).or_default()
}

fn prune_sources(sources: &mut HashMap<String, LoginRateLimitState>, now: u64) {
    sources.retain(|_, state| {
        active_lock(state, now);
        prune_failures(state, now);
        state.locked_until_ms.is_some() || !state.failures.is_empty()
    });
}

fn prune_failures(state: &mut LoginRateLimitState, now: u64) {
    while state
        .failures
        .front()
        .is_some_and(|timestamp| now.saturating_sub(*timestamp) >= FAILURE_WINDOW_MS)
    {
        state.failures.pop_front();
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
