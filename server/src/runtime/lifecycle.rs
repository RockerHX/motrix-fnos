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

#[cfg(test)]
mod tests;
