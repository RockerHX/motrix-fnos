use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn fifth_failure_locks_all_logins_for_thirty_seconds() {
    let (limiter, now) = test_limiter();
    for _ in 0..4 {
        assert_eq!(
            limiter.record_failure().expect("failure should record"),
            None
        );
    }
    assert_eq!(
        limiter.record_failure().expect("failure should record"),
        Some(30)
    );
    assert_eq!(
        limiter.retry_after_seconds().expect("limit should read"),
        Some(30)
    );

    now.store(29_001, Ordering::Relaxed);
    assert_eq!(
        limiter.retry_after_seconds().expect("limit should read"),
        Some(1)
    );
    now.store(30_000, Ordering::Relaxed);
    assert_eq!(
        limiter.retry_after_seconds().expect("limit should read"),
        None
    );
    assert_eq!(
        limiter.record_failure().expect("failure should record"),
        None
    );
}

#[test]
fn failures_outside_window_expire_and_success_clears_state() {
    let (limiter, now) = test_limiter();
    for _ in 0..4 {
        assert_eq!(
            limiter.record_failure().expect("failure should record"),
            None
        );
    }
    now.store(5 * 60 * 1_000, Ordering::Relaxed);
    assert_eq!(
        limiter.record_failure().expect("failure should record"),
        None
    );

    for _ in 0..3 {
        assert_eq!(
            limiter.record_failure().expect("failure should record"),
            None
        );
    }
    limiter.record_success().expect("success should clear");
    for _ in 0..4 {
        assert_eq!(
            limiter.record_failure().expect("failure should record"),
            None
        );
    }
}

#[test]
fn limiter_is_global_across_clones() {
    let (limiter, _) = test_limiter();
    let another_request = limiter.clone();
    for _ in 0..4 {
        assert_eq!(
            limiter.record_failure().expect("failure should record"),
            None
        );
    }
    assert_eq!(
        another_request
            .record_failure()
            .expect("failure should record"),
        Some(30)
    );
}

fn test_limiter() -> (LoginRateLimiter, Arc<AtomicU64>) {
    let now = Arc::new(AtomicU64::new(0));
    let clock_now = now.clone();
    (
        LoginRateLimiter::with_clock(Arc::new(move || clock_now.load(Ordering::Relaxed))),
        now,
    )
}
