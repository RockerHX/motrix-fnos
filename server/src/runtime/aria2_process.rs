use crate::app::ServerRuntimeConfig;
use crate::aria2::{process_args, summarize_args, terminate_process};
use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use crate::debug_logs::DebugLogStore;
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub binary_source: Option<Aria2BinarySource>,
    pub message: String,
}

#[derive(Debug)]
pub struct ManagedAria2Process {
    child: Child,
    source: Aria2BinarySource,
}

impl ManagedAria2Process {
    pub fn new(child: Child, source: Aria2BinarySource) -> Self {
        Self { child, source }
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn source(&self) -> Aria2BinarySource {
        self.source.clone()
    }

    fn is_running(&mut self) -> Result<bool, String> {
        self.child
            .try_wait()
            .map(|status| status.is_none())
            .map_err(|error| format!("读取 Aria2 进程状态失败：{}", error))
    }

    fn kill(&mut self) -> Result<(), String> {
        self.child
            .kill()
            .map_err(|error| format!("停止 Aria2 进程句柄失败：{}", error))
    }

    fn wait(&mut self) {
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAria2Binary {
    pub path: PathBuf,
    pub source: Aria2BinarySource,
}

pub fn process_status(
    process: &Mutex<Option<ManagedAria2Process>>,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process
        .lock()
        .map_err(|_| "无法读取 Aria2 进程状态".to_string())?;

    let Some(child) = guard.as_mut() else {
        return Ok(Aria2ProcessStatus {
            running: false,
            pid: None,
            binary_source: None,
            message: "Aria2 进程未启动".to_string(),
        });
    };

    if child.is_running()? {
        return Ok(Aria2ProcessStatus {
            running: true,
            pid: Some(child.id()),
            binary_source: Some(child.source()),
            message: "Aria2 进程已启动".to_string(),
        });
    }

    let pid = child.id();
    let source = child.source();
    let _ = guard.take();
    Ok(Aria2ProcessStatus {
        running: false,
        pid: Some(pid),
        binary_source: Some(source),
        message: format!("Aria2 进程已退出，PID {}", pid),
    })
}

pub fn start_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(child) = guard.as_mut() {
        let pid = child.id();
        let source = child.source();
        if child.is_running()? {
            debug_logs.info("aria2", format!("Aria2 进程已在运行，PID {}", pid));
            return Ok(Aria2ProcessStatus {
                running: true,
                pid: Some(pid),
                binary_source: Some(source),
                message: "Aria2 进程已在运行".to_string(),
            });
        }

        debug_logs.warn("aria2", format!("清理已退出的 Aria2 进程句柄，PID {}", pid));
        let _ = guard.take();
    }

    if rpc_port_in_use(config) {
        let error = format!(
            "Aria2 RPC 端口 {}:{} 已被其他进程占用，请先退出残留的 Aria2 Next 进程后重试",
            config.rpc_host, config.rpc_port
        );
        debug_logs.error("aria2", &error);
        return Err(error);
    }

    let args = process_args(config);
    log_start_summary(debug_logs, config, &args);
    let resolved = resolve_aria2_binary(runtime, config)?;
    let child = Command::new(&resolved.path)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("启动 Aria2 Next 失败：{}", error))?;
    let pid = child.id();
    *guard = Some(ManagedAria2Process::new(child, resolved.source.clone()));
    debug_logs.info(
        "aria2",
        format!(
            "Aria2 进程启动成功，来源 {}，PID {}",
            source_label(&resolved.source),
            pid
        ),
    );

    Ok(Aria2ProcessStatus {
        running: true,
        pid: Some(pid),
        binary_source: Some(resolved.source.clone()),
        message: format!("Aria2 进程启动成功（{}）", source_label(&resolved.source)),
    })
}

pub fn stop_process(
    process: &Mutex<Option<ManagedAria2Process>>,
    debug_logs: &DebugLogStore,
) -> Result<Aria2ProcessStatus, String> {
    let mut guard = process.lock().map_err(|_| {
        debug_logs.error("aria2", "无法写入 Aria2 进程状态");
        "无法写入 Aria2 进程状态".to_string()
    })?;

    if let Some(mut child) = guard.take() {
        let pid = child.id();
        if !child.is_running()? {
            debug_logs.warn(
                "aria2",
                format!("停止 Aria2 进程：PID {} 已不存在，清理本地句柄", pid),
            );
        } else {
            debug_logs.info("aria2", format!("准备停止 Aria2 进程，PID {}", pid));
            if let Err(error) = child.kill() {
                debug_logs.warn("aria2", format!("{}，尝试按 PID 兜底终止，PID {}", error, pid));
            }
            child.wait();
            if !wait_until_process_exits(pid, Duration::from_millis(800))
                && !terminate_process(pid)
            {
                let error = format!("停止 Aria2 进程后 PID {} 仍然存活", pid);
                debug_logs.error("aria2", &error);
                return Err(error);
            }
            debug_logs.info("aria2", format!("Aria2 进程已停止，PID {}", pid));
        }
    } else {
        debug_logs.info("aria2", "停止 Aria2 进程：当前没有运行中的进程");
    }

    Ok(Aria2ProcessStatus {
        running: false,
        pid: None,
        binary_source: None,
        message: "Aria2 进程已停止".to_string(),
    })
}

pub fn resolve_aria2_binary(
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
) -> Result<ResolvedAria2Binary, String> {
    resolve_aria2_binary_with(
        runtime,
        config,
        std::env::current_exe().ok().as_deref(),
        repo_root_from_manifest_dir().as_deref(),
    )
}

fn resolve_aria2_binary_with(
    runtime: &ServerRuntimeConfig,
    config: &Aria2Config,
    current_exe: Option<&Path>,
    repo_root: Option<&Path>,
) -> Result<ResolvedAria2Binary, String> {
    if let Some(path) = runtime.aria2_path.as_deref() {
        return resolve_explicit_binary_path(path);
    }

    if let Some(path) = current_exe
        .and_then(|path| packaged_binary_path(path, &config.sidecar_name))
        .filter(|path| path.is_file())
    {
        return Ok(ResolvedAria2Binary {
            path,
            source: Aria2BinarySource::Sidecar,
        });
    }

    if let Some(path) = repo_root
        .map(|root| repo_debug_binary_path(root, config))
        .filter(|path| path.is_file())
    {
        return Ok(ResolvedAria2Binary {
            path,
            source: Aria2BinarySource::Sidecar,
        });
    }

    Err(format!(
        "未找到可用 Aria2 Next 可执行文件：已检查 MOTRIX_FNOS_ARIA2_PATH、打包目录 bin/{}、仓库调试路径 {}",
        platform_binary_name(&config.sidecar_name),
        repo_root
            .map(|root| repo_debug_binary_path(root, config).display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string())
    ))
}

fn resolve_explicit_binary_path(path: &Path) -> Result<ResolvedAria2Binary, String> {
    if !path.is_file() {
        return Err(format!(
            "MOTRIX_FNOS_ARIA2_PATH 指向的 Aria2 Next 路径不存在或不是文件：{}",
            path.display()
        ));
    }

    Ok(ResolvedAria2Binary {
        path: path.to_path_buf(),
        source: Aria2BinarySource::ExternalPath,
    })
}

fn repo_root_from_manifest_dir() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
}

