use motrix_fnos_server::auth::AuthService;
use motrix_fnos_server::database::connect_database;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::symlink;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Terminal {
    master: File,
    child: Child,
    output: String,
}

impl Terminal {
    fn start(command: &Path) -> Self {
        let (mut master, mut slave) = (0, 0);
        // openpty returns owned descriptors; the child gets a controlling terminal.
        unsafe {
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
        }
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        let mut command = Command::new(command);
        command
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
            let flags = libc::fcntl(master.as_raw_fd(), libc::F_GETFL);
            assert_ne!(flags, -1);
            assert_ne!(
                libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK),
                -1
            );
        }
        Self {
            master,
            child: command.spawn().unwrap(),
            output: String::new(),
        }
    }

    fn read_output(&mut self) {
        let mut bytes = [0; 4096];
        loop {
            match self.master.read(&mut bytes) {
                Ok(0) | Err(_) => return,
                Ok(count) => self
                    .output
                    .push_str(&String::from_utf8_lossy(&bytes[..count])),
            }
        }
    }

    fn wait_for(&mut self, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            self.read_output();
            if self.output.contains(text) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("terminal did not show {text}: {}", self.output);
    }

    fn echo_enabled(&self) -> bool {
        let mut settings = unsafe { std::mem::zeroed::<libc::termios>() };
        assert_eq!(
            unsafe { libc::tcgetattr(self.master.as_raw_fd(), &mut settings) },
            0
        );
        settings.c_lflag & libc::ECHO != 0
    }

    fn send(&mut self, input: &[u8]) {
        self.master.write_all(input).unwrap();
    }

    fn finish(&mut self) -> bool {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            self.read_output();
            if let Some(status) = self.child.try_wait().unwrap() {
                self.read_output();
                assert!(self.echo_enabled(), "terminal echo must be restored");
                return status.success();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("terminal command timed out: {}", self.output);
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Installation(PathBuf);

impl Drop for Installation {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn installer_and_local_terminal_reset_use_installed_paths_without_leaking_passwords() {
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
    let before = auth
        .verify_password("original management password")
        .await
        .unwrap();
    let token = auth.issue_admin_token(&before).await.unwrap();
    database.pool.close().await;
    let reset = root.join("cmd/reset-web-auth");
    assert!(
        !Command::new(&reset).output().unwrap().status.success(),
        "non-terminal reset must fail"
    );
    for (password, confirmation, success) in [
        ("hidden new password", "different password", false),
        ("short", "short", false),
        ("hidden new password", "hidden new password", true),
    ] {
        let mut terminal = Terminal::start(&reset);
        terminal.wait_for("请输入新管理密码");
        assert!(!terminal.echo_enabled());
        terminal.send(format!("{password}\n").as_bytes());
        terminal.wait_for("请再次输入新管理密码");
        terminal.send(format!("{confirmation}\n").as_bytes());
        assert_eq!(terminal.finish(), success, "{}", terminal.output);
        assert!(!terminal.output.contains(password));
        assert!(!terminal.output.contains(confirmation));
        let database = connect_database(root.join("var/motrix-fnos.sqlite"))
            .await
            .unwrap();
        let auth = AuthService::new(database.pool.clone());
        if success {
            let after = auth.verify_password(password).await.unwrap();
            assert_eq!(after.auth_version, before.auth_version + 1);
            assert!(auth
                .verify_password("original management password")
                .await
                .is_err());
            assert!(auth
                .validate_admin_token(&token, after.auth_version)
                .await
                .is_err());
        } else {
            assert_eq!(
                auth.verify_password("original management password")
                    .await
                    .unwrap(),
                before
            );
        }
        database.pool.close().await;
    }
    for cancel in [b'\x03', b'\x04'] {
        let mut terminal = Terminal::start(&reset);
        terminal.wait_for("请输入新管理密码");
        assert!(!terminal.echo_enabled());
        terminal.send(&[cancel]);
        assert!(!terminal.finish(), "cancel must fail: {}", terminal.output);
    }
    let database = connect_database(root.join("var/motrix-fnos.sqlite"))
        .await
        .unwrap();
    assert!(AuthService::new(database.pool.clone())
        .verify_password("hidden new password")
        .await
        .is_ok());
    database.pool.close().await;
    assert!(!fs::read_to_string(root.join("var/logs/lifecycle.log"))
        .unwrap()
        .contains("password"));
}
