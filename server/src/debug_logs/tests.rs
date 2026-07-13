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
