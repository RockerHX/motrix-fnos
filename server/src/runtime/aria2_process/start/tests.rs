use super::super::tests::{sample_config, sample_runtime, temp_dir};
use super::*;

#[cfg(unix)]
#[test]
fn start_process_sets_utf8_path_encoding() {
    use std::os::unix::fs::PermissionsExt;

    const PROBE_ENV: &str = "MOTRIX_TEST_ENGINE_LOCALE_PROBE";
    if std::env::var_os(PROBE_ENV).is_none() {
        for lang in [None, Some("C"), Some("en_US.utf8")] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "runtime::aria2_process::start::tests::start_process_sets_utf8_path_encoding",
                ])
                .env(PROBE_ENV, "1")
                .env("LC_ALL", "C")
                .env_remove("LC_CTYPE")
                .env_remove("LANG");
            if let Some(lang) = lang {
                command.env("LANG", lang);
            }
            let output = command.output().unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "LANG={lang:?}: {stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    }

    let root = temp_dir("engine-locale");
    std::fs::create_dir_all(&root).unwrap();
    let binary = root.join("fake-aria2");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s' \"${LANG-}\" > \"${0%/*}/engine-lang\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let runtime = sample_runtime(Some(binary));
    let mut config = sample_config();
    config.rpc_port = 0;
    let process = Mutex::new(None);
    start_process_with_log_level(
        &process,
        &runtime,
        &config,
        &DebugLogStore::default(),
        Aria2LogLevel::Warn,
        None,
    )
    .unwrap();
    let mut child = process.into_inner().unwrap().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while child.is_running().unwrap() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let exited = !child.is_running().unwrap();
    if !exited {
        child.kill().unwrap();
    }
    let lang = std::fs::read_to_string(root.join("engine-lang"));
    std::fs::remove_dir_all(root).unwrap();
    assert!(exited, "locale probe should exit within the deadline");
    assert_eq!(lang.unwrap(), "C.UTF-8");
}
