use super::{
    build_diagnostic_bundle_from_parts, DiagnosticSummary, ARIA2_LOG_BUNDLE_MAX_BYTES,
    DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES, LIFECYCLE_LOG_BUNDLE_MAX_BYTES, SERVER_LOG_BUNDLE_MAX_BYTES,
};
use crate::aria2::{Aria2LogLevel, Aria2LogModeStatus};
use crate::debug_logs::{DebugLogCategory, DebugLogEntry, DebugLogLevel};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::ZipArchive;

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn diagnostic_bundle_contains_only_whitelisted_redacted_logs() {
    let root = temp_dir("redacted");
    write_file(
        &root.join("logs/server.log"),
        "server password=server-secret https://example.com/file?token=url-secret\n",
    );
    write_file(
        &root.join("logs/lifecycle.log"),
        "lifecycle Authorization: Bearer bearer-secret\n",
    );
    write_file(
        &root.join("aria2/aria2.log"),
        "aria2 rpc-secret=aria2-secret\n",
    );
    write_file(&root.join("aria2/aria2.1.log"), "spdlog history\n");
    write_file(&root.join("aria2/aria2.log.1"), "legacy history\n");
    write_file(&root.join("motrix-fnos.sqlite"), "database-must-not-export");
    let logs = vec![debug_log("token=debug-secret")];

    let entries = unzip(
        &build_diagnostic_bundle_from_parts(&root, &logs, summary()).expect("bundle should build"),
    );
    let names = entries
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let contents = entries
        .iter()
        .map(|(_, contents)| contents.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    for expected in [
        "summary.json",
        "logs/app-debug.jsonl",
        "logs/server.log",
        "logs/lifecycle.log",
        "logs/aria2/aria2.log",
        "logs/aria2/aria2.1.log",
        "logs/aria2/aria2.log.1",
    ] {
        assert!(
            names.contains(&expected),
            "missing archive entry: {expected}"
        );
    }
    assert!(!names.iter().any(|name| name.contains("sqlite")));
    for secret in [
        "server-secret",
        "url-secret",
        "bearer-secret",
        "aria2-secret",
        "debug-secret",
        "database-must-not-export",
    ] {
        assert!(!contents.contains(secret), "secret leaked: {secret}");
    }
    assert!(contents.contains("[REDACTED]"));
    remove_temp_dir(root);
}

#[test]
fn diagnostic_bundle_limits_total_input_and_keeps_latest_log_tails() {
    let root = temp_dir("limits");
    write_file(
        &root.join("aria2/aria2.log"),
        &format!(
            "aria2-head\n{}aria2-tail\n",
            "a".repeat(ARIA2_LOG_BUNDLE_MAX_BYTES)
        ),
    );
    write_file(&root.join("aria2/aria2.1.log"), "aria2-history-tail\n");
    write_file(
        &root.join("logs/server.log"),
        &format!(
            "server-head\n{}server-tail\n",
            "s".repeat(SERVER_LOG_BUNDLE_MAX_BYTES)
        ),
    );
    write_file(&root.join("logs/server.log.1"), "server-history-tail\n");
    write_file(
        &root.join("logs/lifecycle.log"),
        &format!(
            "lifecycle-head\n{}lifecycle-tail\n",
            "l".repeat(LIFECYCLE_LOG_BUNDLE_MAX_BYTES)
        ),
    );
    write_file(
        &root.join("logs/lifecycle.log.1"),
        "lifecycle-history-tail\n",
    );

    let entries = unzip(
        &build_diagnostic_bundle_from_parts(&root, &[], summary()).expect("bundle should build"),
    );
    let total_size = entries
        .iter()
        .map(|(_, contents)| contents.len())
        .sum::<usize>();

    assert!(total_size <= DIAGNOSTIC_BUNDLE_MAX_INPUT_BYTES);
    assert!(entry(&entries, "logs/aria2/aria2.log").contains("aria2-tail"));
    assert!(entry(&entries, "logs/server.log").contains("server-tail"));
    assert!(entry(&entries, "logs/lifecycle.log").contains("lifecycle-tail"));
    assert!(entry(&entries, "logs/aria2/aria2.1.log").contains("aria2-history-tail"));
    assert!(entry(&entries, "logs/server.log.1").contains("server-history-tail"));
    assert!(entry(&entries, "logs/lifecycle.log.1").contains("lifecycle-history-tail"));
    assert!(!entry(&entries, "logs/aria2/aria2.log").contains("aria2-head"));
    remove_temp_dir(root);
}

#[test]
fn diagnostic_bundle_tolerates_missing_log_files() {
    let root = temp_dir("missing");

    let entries = unzip(
        &build_diagnostic_bundle_from_parts(&root, &[], summary()).expect("bundle should build"),
    );

    assert!(entries.iter().any(|(name, _)| name == "summary.json"));
    assert!(entries
        .iter()
        .any(|(name, _)| name == "logs/app-debug.jsonl"));
    assert!(!entries.iter().any(|(name, _)| name == "logs/server.log"));
    remove_temp_dir(root);
}

#[cfg(unix)]
#[test]
fn diagnostic_bundle_skips_symbolic_linked_logs() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("symlink");
    let outside = root.join("outside.log");
    write_file(&outside, "token=outside-secret\n");
    fs::create_dir_all(root.join("logs")).expect("log directory should create");
    symlink(&outside, root.join("logs/server.log")).expect("symlink should create");

    let entries = unzip(
        &build_diagnostic_bundle_from_parts(&root, &[], summary()).expect("bundle should build"),
    );
    let contents = entries
        .iter()
        .map(|(_, contents)| contents.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(!entries.iter().any(|(name, _)| name == "logs/server.log"));
    assert!(!contents.contains("outside-secret"));
    remove_temp_dir(root);
}

#[cfg(unix)]
#[test]
fn diagnostic_bundle_skips_symbolic_linked_log_directories() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("symlink-directory");
    let outside = root.join("outside-logs");
    write_file(&outside.join("server.log"), "token=outside-secret\n");
    symlink(&outside, root.join("logs")).expect("symlink should create");

    let entries = unzip(
        &build_diagnostic_bundle_from_parts(&root, &[], summary()).expect("bundle should build"),
    );

    assert!(!entries.iter().any(|(name, _)| name == "logs/server.log"));
    remove_temp_dir(root);
}

