use motrix_fnos_server::auth::AuthService;
use motrix_fnos_server::database::connect_database;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Installation(PathBuf);

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn installer_uses_installed_paths_without_leaking_passwords() {
    let installation = Installation(std::env::temp_dir().join(format!(
            "motrix-auth-cli-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let root = &installation.0;
    fs::create_dir_all(root.join("cmd")).unwrap();
    fs::create_dir_all(root.join("target/bin")).unwrap();
    fs::create_dir_all(root.join("var")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packaging/fnos/cmd");
    for file in ["common.sh", "install_callback", "reset-web-auth"] {
        symlink(source.join(file), root.join("cmd").join(file)).unwrap();
    }
    symlink(
        env!("CARGO_BIN_EXE_motrix-fnos-server"),
        root.join("target/bin/motrix-fnos-server"),
    )
    .unwrap();
    let callback = root.join("cmd/install_callback");
    let error_file = root.join("install-error");
    let rejected = Command::new(&callback)
        .env("TRIM_TEMP_LOGFILE", &error_file)
        .env("wizard_management_password", "private installer password")
        .env(
            "wizard_management_password_confirm",
            "mismatching installer password",
        )
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("private installer password"));
    assert!(!fs::read_to_string(&error_file)
        .unwrap()
        .contains("private installer password"));
    let installed = Command::new(&callback)
        .env("wizard_management_password", "original management password")
        .env(
            "wizard_management_password_confirm",
            "original management password",
        )
        .output()
        .unwrap();
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(
        Command::new(&callback).output().unwrap().status.success(),
        "reinstall preserves the configured password without wizard input"
    );
    let database = connect_database(root.join("var/motrix-fnos.sqlite"))
        .await
        .unwrap();
    let auth = AuthService::new(database.pool.clone());
    let configured = auth
        .verify_password("original management password")
        .await
        .unwrap();
    assert!(!configured.setup_required);
    database.pool.close().await;
}
