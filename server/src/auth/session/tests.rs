use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn creates_unique_high_entropy_sessions_and_expected_cookies() {
    let store = SessionStore::new();
    let first = store
        .create(SessionKind::Admin, 1)
        .expect("session should create");
    let second = store
        .create(SessionKind::Admin, 1)
        .expect("session should create");
    assert_ne!(first.id, second.id);
    assert_ne!(first.csrf_token, second.csrf_token);
    assert_eq!(
        URL_SAFE_NO_PAD
            .decode(&first.id)
            .expect("id should decode")
            .len(),
        32
    );

    let cookie = session_cookie(&first.id, false);
    assert!(cookie.starts_with("motrix_web_session="));
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(cookie.contains("Path=/"));
    assert!(cookie.contains("Max-Age=43200"));
    assert!(!cookie.contains("Secure"));
    assert!(clear_session_cookie(false).contains("Max-Age=0"));
    assert!(session_cookie(&first.id, true).contains("; Secure"));
    assert!(clear_session_cookie(true).contains("; Secure"));
}

#[test]
fn enforces_idle_and_absolute_expiration_with_activity_refresh() {
    let now = Arc::new(AtomicU64::new(1_000));
    let clock_now = now.clone();
    let store = SessionStore::with_clock(Arc::new(move || clock_now.load(Ordering::Relaxed)));
    let session = store
        .create(SessionKind::Admin, 7)
        .expect("session should create");

    now.store(30 * 60 * 1_000, Ordering::Relaxed);
    assert!(store
        .validate(&session.id, 7)
        .expect("validation should run")
        .is_some());
    now.store(59 * 60 * 1_000, Ordering::Relaxed);
    assert!(store
        .validate(&session.id, 7)
        .expect("validation should run")
        .is_some());
    now.store(12 * 60 * 60 * 1_000 + 1_000, Ordering::Relaxed);
    assert!(store
        .validate(&session.id, 7)
        .expect("validation should run")
        .is_none());

    let idle = store
        .create(SessionKind::AnonymousManagement, 8)
        .expect("session should create");
    now.fetch_add(30 * 60 * 1_000, Ordering::Relaxed);
    assert!(store
        .validate(&idle.id, 8)
        .expect("validation should run")
        .is_none());
}

#[test]
fn validation_without_activity_does_not_extend_idle_timeout() {
    let now = Arc::new(AtomicU64::new(1_000));
    let clock_now = now.clone();
    let store = SessionStore::with_clock(Arc::new(move || clock_now.load(Ordering::Relaxed)));
    let session = store
        .create(SessionKind::Admin, 7)
        .expect("session should create");

    now.store(10 * 60 * 1_000, Ordering::Relaxed);
    assert!(store
        .validate_without_activity(&session.id, 7)
        .expect("validation should run")
        .is_some());
    now.store(30 * 60 * 1_000 + 1_000, Ordering::Relaxed);
    assert!(store
        .validate_without_activity(&session.id, 7)
        .expect("validation should run")
        .is_none());
}

#[test]
fn detailed_validation_reports_expiration_and_auth_version_failures() {
    let now = Arc::new(AtomicU64::new(1_000));
    let clock_now = now.clone();
    let store = SessionStore::with_clock(Arc::new(move || clock_now.load(Ordering::Relaxed)));
    let session = store
        .create(SessionKind::Admin, 7)
        .expect("session should create");

    assert!(matches!(
        store
            .validate_detailed(&session.id, 8, true)
            .expect("validation should run"),
        SessionValidation::Invalid(SessionValidationFailure::AuthVersionMismatch)
    ));

    let expired = store
        .create(SessionKind::Admin, 7)
        .expect("session should create");
    now.store(30 * 60 * 1_000 + 1_001, Ordering::Relaxed);
    assert!(matches!(
        store
            .validate_detailed(&expired.id, 7, true)
            .expect("validation should run"),
        SessionValidation::Invalid(SessionValidationFailure::Expired)
    ));

    assert!(matches!(
        store
            .validate_detailed("missing-session", 7, true)
            .expect("validation should run"),
        SessionValidation::Invalid(SessionValidationFailure::NotFound)
    ));
}

#[test]
fn rejects_auth_version_changes_and_cross_session_csrf() {
    let store = SessionStore::new();
    let first = store
        .create(SessionKind::Admin, 3)
        .expect("session should create");
    let second = store
        .create(SessionKind::AnonymousManagement, 3)
        .expect("session should create");

    assert!(store
        .validate_csrf(&first.id, 3, &first.csrf_token)
        .expect("csrf should validate"));
    assert!(!store
        .validate_csrf(&first.id, 3, &second.csrf_token)
        .expect("csrf should validate"));
    assert!(!store
        .validate_csrf(&first.id, 3, "invalid")
        .expect("csrf should validate"));
    assert!(store
        .validate_without_activity(&first.id, 4)
        .expect("validation should run")
        .is_none());
    assert!(store
        .validate(&first.id, 3)
        .expect("validation should run")
        .is_none());

    assert_eq!(
        store
            .validate(&second.id, 3)
            .expect("validation should run")
            .expect("session should exist")
            .kind,
        SessionKind::AnonymousManagement
    );
    store.revoke_all().expect("sessions should revoke");
    assert!(store
        .validate(&second.id, 3)
        .expect("validation should run")
        .is_none());
}
