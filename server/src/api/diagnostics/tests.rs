use super::DIAGNOSTIC_BUNDLE_SLOTS;

#[tokio::test]
async fn diagnostic_bundle_allows_only_one_generation_at_a_time() {
    let first = DIAGNOSTIC_BUNDLE_SLOTS
        .try_acquire()
        .expect("first bundle slot should be available");
    assert!(DIAGNOSTIC_BUNDLE_SLOTS.try_acquire().is_err());
    drop(first);
    assert!(DIAGNOSTIC_BUNDLE_SLOTS.try_acquire().is_ok());
}
