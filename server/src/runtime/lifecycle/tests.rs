use super::*;
use std::sync::Arc;

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

#[test]
fn coordinator_policy_keeps_recovery_gate_disabled_with_fixed_timeouts() {
    let coordinator = Aria2LifecycleCoordinator::default();
    let policy = coordinator.policy();

    assert!(!policy.auto_stop_enabled);
    assert_eq!(policy.idle_debounce, Duration::from_secs(30));
    assert_eq!(policy.rpc_ready_timeout, Duration::from_secs(3));
    assert_eq!(policy.session_timeout, Duration::from_secs(15));
    assert_eq!(policy.process_exit_timeout, Duration::from_secs(2));
    assert_eq!(policy.request_wait_timeout, Duration::from_secs(15));
}

#[test]
fn coordinator_tracks_activity_and_request_leases_until_drop() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    let activity = coordinator
        .acquire_activity()
        .expect("activity lease should be acquired");
    let request = coordinator
        .acquire_request()
        .expect("request lease should be acquired");

    let snapshot = coordinator.snapshot().expect("snapshot should load");
    assert_eq!(snapshot.active_leases, 2);
    assert_eq!(snapshot.in_flight_requests, 1);
    assert_eq!(activity.kind(), Aria2LeaseKind::Activity);
    assert_eq!(request.kind(), Aria2LeaseKind::Request);
    assert!(request.lease_id() > activity.lease_id());

    drop(request);
    let snapshot = coordinator.snapshot().expect("snapshot should load");
    assert_eq!(snapshot.active_leases, 1);
    assert_eq!(snapshot.in_flight_requests, 0);
    drop(activity);
    assert_eq!(
        coordinator
            .snapshot()
            .expect("snapshot should load")
            .active_leases,
        0
    );
}

#[test]
fn cancellation_generation_invalidates_existing_request_lease() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    let request = coordinator
        .acquire_request()
        .expect("request lease should be acquired");

    assert!(!request.is_cancelled().expect("lease should be readable"));
    let generation = coordinator
        .cancel_in_flight()
        .expect("cancellation should be published");
    assert_eq!(
        coordinator
            .snapshot()
            .expect("snapshot should load")
            .cancellation_generation,
        generation
    );
    assert!(request.is_cancelled().expect("lease should be readable"));
}

#[test]
fn stopping_phase_rejects_new_lifecycle_leases() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    coordinator
        .set_phase(Aria2LifecyclePhase::Stopping)
        .expect("phase should change");

    let error = match coordinator.acquire_request() {
        Ok(_) => panic!("stopping phase should reject new requests"),
        Err(error) => error,
    };
    assert_eq!(error, "Aria2 正在停止，请稍后重试");
}
