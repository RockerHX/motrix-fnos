use super::*;
use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
use std::sync::Arc;

#[test]
fn default_snapshot_enables_auto_stop_after_recovery_gate() {
    let snapshot = Aria2LifecycleSnapshot::default();

    assert_eq!(snapshot.phase, Aria2LifecyclePhase::Stopped);
    assert!(snapshot.auto_stop_enabled);
    assert!(!snapshot.can_auto_stop());
}

#[test]
fn activity_classifier_matches_engine_keepalive_matrix() {
    let paused = sample_task(DownloadTaskStatus::Paused);
    let complete = sample_task(DownloadTaskStatus::Complete);
    let error = sample_task(DownloadTaskStatus::Error);

    let mut confirmation_wait = sample_task(DownloadTaskStatus::Pending);
    confirmation_wait.source_type = DownloadTaskSourceType::Magnet;
    confirmation_wait.url = "magnet:?xt=urn:btih:confirmation".to_string();
    confirmation_wait.gid = None;
    confirmation_wait.confirmation_required = true;
    confirmation_wait.metadata_torrent_path = Some("/app-data/magnet/task.torrent".to_string());
    confirmation_wait.file_path = None;

    let mut missing_metadata = sample_task(DownloadTaskStatus::Complete);
    missing_metadata.source_type = DownloadTaskSourceType::Torrent;
    missing_metadata.gid = None;
    missing_metadata.metadata_torrent_path = None;

    let mut static_magnet = sample_task(DownloadTaskStatus::Pending);
    static_magnet.source_type = DownloadTaskSourceType::Magnet;
    static_magnet.url = "magnet:?xt=urn:btih:static".to_string();
    static_magnet.gid = None;
    static_magnet.file_path = None;
    static_magnet.metadata_torrent_path = None;

    let mut active_download = sample_task(DownloadTaskStatus::Active);
    active_download.download_speed = 0;

    let mut metadata_parsing = sample_task(DownloadTaskStatus::Pending);
    metadata_parsing.source_type = DownloadTaskSourceType::Magnet;
    metadata_parsing.url = "magnet:?xt=urn:btih:parsing".to_string();
    metadata_parsing.gid = Some("metadata-gid".to_string());
    metadata_parsing.file_path = None;
    metadata_parsing.metadata_torrent_path = None;

    let mut seeding = sample_task(DownloadTaskStatus::Complete);
    seeding.source_type = DownloadTaskSourceType::Torrent;

    let cases = [
        ("无任务", Vec::new(), Aria2ActivitySignals::default(), true),
        (
            "暂停任务",
            vec![paused],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "完成任务",
            vec![complete],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "错误任务",
            vec![error],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "磁链确认等待",
            vec![confirmation_wait],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "种子缺失 metadata",
            vec![missing_metadata],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "静态磁链",
            vec![static_magnet],
            Aria2ActivitySignals::default(),
            true,
        ),
        (
            "有效下载",
            vec![active_download],
            Aria2ActivitySignals::default(),
            false,
        ),
        (
            "metadata 解析",
            vec![metadata_parsing],
            Aria2ActivitySignals::default(),
            false,
        ),
        (
            "BT 做种",
            vec![seeding],
            Aria2ActivitySignals {
                has_bt_upload: true,
                ..Aria2ActivitySignals::default()
            },
            false,
        ),
        (
            "在途任务操作",
            Vec::new(),
            Aria2ActivitySignals {
                has_inflight_operation: true,
                ..Aria2ActivitySignals::default()
            },
            false,
        ),
        (
            "排队请求",
            Vec::new(),
            Aria2ActivitySignals {
                has_queued_request: true,
                ..Aria2ActivitySignals::default()
            },
            false,
        ),
    ];

    for (name, tasks, signals, expected_idle) in cases {
        let activity = Aria2ActivitySnapshot::from_tasks(&tasks, signals);
        assert_eq!(activity.is_idle(), expected_idle, "场景：{name}");
        assert_eq!(activity.blocks_auto_stop(), !expected_idle, "场景：{name}");
    }
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
fn coordinator_policy_enables_auto_stop_with_fixed_timeouts() {
    let coordinator = Aria2LifecycleCoordinator::default();
    let policy = coordinator.policy();

    assert!(policy.auto_stop_enabled);
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

#[test]
fn quiescing_activity_cancels_stop_before_atomic_transition() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    coordinator
        .set_phase(Aria2LifecyclePhase::Ready)
        .expect("phase should change");
    let quiescing = coordinator
        .begin_quiescing()
        .expect("quiescing should begin");
    let activity = coordinator
        .acquire_activity()
        .expect("quiescing should still accept new activity");

    let error = match coordinator.acquire_stop_permit(quiescing) {
        Ok(_) => panic!("active workflow should cancel stop transition"),
        Err(error) => error,
    };
    assert!(error.contains("在途生命周期操作"));
    assert_eq!(
        coordinator.snapshot().expect("snapshot should load").phase,
        Aria2LifecyclePhase::Ready
    );
    drop(activity);
}

#[test]
fn stop_permit_atomically_rejects_new_work_until_completed() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    coordinator
        .set_phase(Aria2LifecyclePhase::Ready)
        .expect("phase should change");
    let quiescing = coordinator
        .begin_quiescing()
        .expect("quiescing should begin");
    let permit = coordinator
        .acquire_stop_permit(quiescing)
        .expect("idle coordinator should issue stop permit");

    assert_eq!(
        coordinator.snapshot().expect("snapshot should load").phase,
        Aria2LifecyclePhase::Stopping
    );
    assert_eq!(
        coordinator
            .acquire_activity()
            .err()
            .expect("stopping should reject activity"),
        "Aria2 正在停止，请稍后重试"
    );
    permit
        .complete(Aria2LifecyclePhase::Stopped)
        .expect("stop should complete");
    assert_eq!(
        coordinator.snapshot().expect("snapshot should load").phase,
        Aria2LifecyclePhase::Stopped
    );
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

#[tokio::test]
async fn coordinator_reports_requests_waiting_for_lifecycle_lock() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    let operation = coordinator.lock_lifecycle_operation().await;
    let waiting_coordinator = Arc::clone(&coordinator);
    let waiting_request = tokio::spawn(async move {
        let _operation = waiting_coordinator
            .lock_lifecycle_operation_for_request()
            .await
            .expect("queued request should acquire the lifecycle lock");
    });

    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(
        coordinator
            .snapshot()
            .expect("snapshot should load")
            .queued_requests,
        1
    );
    drop(operation);
    waiting_request.await.expect("queued request should join");
    assert_eq!(
        coordinator
            .snapshot()
            .expect("snapshot should load")
            .queued_requests,
        0
    );
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

#[tokio::test]
async fn lifecycle_failure_backoff_does_not_block_user_work() {
    let coordinator = Arc::new(Aria2LifecycleCoordinator::default());
    coordinator
        .record_failure("后台探测失败")
        .expect("failure should be recorded");

    let activity = coordinator
        .acquire_activity()
        .expect("user activity should ignore background backoff");
    let operation = coordinator
        .lock_lifecycle_operation_for_request()
        .await
        .expect("user lifecycle request should ignore background backoff");

    assert_eq!(
        coordinator
            .snapshot()
            .expect("snapshot should load")
            .consecutive_failures,
        1
    );
    drop(operation);
    drop(activity);
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
