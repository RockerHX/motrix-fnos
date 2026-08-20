use super::*;
use std::time::Duration;

#[test]
fn detailed_log_mode_defaults_to_warn_and_keeps_a_thirty_minute_deadline() {
    let coordinator = Aria2LogModeCoordinator::default();
    let before = current_timestamp_ms();

    coordinator.enable_detailed();
    let status = coordinator.status(false);
    let after = current_timestamp_ms();

    assert_eq!(coordinator.current_level(), Aria2LogLevel::Debug);
    assert!(status.detailed);
    assert!(status.applies_on_next_start);
    let until = status
        .detailed_until_ms
        .expect("detailed mode should have a deadline");
    assert!(until >= before + ARIA2_DETAILED_LOG_DURATION.as_millis() as u64);
    assert!(until <= after + ARIA2_DETAILED_LOG_DURATION.as_millis() as u64);
}

#[test]
fn expired_detailed_mode_returns_to_warn_and_marks_restore_pending() {
    let coordinator = Aria2LogModeCoordinator::with_detailed_duration(Duration::ZERO);
    coordinator.enable_detailed();

    let change = coordinator
        .expire_if_due()
        .expect("zero-duration detailed mode should expire");

    assert_eq!(change.level(), Aria2LogLevel::Warn);
    assert_eq!(coordinator.current_level(), Aria2LogLevel::Warn);
    assert_eq!(
        coordinator.worker_action(),
        Some(Aria2LogModeWorkerAction::RetryRestore)
    );
    coordinator.mark_applied(change);
    assert_eq!(coordinator.worker_action(), None);
}