fn packaged_binary_path(current_exe: &Path, sidecar_name: &str) -> Option<PathBuf> {
    current_exe
        .parent()
        .map(|dir| dir.join("bin").join(platform_binary_name(sidecar_name)))
}

fn repo_debug_binary_path(repo_root: &Path, config: &Aria2Config) -> PathBuf {
    repo_root.join("src-tauri").join("binaries").join(format!(
        "{}-{}{}",
        config.sidecar_name,
        config.target_triple,
        executable_suffix_for_target(&config.target_triple)
    ))
}

fn platform_binary_name(sidecar_name: &str) -> String {
    format!("{sidecar_name}{}", executable_suffix_for_target(std::env::consts::OS))
}

fn executable_suffix_for_target(target: &str) -> &'static str {
    if target.contains("windows") {
        ".exe"
    } else {
        ""
    }
}

fn rpc_port_in_use(config: &Aria2Config) -> bool {
    let Ok(addresses) = (config.rpc_host.as_str(), config.rpc_port).to_socket_addrs() else {
        return false;
    };

    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok())
}

fn source_label(source: &Aria2BinarySource) -> &'static str {
    match source {
        Aria2BinarySource::ExternalPath => "外部路径",
        Aria2BinarySource::Sidecar => "内置 sidecar",
    }
}

fn log_start_summary(debug_logs: &DebugLogStore, config: &Aria2Config, args: &[String]) {
    debug_logs.info(
        "aria2",
        format!(
            "准备启动 Aria2 Next，来源 {}，target {}，RPC {}:{}，参数 {}",
            source_label(&config.binary_source),
            config.target_triple,
            config.rpc_host,
            config.rpc_port,
            summarize_args(args)
        ),
    );

    if let Some(path) = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--ca-certificate="))
    {
        debug_logs.info("aria2.ca", format!("CA 证书探测成功：{}", path));
    } else {
        debug_logs.warn("aria2.ca", "未探测到可用 CA 证书路径");
    }
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
        .unwrap_or(false)
}

