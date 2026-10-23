use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn startup_proxy_is_private_not_in_arguments_and_removed_after_use() {
    let root = std::env::temp_dir().join(format!(
        "motrix-startup-config-{}",
        crate::aria2::generate_rpc_secret()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let proxy = "socks5://test-user:test-secret@127.0.0.1:9/";
    let config = StartupConfig::new(&root, proxy).unwrap();
    let path = config.0.clone();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!("bt-proxy={proxy}\n")
    );
    let mut args = vec!["--no-conf=true".to_string()];
    config.apply_args(&mut args);
    let summary = crate::aria2::summarize_args(&args);
    assert!(!summary.contains("test-secret"));
    assert!(!args.iter().any(|arg| arg == "--no-conf=true"));
    assert!(args.iter().any(|arg| arg.starts_with("--conf-path=")));
    drop(config);
    assert!(!path.exists());
    assert!(StartupConfig::new(&root, "http://127.0.0.1:9\nenable-dht=true").is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn startup_config_rejects_a_symlink_directory() {
    let root = std::env::temp_dir().join(format!(
        "motrix-startup-symlink-{}",
        crate::aria2::generate_rpc_secret()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(std::env::temp_dir(), root.join("engine")).unwrap();
    assert!(StartupConfig::new(&root, "http://127.0.0.1:9").is_err());
    std::fs::remove_dir_all(root).unwrap();
}