fn summary() -> DiagnosticSummary {
    DiagnosticSummary {
        generated_at_ms: 1,
        app_version: "test",
        aria2_version: Some("2.5.5".to_string()),
        aria2_running: false,
        aria2_lifecycle_phase: "Stopped".to_string(),
        aria2_log_mode: Aria2LogModeStatus {
            mode: Aria2LogLevel::Warn,
            detailed: false,
            detailed_until_ms: None,
            max_file_size_bytes: 10 * 1024 * 1024,
            max_file_count: 3,
            applies_on_next_start: false,
        },
    }
}

fn debug_log(message: &str) -> DebugLogEntry {
    DebugLogEntry {
        id: 1,
        timestamp_ms: 1,
        last_timestamp_ms: 1,
        level: DebugLogLevel::Error,
        category: DebugLogCategory::Api,
        module: "api.test".to_string(),
        message: message.to_string(),
        repeat_count: 1,
    }
}

fn unzip(bytes: &[u8]) -> Vec<(String, String)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip should open");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).expect("zip entry should open");
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .expect("zip entry should be text");
        entries.push((file.name().to_string(), contents));
    }
    entries
}

fn entry<'a>(entries: &'a [(String, String)], name: &str) -> &'a str {
    entries
        .iter()
        .find_map(|(entry_name, contents)| (entry_name == name).then_some(contents.as_str()))
        .expect("archive entry should exist")
}

fn write_file(path: &Path, contents: &str) {
    let parent = path.parent().expect("test path should have parent");
    fs::create_dir_all(parent).expect("parent directory should create");
    fs::write(path, contents).expect("test file should write");
}

fn temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "motrix-fnos-diagnostic-{label}-{}-{counter}",
        std::process::id()
    ))
}

fn remove_temp_dir(path: PathBuf) {
    let _ = fs::remove_dir_all(path);
}