#[cfg(windows)]
fn wait_until_process_exits(pid: u32, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if !process_is_running(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    !process_is_running(pid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::DEFAULT_HTTP_ADDR;

    #[test]
    fn resolve_aria2_binary_prefers_explicit_env_path() {
        let temp_dir = temp_dir("resolve-env");
        let explicit_path = temp_dir.join("custom-aria2");
        std::fs::create_dir_all(&temp_dir).expect("temp dir should create");
        std::fs::write(&explicit_path, b"").expect("explicit path should exist");

        let runtime = sample_runtime(Some(explicit_path.clone()));
        let config = sample_config();
        let resolved = resolve_aria2_binary_with(&runtime, &config, None, None)
            .expect("explicit binary should resolve");

        assert_eq!(resolved.path, explicit_path);
        assert_eq!(resolved.source, Aria2BinarySource::ExternalPath);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn resolve_aria2_binary_uses_packaged_path_before_repo_fallback() {
        let temp_dir = temp_dir("resolve-packaged");
        let current_exe = temp_dir.join("server").join("motrix-fnos-server");
        let packaged_path = current_exe
            .parent()
            .expect("current exe should have parent")
            .join("bin")
            .join(platform_binary_name("aria2-next"));
        let repo_root = temp_dir.join("repo");
        let repo_path = repo_debug_binary_path(&repo_root, &sample_config());

        std::fs::create_dir_all(packaged_path.parent().expect("packaged parent should exist"))
            .expect("packaged dir should create");
        std::fs::write(&packaged_path, b"").expect("packaged path should exist");
        std::fs::create_dir_all(repo_path.parent().expect("repo path should have parent"))
            .expect("repo dir should create");
        std::fs::write(&repo_path, b"").expect("repo path should exist");

        let runtime = sample_runtime(None);
        let config = sample_config();
        let resolved = resolve_aria2_binary_with(
            &runtime,
            &config,
            Some(current_exe.as_path()),
            Some(repo_root.as_path()),
        )
        .expect("packaged binary should resolve");

        assert_eq!(resolved.path, packaged_path);
        assert_eq!(resolved.source, Aria2BinarySource::Sidecar);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn resolve_aria2_binary_falls_back_to_repo_debug_binary() {
        let temp_dir = temp_dir("resolve-repo");
        let repo_root = temp_dir.join("repo");
        let repo_path = repo_debug_binary_path(&repo_root, &sample_config());

        std::fs::create_dir_all(repo_path.parent().expect("repo path should have parent"))
            .expect("repo dir should create");
        std::fs::write(&repo_path, b"").expect("repo path should exist");

        let runtime = sample_runtime(None);
        let config = sample_config();
        let resolved =
            resolve_aria2_binary_with(&runtime, &config, None, Some(repo_root.as_path()))
                .expect("repo binary should resolve");

        assert_eq!(resolved.path, repo_path);
        assert_eq!(resolved.source, Aria2BinarySource::Sidecar);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn process_status_reports_not_started_when_process_missing() {
        let process = Mutex::new(None);

        let status = process_status(&process).expect("status should load");

        assert!(!status.running);
        assert_eq!(status.pid, None);
        assert_eq!(status.binary_source, None);
        assert_eq!(status.message, "Aria2 进程未启动");
    }

    #[test]
    fn process_status_clears_finished_process_handle() {
        let child = spawn_quick_exit_child();
        let process = Mutex::new(Some(ManagedAria2Process::new(
            child,
            Aria2BinarySource::Sidecar,
        )));
        std::thread::sleep(Duration::from_millis(80));

        let status = process_status(&process).expect("status should load");

        assert!(!status.running);
        assert!(status.pid.is_some());
        assert_eq!(status.binary_source, Some(Aria2BinarySource::Sidecar));
        assert!(process.lock().expect("lock should succeed").is_none());
    }

    #[test]
    fn stop_process_succeeds_when_no_process_running() {
        let process = Mutex::new(None);
        let status =
            stop_process(&process, &DebugLogStore::default()).expect("stop should succeed");

        assert!(!status.running);
        assert_eq!(status.pid, None);
        assert_eq!(status.binary_source, None);
        assert_eq!(status.message, "Aria2 进程已停止");
    }

    fn sample_runtime(aria2_path: Option<PathBuf>) -> ServerRuntimeConfig {
        let app_data_dir = temp_dir("runtime");
        ServerRuntimeConfig {
            database_path: app_data_dir.join("motrix-fnos.db"),
            app_data_dir,
            http_addr: DEFAULT_HTTP_ADDR.parse().expect("addr should parse"),
            aria2_path,
        }
    }

    fn sample_config() -> Aria2Config {
        Aria2Config {
            aria2_path: None,
            binary_source: Aria2BinarySource::Sidecar,
            sidecar_name: "aria2-next".to_string(),
            target_triple: "test-target".to_string(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: "secret".to_string(),
            session_path: None,
            log_path: None,
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "motrix-fnos-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be valid")
                .as_millis()
        ))
    }

    #[cfg(unix)]
    fn spawn_quick_exit_child() -> Child {
        Command::new("sh")
            .args(["-c", "exit 0"])
            .spawn()
            .expect("shell should spawn")
    }

    #[cfg(windows)]
    fn spawn_quick_exit_child() -> Child {
        Command::new("cmd")
            .args(["/C", "exit 0"])
            .spawn()
            .expect("cmd should spawn")
    }
}
