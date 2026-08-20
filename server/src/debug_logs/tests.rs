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

#[test]
fn rolling_file_writer_rotates_when_size_limit_is_reached() {
    let root = test_log_dir("rotate");
    let path = root.join("server.log");
    let writer = RollingFileMakeWriter::new(&path, 8, 2).expect("writer should create");
    let mut first = writer.make_writer();
    first.write_all(b"first\n").expect("first log should write");
    first
        .write_all(b"second\n")
        .expect("second log should write");

    assert_eq!(
        std::fs::read_to_string(&path).expect("current log should exist"),
        "second\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.with_extension("log.1")).expect("rotated log should exist"),
        "first\n"
    );
    remove_test_log_dir(root);
}

#[test]
fn rolling_file_writer_keeps_fixed_number_of_backups() {
    let root = test_log_dir("retention");
    let path = root.join("server.log");
    let writer = RollingFileMakeWriter::new(&path, 4, 2).expect("writer should create");
    let mut output = writer.make_writer();
    for line in [b"aa\n".as_slice(), b"bb\n", b"cc\n", b"dd\n"] {
        output.write_all(line).expect("log should write");
    }

    assert!(path.exists());
    assert!(path.with_file_name("server.log.1").exists());
    assert!(path.with_file_name("server.log.2").exists());
    assert!(!path.with_file_name("server.log.3").exists());
    remove_test_log_dir(root);
}

#[test]
fn rolling_file_writer_reopens_existing_file_and_appends() {
    let root = test_log_dir("reopen");
    let path = root.join("server.log");
    {
        let writer = RollingFileMakeWriter::new(&path, 32, 2).expect("writer should create");
        let mut output = writer.make_writer();
        output.write_all(b"before\n").expect("log should write");
    }
    {
        let writer = RollingFileMakeWriter::new(&path, 32, 2).expect("writer should reopen");
        let mut output = writer.make_writer();
        output.write_all(b"after\n").expect("log should append");
    }

    assert_eq!(
        std::fs::read_to_string(&path).expect("log should exist"),
        "before\nafter\n"
    );
    remove_test_log_dir(root);
}

#[test]
fn rolling_file_writer_returns_error_when_parent_is_not_a_directory() {
    let root = test_log_dir("error");
    let blocker = root.join("blocker");
    std::fs::write(&blocker, b"not a directory").expect("blocker should write");

    let result = RollingFileMakeWriter::new(blocker.join("server.log"), 8, 2);

    assert!(result.is_err());
    remove_test_log_dir(root);
}

fn test_log_dir(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-debug-log-{name}-{}",
        current_timestamp_ms()
    ));
    std::fs::create_dir_all(&path).expect("test log directory should create");
    path
}

fn remove_test_log_dir(path: std::path::PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}
