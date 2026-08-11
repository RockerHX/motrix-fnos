use super::try_acquire_diagnostic_bundle_slot;
use axum::http::StatusCode;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[test]
fn diagnostic_bundle_allows_only_one_generation_at_a_time() {
    let slots = Arc::new(Semaphore::new(1));
    let first = try_acquire_diagnostic_bundle_slot(Arc::clone(&slots))
        .expect("first bundle slot should be available");
    let error = try_acquire_diagnostic_bundle_slot(Arc::clone(&slots))
        .expect_err("second bundle slot should be rejected");
    assert_eq!(error.status(), StatusCode::TOO_MANY_REQUESTS);
    drop(first);
    assert!(try_acquire_diagnostic_bundle_slot(slots).is_ok());
}
