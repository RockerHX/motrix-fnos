use super::*;

#[test]
fn aria2_runtime_paths_use_app_data_aria2_directory() {
    let app_data_dir = PathBuf::from("/tmp/motrix-fnos-app-data");
    let paths = aria2_runtime_paths(&app_data_dir);

    assert_eq!(paths.runtime_dir, app_data_dir.join("aria2"));
    assert_eq!(paths.session_path, app_data_dir.join("aria2/aria2.session"));
    assert_eq!(paths.state_dir, app_data_dir.join("aria2/state"));
    assert_eq!(paths.log_path, app_data_dir.join("aria2/aria2.log"));
}

#[test]
fn ensure_aria2_session_file_creates_missing_file_without_truncating() {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-session-{}.session",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis()
    ));

    ensure_aria2_session_file(&path).expect("session file should be created");
    assert!(path.is_file());
    fs::write(&path, b"content").expect("session content should write");
    ensure_aria2_session_file(&path).expect("existing session file should be kept");
    assert_eq!(
        fs::read_to_string(&path).expect("session should read"),
        "content"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn old_runtime_record_without_identity_fields_still_reads() {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-old-runtime-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis()
    ));
    std::fs::write(
        &path,
        r#"{
  "pid": 42,
  "actualPort": 6800,
  "rpcSecret": "secret",
  "rpcEndpoint": "http://127.0.0.1:6800/jsonrpc",
  "binarySource": "sidecar"
}
"#,
    )
    .expect("old runtime fixture should write");

    let restored = read_aria2_runtime_record(&path)
        .expect("old runtime should read")
        .expect("old runtime should exist");

    assert_eq!(restored.pid, 42);
    assert_eq!(restored.actual_port, 6800);
    assert_eq!(restored.binary_source, Aria2BinarySource::Sidecar);
    assert!(restored.sidecar_name.is_none());
    assert!(restored.app_data_dir.is_none());
    assert!(restored.launch_args.is_none());

    remove_aria2_runtime_record(&path).expect("old runtime should remove");
}

#[test]
fn runtime_record_round_trips_and_removes() {
    let path = std::env::temp_dir().join(format!(
        "motrix-fnos-runtime-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be valid")
            .as_millis()
    ));
    let runtime = Aria2RuntimeInfo {
        pid: 42,
        actual_port: 16800,
        rpc_secret: "secret".to_string(),
        rpc_endpoint: "http://127.0.0.1:16800/jsonrpc".to_string(),
        binary_source: Aria2BinarySource::Sidecar,
        sidecar_name: Some("aria2-next".to_string()),
        app_data_dir: Some("/tmp/motrix-fnos".to_string()),
        aria2_session_path: Some("/tmp/motrix-fnos/aria2/aria2.session".to_string()),
        aria2_state_path: Some("/tmp/motrix-fnos/aria2/state".to_string()),
        aria2_log_path: Some("/tmp/motrix-fnos/aria2/aria2.log".to_string()),
        launch_args: Some(vec!["--enable-rpc=true".to_string()]),
        process_start_time: None,
        process_uid: None,
    };

    write_aria2_runtime_record(&path, &runtime).expect("runtime should write");
    let restored = read_aria2_runtime_record(&path)
        .expect("runtime should read")
        .expect("runtime should exist");
    assert_eq!(restored, runtime);

    remove_aria2_runtime_record(&path).expect("runtime should remove");
    assert!(read_aria2_runtime_record(&path)
        .expect("missing runtime should read")
        .is_none());
}

#[tokio::test]
async fn native_runtime_is_isolated_from_legacy_session_and_uses_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = state_test_dir("isolated");
    fs::create_dir_all(root.join("aria2")).unwrap();
    fs::write(root.join("aria2/aria2.session"), b"legacy-state").unwrap();
    let database = crate::database::connect_database(root.join("motrix-fnos.sqlite"))
        .await
        .unwrap();
    let state = ServerState::new(database, Vec::new(), 1);
    let mut config = Aria2Config::from_env();
    config.state_dir = Some("ignored-external-path".to_string());
    let config = state.with_aria2_runtime_paths(config).unwrap();
    let paths = native_aria2_runtime_paths(&root);
    assert_eq!(config.session_path.as_deref(), paths.session_path.to_str());
    assert_eq!(config.state_dir.as_deref(), paths.state_dir.to_str());
    assert_eq!(
        fs::read(root.join("aria2/aria2.session")).unwrap(),
        b"legacy-state"
    );
    assert_eq!(
        fs::metadata(paths.state_dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(paths.session_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    state.database.pool.close().await;
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_state_rejects_directory_and_file_links_without_touching_targets() {
    let root = state_test_dir("links");
    let outside = root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("session"), b"original").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("engine")).unwrap();
    assert!(ensure_private_directory(&root, Path::new("engine/native-v2/state")).is_err());
    let link = root.join("session-link");
    std::os::unix::fs::symlink(outside.join("session"), &link).unwrap();
    assert!(ensure_aria2_session_file(&link).is_err());
    fs::hard_link(outside.join("session"), root.join("session-hard-link")).unwrap();
    assert!(ensure_aria2_session_file(&root.join("session-hard-link")).is_err());
    assert_eq!(fs::read(outside.join("session")).unwrap(), b"original");
    let native = root.join("state");
    fs::create_dir(&native).unwrap();
    std::os::unix::fs::symlink(outside.join("session"), native.join("checkpoint.db")).unwrap();
    assert!(validate_native_state_tree(&native).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_runtime_record_is_an_error_instead_of_no_record() {
    let root = state_test_dir("invalid-record");
    let path = root.join("aria2-runtime.json");
    fs::create_dir(&path).unwrap();
    assert!(read_aria2_runtime_record(&path).is_err());
    fs::remove_dir(&path).unwrap();
    fs::write(&path, b"truncated-json").unwrap();
    assert!(read_aria2_runtime_record(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"truncated-json");
    fs::remove_dir_all(root).unwrap();
}

fn state_test_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "motrix-state-{label}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    root
}
