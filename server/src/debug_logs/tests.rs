use super::*;

#[test]
fn store_keeps_recent_entries_with_capacity_limit() {
    let store = DebugLogStore::new(2);

    store.info("test", "first");
    store.warn("test", "second");
    store.error("test", "third");

    let entries = store.list();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].message, "second");
    assert_eq!(entries[1].message, "third");
}

#[test]
fn clear_removes_all_entries() {
    let store = DebugLogStore::new(2);
    store.info("test", "message");

    store.clear();

    assert!(store.list().is_empty());
}

#[test]
fn list_returns_entries_in_time_order() {
    let store = DebugLogStore::new(3);
    store.info("test", "first");
    store.warn("test", "second");
    store.error("test", "third");

    let entries = store.list();
    assert_eq!(
        entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(entries
        .windows(2)
        .all(|window| window[0].timestamp_ms <= window[1].timestamp_ms));
}

#[test]
fn store_collapses_consecutive_duplicate_entries() {
    let store = DebugLogStore::new(3);

    store.warn("aria2.rpc", "same");
    store.warn("aria2.rpc", "same");
    store.warn("aria2.rpc", "same");

    let entries = store.list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].repeat_count, 3);
    assert_eq!(entries[0].category, DebugLogCategory::Aria2);
    assert!(entries[0].last_timestamp_ms >= entries[0].timestamp_ms);
}

#[test]
fn store_serializes_category_and_repeat_fields() {
    let store = DebugLogStore::new(2);
    store.error("tasks.create", "failed");

    let value = serde_json::to_value(store.list().remove(0)).expect("log should serialize");

    assert_eq!(value["category"], "task");
    assert_eq!(value["repeatCount"], 1);
    assert!(value["lastTimestampMs"].as_u64().is_some());
}

#[test]
fn redactor_removes_sensitive_url_parts_and_fields() {
    let message = concat!(
        "GET https://example.com/file.zip?token=url-secret&name=visible#fragment ",
        "password=pass phrase, rpc_secret:rpc-secret ",
        "Authorization: Bearer bearer-secret ",
        "Cookie: motrix_web_session=session-secret; X-CSRF-Token=csrf-secret"
    );

    let redacted = redact_log_message(message);

    assert!(redacted.contains("https://example.com/file.zip"));
    assert!(redacted.contains("[REDACTED]"));
    for secret in [
        "url-secret",
        "pass phrase",
        "rpc-secret",
        "bearer-secret",
        "session-secret",
        "csrf-secret",
    ] {
        assert!(!redacted.contains(secret), "secret leaked: {secret}");
    }
}

#[test]
fn debug_log_store_redacts_before_persisting() {
    let store = DebugLogStore::new(2);

    store.error(
        "api.test",
        "请求失败，url=https://example.com/?secret=url-secret token=rpc-secret",
    );

    let entry = store.list().pop().expect("entry should exist");
    assert!(!entry.message.contains("url-secret"));
    assert!(!entry.message.contains("rpc-secret"));
    assert!(entry.message.contains("[REDACTED]"));
}
