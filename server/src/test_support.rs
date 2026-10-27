use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;

use crate::app::{
    bootstrap_http_app_state, HttpAppState, ServerRuntimeConfig, DEFAULT_HTTP_ADDR,
    DEFAULT_JSONRPC_ADDR,
};
use crate::auth::{AuthError, AuthService, AuthState};
use crate::config::aria2::Aria2BinarySource;
use crate::runtime::ManagedAria2Process;

pub(crate) fn test_runtime(app_data_dir: &Path) -> ServerRuntimeConfig {
    ServerRuntimeConfig {
        database_path: app_data_dir.join("motrix-fnos.sqlite"),
        accessible_paths_path: app_data_dir.join("accessible-paths.json"),
        app_data_dir: app_data_dir.to_path_buf(),
        http_addr: DEFAULT_HTTP_ADDR.parse().unwrap(),
        jsonrpc_addr: DEFAULT_JSONRPC_ADDR.parse().unwrap(),
        lan_jsonrpc_addr: "127.0.0.1:0".parse().unwrap(),
        aria2_path: Some(native_version_only_engine(app_data_dir)),
        trusted_proxy_ips: Vec::new(),
    }
}

pub(crate) fn test_path(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "motrix-fnos-owned-{label}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed),
    ))
}

pub(crate) struct TestAppState {
    state: Arc<HttpAppState>,
    closed: bool,
}

impl TestAppState {
    pub(crate) async fn new(label: &str) -> Self {
        let root = test_path(label);
        std::fs::create_dir(&root).expect("test must own a new directory");
        Self {
            state: bootstrap_http_app_state(&test_runtime(&root))
                .await
                .unwrap(),
            closed: false,
        }
    }

    pub(crate) async fn with_rpc(label: &str, port: u16) -> Self {
        let fixture = Self::new(label).await;
        let child = spawn_graceful_engine_child();
        let config =
            crate::aria2::runtime_config(&fixture.base_aria2_config, port, "secret".into());
        fixture
            .set_aria2_runtime(fixture.build_aria2_runtime_info(
                child.id(),
                &config,
                Aria2BinarySource::ExternalPath,
                vec!["--mock".into()],
            ))
            .unwrap();
        *fixture.aria2_process.lock().unwrap() = Some(ManagedAria2Process::new(
            child,
            Aria2BinarySource::ExternalPath,
        ));
        fixture
    }

    pub(crate) async fn close(mut self) {
        self.request_shutdown("测试资源回收");
        crate::runtime::stop_process(&self.aria2_process, &self.core.debug_logs).unwrap();
        self.clear_aria2_runtime().unwrap();
        self.core.database.pool.close().await;
        std::fs::remove_dir_all(&self.runtime.app_data_dir).unwrap();
        self.closed = true;
    }
}

impl std::ops::Deref for TestAppState {
    type Target = Arc<HttpAppState>;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for TestAppState {
    fn drop(&mut self) {
        if !self.closed {
            self.request_shutdown("测试中止");
            let _ = crate::runtime::stop_process(&self.aria2_process, &self.core.debug_logs);
            // 失败时保留数据库诊断，不能在连接尚未关闭时删目录。
            assert!(
                std::thread::panicking(),
                "test fixture must be closed: {}",
                self.runtime.app_data_dir.display()
            );
        }
    }
}

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

#[cfg(test)]
mod tests;
