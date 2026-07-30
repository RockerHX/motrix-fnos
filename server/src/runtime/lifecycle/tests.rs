use super::*;

#[test]
fn default_snapshot_keeps_auto_stop_disabled_until_recovery_gate() {
    let snapshot = Aria2LifecycleSnapshot::default();

    assert_eq!(snapshot.phase, Aria2LifecyclePhase::Stopped);
    assert!(!snapshot.auto_stop_enabled);
    assert!(!snapshot.can_auto_stop());
}

#[test]
fn active_task_blocks_auto_stop_even_when_speed_is_zero() {
    let activity = Aria2ActivitySnapshot {
        has_active_task: true,
        ..Aria2ActivitySnapshot::default()
    };

    assert!(!activity.is_idle());
    assert!(activity.blocks_auto_stop());
}

#[test]
fn confirmation_wait_without_engine_activity_is_idle() {
    let activity = Aria2ActivitySnapshot::default();

    assert!(activity.is_idle());
    assert!(!activity.blocks_auto_stop());
}

#[test]
fn manual_review_blocks_automatic_lifecycle_mutation() {
    let activity = Aria2ActivitySnapshot {
        requires_manual_review: true,
        ..Aria2ActivitySnapshot::default()
    };
    let snapshot = Aria2LifecycleSnapshot {
        phase: Aria2LifecyclePhase::Ready,
        activity,
        auto_stop_enabled: true,
        consecutive_failures: 0,
    };

    assert!(!snapshot.can_auto_stop());
}

#[test]
fn only_ready_idle_enabled_snapshot_can_auto_stop() {
    let snapshot = Aria2LifecycleSnapshot {
        phase: Aria2LifecyclePhase::Ready,
        activity: Aria2ActivitySnapshot::default(),
        auto_stop_enabled: true,
        consecutive_failures: 0,
    };

    assert!(snapshot.can_auto_stop());
}

#[test]
fn recovery_gate_is_not_inferred_from_ready_phase() {
    let snapshot = Aria2LifecycleSnapshot {
        phase: Aria2LifecyclePhase::Ready,
        activity: Aria2ActivitySnapshot::default(),
        auto_stop_enabled: false,
        consecutive_failures: 0,
    };

    assert!(!snapshot.can_auto_stop());
}
