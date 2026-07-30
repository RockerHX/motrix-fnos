use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aria2LifecyclePhase {
    Stopped,
    Starting,
    Ready,
    Stopping,
    Faulted,
}

impl Default for Aria2LifecyclePhase {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Aria2ActivitySnapshot {
    pub has_active_task: bool,
    pub has_metadata_activity: bool,
    pub has_bt_upload: bool,
    pub has_inflight_operation: bool,
    pub has_queued_request: bool,
    pub requires_manual_review: bool,
}

impl Aria2ActivitySnapshot {
    pub fn blocks_auto_stop(self) -> bool {
        self.has_active_task
            || self.has_metadata_activity
            || self.has_bt_upload
            || self.has_inflight_operation
            || self.has_queued_request
            || self.requires_manual_review
    }

    pub fn is_idle(self) -> bool {
        !self.blocks_auto_stop()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aria2LifecycleSnapshot {
    pub phase: Aria2LifecyclePhase,
    pub activity: Aria2ActivitySnapshot,
    pub auto_stop_enabled: bool,
    pub consecutive_failures: u32,
}

impl Aria2LifecycleSnapshot {
    pub fn new(auto_stop_enabled: bool) -> Self {
        Self {
            phase: Aria2LifecyclePhase::Stopped,
            activity: Aria2ActivitySnapshot::default(),
            auto_stop_enabled,
            consecutive_failures: 0,
        }
    }

    pub fn can_auto_stop(self) -> bool {
        self.auto_stop_enabled
            && self.phase == Aria2LifecyclePhase::Ready
            && self.activity.is_idle()
    }
}

impl Default for Aria2LifecycleSnapshot {
    fn default() -> Self {
        Self::new(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aria2LifecyclePolicy {
    pub auto_stop_enabled: bool,
    pub idle_debounce: Duration,
    pub rpc_ready_timeout: Duration,
    pub session_timeout: Duration,
    pub process_exit_timeout: Duration,
    pub request_wait_timeout: Duration,
}

impl Default for Aria2LifecyclePolicy {
    fn default() -> Self {
        Self {
            auto_stop_enabled: false,
            idle_debounce: Duration::from_secs(30),
            rpc_ready_timeout: Duration::from_secs(3),
            session_timeout: Duration::from_secs(15),
            process_exit_timeout: Duration::from_secs(2),
            request_wait_timeout: Duration::from_secs(15),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aria2LeaseKind {
    Activity,
    Request,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aria2LifecycleCoordinatorSnapshot {
    pub phase: Aria2LifecyclePhase,
    pub active_leases: usize,
    pub in_flight_requests: usize,
    pub cancellation_generation: u64,
}

#[derive(Debug)]
struct CoordinatorState {
    phase: Aria2LifecyclePhase,
    next_lease_id: u64,
    active_leases: usize,
    in_flight_requests: usize,
    cancellation_generation: u64,
}

pub struct Aria2LifecycleCoordinator {
    policy: Aria2LifecyclePolicy,
    state: Mutex<CoordinatorState>,
    operation: tokio::sync::Mutex<()>,
    changes: Notify,
}

impl Aria2LifecycleCoordinator {
    pub fn new(policy: Aria2LifecyclePolicy) -> Self {
        Self {
            policy,
            state: Mutex::new(CoordinatorState {
                phase: Aria2LifecyclePhase::Stopped,
                next_lease_id: 0,
                active_leases: 0,
                in_flight_requests: 0,
                cancellation_generation: 0,
            }),
            operation: tokio::sync::Mutex::new(()),
            changes: Notify::new(),
        }
    }

    pub fn policy(&self) -> Aria2LifecyclePolicy {
        self.policy
    }

    pub async fn lock_lifecycle_operation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.operation.lock().await
    }

    pub fn snapshot(&self) -> Result<Aria2LifecycleCoordinatorSnapshot, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "无法读取 Aria2 生命周期协调状态".to_string())?;
        Ok(Aria2LifecycleCoordinatorSnapshot {
            phase: state.phase,
            active_leases: state.active_leases,
            in_flight_requests: state.in_flight_requests,
            cancellation_generation: state.cancellation_generation,
        })
    }

    pub fn set_phase(&self, phase: Aria2LifecyclePhase) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "无法更新 Aria2 生命周期阶段".to_string())?;
        state.phase = phase;
        drop(state);
        self.changes.notify_waiters();
        Ok(())
    }

    pub fn acquire_activity(self: &Arc<Self>) -> Result<Aria2Lease, String> {
        self.acquire(Aria2LeaseKind::Activity)
    }

    pub fn acquire_request(self: &Arc<Self>) -> Result<Aria2Lease, String> {
        self.acquire(Aria2LeaseKind::Request)
    }

    fn acquire(self: &Arc<Self>, kind: Aria2LeaseKind) -> Result<Aria2Lease, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "无法申请 Aria2 生命周期租约".to_string())?;
        if state.phase == Aria2LifecyclePhase::Stopping {
            return Err("Aria2 正在停止，请稍后重试".to_string());
        }
        state.next_lease_id = state
            .next_lease_id
            .checked_add(1)
            .ok_or_else(|| "Aria2 生命周期租约编号已耗尽".to_string())?;
        state.active_leases += 1;
        if kind == Aria2LeaseKind::Request {
            state.in_flight_requests += 1;
        }
        Ok(Aria2Lease {
            coordinator: Arc::clone(self),
            kind,
            lease_id: state.next_lease_id,
            cancellation_generation: state.cancellation_generation,
        })
    }

    pub fn cancel_in_flight(&self) -> Result<u64, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "无法发送 Aria2 请求取消通知".to_string())?;
        state.cancellation_generation = state
            .cancellation_generation
            .checked_add(1)
            .ok_or_else(|| "Aria2 请求取消代次已耗尽".to_string())?;
        let generation = state.cancellation_generation;
        drop(state);
        self.changes.notify_waiters();
        Ok(generation)
    }

    pub async fn wait_for_change(&self) {
        self.changes.notified().await;
    }
}

impl Default for Aria2LifecycleCoordinator {
    fn default() -> Self {
        Self::new(Aria2LifecyclePolicy::default())
    }
}

pub struct Aria2Lease {
    coordinator: Arc<Aria2LifecycleCoordinator>,
    kind: Aria2LeaseKind,
    lease_id: u64,
    cancellation_generation: u64,
}

impl Aria2Lease {
    pub fn kind(&self) -> Aria2LeaseKind {
        self.kind
    }

    pub fn lease_id(&self) -> u64 {
        self.lease_id
    }

    pub fn is_cancelled(&self) -> Result<bool, String> {
        let snapshot = self.coordinator.snapshot()?;
        Ok(snapshot.cancellation_generation != self.cancellation_generation)
    }
}

impl Drop for Aria2Lease {
    fn drop(&mut self) {
        let Ok(mut state) = self.coordinator.state.lock() else {
            return;
        };
        state.active_leases = state.active_leases.saturating_sub(1);
        if self.kind == Aria2LeaseKind::Request {
            state.in_flight_requests = state.in_flight_requests.saturating_sub(1);
        }
        drop(state);
        self.coordinator.changes.notify_waiters();
    }
}

#[cfg(test)]
mod tests;
