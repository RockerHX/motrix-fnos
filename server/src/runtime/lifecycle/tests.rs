use super::*;
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
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
fn activity_classifier_requires_a_gid_for_active_tasks() {
    let mut active = sample_task(DownloadTaskStatus::Active);
    active.download_speed = 0;
    let activity = Aria2ActivitySnapshot::from_tasks(&[active], Aria2ActivitySignals::default());
    assert!(activity.has_active_task);
    assert!(!activity.is_idle());

    let mut missing_gid = sample_task(DownloadTaskStatus::Active);
    missing_gid.gid = None;
    let activity =
        Aria2ActivitySnapshot::from_tasks(&[missing_gid], Aria2ActivitySignals::default());
    assert!(!activity.has_active_task);
    assert!(activity.requires_manual_review);
    assert!(!activity.is_idle());
}

#[test]
fn activity_classifier_allows_completed_magnet_confirmation_wait() {
    let mut task = sample_task(DownloadTaskStatus::Paused);
    task.source_type = DownloadTaskSourceType::Magnet;
    task.gid = None;
    task.confirmation_required = true;
    task.metadata_torrent_path = Some("/app-data/magnet/task.torrent".to_string());
    let activity = Aria2ActivitySnapshot::from_tasks(&[task], Aria2ActivitySignals::default());
    assert!(activity.is_idle());
}

#[test]
fn activity_classifier_keeps_metadata_bt_operations_and_queue_active() {
    let mut metadata_task = sample_task(DownloadTaskStatus::Pending);
    metadata_task.source_type = DownloadTaskSourceType::Magnet;
    metadata_task.url = "magnet:?xt=urn:btih:test".to_string();
    metadata_task.file_path = None;
    metadata_task.metadata_torrent_path = None;
    let activity = Aria2ActivitySnapshot::from_tasks(
        &[metadata_task],
        Aria2ActivitySignals {
            has_bt_upload: true,
            has_inflight_operation: true,
            has_queued_request: true,
            ..Aria2ActivitySignals::default()
        },
    );
    assert!(activity.has_metadata_activity);
    assert!(activity.has_bt_upload);
    assert!(activity.has_inflight_operation);
    assert!(activity.has_queued_request);
    assert!(!activity.is_idle());
}

#[test]
fn activity_classifier_marks_missing_bt_metadata_for_manual_review() {
    let mut task = sample_task(DownloadTaskStatus::Complete);
    task.source_type = DownloadTaskSourceType::Torrent;
    task.gid = None;
    task.metadata_torrent_path = None;
    let activity = Aria2ActivitySnapshot::from_tasks(&[task], Aria2ActivitySignals::default());
    assert!(activity.requires_manual_review);
    assert!(!activity.is_idle());
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

#[tokio::test]
async fn request_lifecycle_lock_times_out_with_retryable_error() {
    let coordinator = Aria2LifecycleCoordinator::new(Aria2LifecyclePolicy {
        request_wait_timeout: Duration::from_millis(10),
        ..Aria2LifecyclePolicy::default()
    });
    let _operation = coordinator.lock_lifecycle_operation().await;

    let error = coordinator
        .lock_lifecycle_operation_for_request()
        .await
        .expect_err("request should time out while lifecycle operation is held");
    assert!(error.contains("生命周期转换超时"));
    assert!(error.contains("请稍后重试"));
}

#[test]
fn lifecycle_failure_state_uses_bounded_backoff_and_clears_on_success() {
    assert_eq!(
        Aria2LifecycleCoordinator::failure_backoff(1),
        Duration::from_secs(5)
    );
    assert_eq!(
        Aria2LifecycleCoordinator::failure_backoff(2),
        Duration::from_secs(15)
    );
    assert_eq!(
        Aria2LifecycleCoordinator::failure_backoff(3),
        Duration::from_secs(60)
    );
    assert_eq!(
        Aria2LifecycleCoordinator::failure_backoff(99),
        Duration::from_secs(300)
    );

    let coordinator = Aria2LifecycleCoordinator::default();
    coordinator
        .record_failure("session 保存失败")
        .expect("failure should be recorded");
    let snapshot = coordinator.snapshot().expect("snapshot should load");
    assert_eq!(snapshot.consecutive_failures, 1);
    assert_eq!(snapshot.last_error.as_deref(), Some("session 保存失败"));
    assert!(snapshot.retry_after.is_some());

    coordinator.clear_failure().expect("failure should clear");
    let snapshot = coordinator.snapshot().expect("snapshot should load");
    assert_eq!(snapshot.consecutive_failures, 0);
    assert_eq!(snapshot.retry_after, None);
    assert_eq!(snapshot.last_error, None);
}

fn sample_task(status: DownloadTaskStatus) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some("gid-1".to_string()),
        status,
        total_length: 1024,
        completed_length: 0,
        download_speed: 128,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::new(),
        created_at: 1,
        updated_at: 2,
    }
}
