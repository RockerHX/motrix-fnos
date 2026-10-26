use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

use crate::auth::{AuthError, AuthService, AuthState};

pub(crate) fn native_version_only_engine(app_data_dir: &Path) -> PathBuf {
    version_only_engine(app_data_dir, "2.8.6")
}

fn version_only_engine(app_data_dir: &Path, version: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(app_data_dir).expect("fixture directory should create");
    let binary = app_data_dir.join(format!("version-only-engine-{version}"));
    std::fs::write(
        &binary,
        format!("#!/bin/sh\n[ \"$#\" -eq 1 ] && [ \"$1\" = \"--version\" ] || exit 1\nprintf 'aria2 version {version}\\n'\n"),
    )
    .expect("engine identity fixture should write");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))
        .expect("engine identity fixture should be executable");
    binary
}

pub(crate) fn spawn_graceful_engine_child() -> std::process::Child {
    use std::io::Read;
    let mut child = std::process::Command::new("sh")
        .args([
            "-c",
            "trap 'exit 0' INT; printf ready; while :; do sleep 0.01; done",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("graceful engine fixture should spawn");
    let mut ready = [0; 5];
    child.stdout.take().unwrap().read_exact(&mut ready).unwrap();
    assert_eq!(&ready, b"ready");
    child
}

pub(crate) trait AuthServiceTestExt {
    async fn setup(&self, password: &str) -> Result<AuthState, AuthError>;
}

impl AuthServiceTestExt for AuthService {
    async fn setup(&self, password: &str) -> Result<AuthState, AuthError> {
        self.initialize_password(password).await
    }
}

#[derive(Clone, Default)]
pub(crate) struct TestTracingCapture {
    output: Arc<Mutex<Vec<u8>>>,
}

impl TestTracingCapture {
    pub(crate) fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync {
        tracing_subscriber::fmt()
            .without_time()
            .with_target(false)
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(self.clone())
            .finish()
    }

    pub(crate) fn contents(&self) -> String {
        self.output
            .lock()
            .map(|output| String::from_utf8_lossy(&output).into_owned())
            .unwrap_or_default()
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut output) = self.output.lock() {
            output.clear();
        }
    }
}

impl<'writer> MakeWriter<'writer> for TestTracingCapture {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

impl Write for TestTracingCapture {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let mut output = self
            .output
            .lock()
            .map_err(|_| io::Error::other("测试日志缓冲区锁已损坏"))?;
        output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
