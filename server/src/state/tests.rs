use super::*;

#[test]
fn aria2_runtime_paths_use_app_data_aria2_directory() {
    let app_data_dir = PathBuf::from("/tmp/motrix-fnos-app-data");
    let paths = aria2_runtime_paths(&app_data_dir);

    assert_eq!(paths.runtime_dir, app_data_dir.join("aria2"));
    assert_eq!(paths.session_path, app_data_dir.join("aria2/aria2.session"));
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
        aria2_log_path: Some("/tmp/motrix-fnos/aria2/aria2.log".to_string()),
        launch_args: Some(vec!["--enable-rpc=true".to_string()]),
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
