use crate::aria2::Aria2RpcClient;
use crate::auth::{AuthRuntime, AuthService, ServerProcessLock};
use crate::config::aria2::{Aria2Config, ARIA2_PATH_ENV};
use crate::database::{
    backup_database, check_integrity, connect_database,
    maintenance::cleanup_history,
    tasks::{list_download_tasks, max_download_task_id, persist_download_task_states},
    DATABASE_FILE_NAME,
};
use crate::runtime::{Aria2LifecycleCoordinator, ManagedAria2Process};
use crate::state::{Aria2RuntimeInfo, ServerState};
use crate::storage::load_accessible_paths;
use crate::tasks::DownloadTask;
use crate::tasks::{is_pending_magnet_metadata_task, DownloadTaskStatus};
use serde::Serialize;
use std::env;
use std::fs;
use std::future::{Future, IntoFuture};
use std::net::IpAddr;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, watch};

pub const APP_DATA_DIR_ENV: &str = "MOTRIX_FNOS_APP_DATA_DIR";
pub const HTTP_ADDR_ENV: &str = "MOTRIX_FNOS_HTTP_ADDR";
pub const JSONRPC_ADDR_ENV: &str = "MOTRIX_FNOS_JSONRPC_ADDR";
pub const ACCESSIBLE_PATHS_FILE_ENV: &str = "MOTRIX_FNOS_ACCESSIBLE_PATHS_FILE";
pub const TRUSTED_PROXY_IPS_ENV: &str = "MOTRIX_TRUSTED_PROXY_IPS";
pub const WEB_COOKIE_SECURE_ENV: &str = "MOTRIX_WEB_COOKIE_SECURE";
pub const DEFAULT_HTTP_ADDR: &str = "0.0.0.0:17080";
pub const DEFAULT_JSONRPC_ADDR: &str = "127.0.0.1:17081";
pub const ACCESSIBLE_PATHS_FILE_NAME: &str = "accessible-paths.json";
const RUNTIME_EVENT_BUFFER: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRuntimeConfig {
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub http_addr: SocketAddr,
    pub jsonrpc_addr: SocketAddr,
    pub aria2_path: Option<PathBuf>,
    pub accessible_paths_path: PathBuf,
    pub trusted_proxy_ips: Vec<IpAddr>,
    pub web_cookie_secure: bool,
}

impl ServerRuntimeConfig {
    pub fn from_env() -> Result<Self, String> {
        let app_data_dir = env::var(APP_DATA_DIR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_local_app_data_dir);
        let http_addr = env::var(HTTP_ADDR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_HTTP_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("解析管理监听地址失败：{}", error))?;
        let jsonrpc_addr = env::var(JSONRPC_ADDR_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_JSONRPC_ADDR.to_string())
            .parse::<SocketAddr>()
            .map_err(|error| format!("解析 JSON-RPC 监听地址失败：{}", error))?;
        if !jsonrpc_addr.ip().is_loopback() {
            return Err(format!(
                "JSON-RPC 监听地址必须使用回环 IP：{}",
                jsonrpc_addr
            ));
        }
        let aria2_path = env::var(ARIA2_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from);
        let database_path = app_data_dir.join(DATABASE_FILE_NAME);
        let accessible_paths_path = env::var(ACCESSIBLE_PATHS_FILE_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| app_data_dir.join(ACCESSIBLE_PATHS_FILE_NAME));
        let trusted_proxy_ips = parse_trusted_proxy_ips()?;
        let web_cookie_secure = parse_bool_env(WEB_COOKIE_SECURE_ENV, false)?;
        Ok(Self {
            app_data_dir,
            database_path,
            http_addr,
            jsonrpc_addr,
            aria2_path,
            accessible_paths_path,
            trusted_proxy_ips,
            web_cookie_secure,
        })
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeExitingPayload {
    pub reason: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TasksSnapshotPayload {
    pub revision: u64,
    pub tasks: Vec<DownloadTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    TasksSnapshot(TasksSnapshotPayload),
    RuntimeExiting(RuntimeExitingPayload),
}

#[derive(Clone)]
pub struct RuntimeEventHub {
    sender: broadcast::Sender<RuntimeEvent>,
}

impl RuntimeEventHub {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(RUNTIME_EVENT_BUFFER);
        Self { sender }
    }

    pub fn send(&self, event: RuntimeEvent) -> Result<usize, String> {
        match self.sender.send(event) {
            Ok(count) => Ok(count),
            // 没有前端订阅者只表示当前无需投递事件，不能因此把正常的后台任务流程判为失败。
            Err(_) => Ok(0),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.sender.subscribe()
    }
}

impl Default for RuntimeEventHub {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HttpAppState {
    pub core: Arc<ServerState>,
    pub auth: AuthRuntime,
    pub runtime: ServerRuntimeConfig,
    pub base_aria2_config: Aria2Config,
    pub aria2_rpc: Aria2RpcClient,
    pub aria2_process: Mutex<Option<ManagedAria2Process>>,
    pub aria2_lifecycle: Arc<Aria2LifecycleCoordinator>,
    pub runtime_events: RuntimeEventHub,
    pub(crate) tasks_snapshot_revision: Mutex<u64>,
    listeners_ready: AtomicBool,
}

impl HttpAppState {
    pub fn new(core: ServerState, runtime: ServerRuntimeConfig) -> Self {
        let mut base_aria2_config = Aria2Config::from_env();
        base_aria2_config.aria2_path = runtime
            .aria2_path
            .as_ref()
            .map(|path| path.display().to_string());

        let auth = AuthRuntime::new(core.database.pool.clone());
        let aria2_lifecycle = Arc::new(Aria2LifecycleCoordinator::default());
        Self {
            core: Arc::new(core),
            auth,
            runtime,
            base_aria2_config,
            aria2_rpc: Aria2RpcClient::with_lifecycle(Arc::clone(&aria2_lifecycle)),
            aria2_process: Mutex::new(None),
            aria2_lifecycle,
            runtime_events: RuntimeEventHub::new(),
            tasks_snapshot_revision: Mutex::new(0),
            listeners_ready: AtomicBool::new(false),
        }
    }

    pub fn aria2_runtime_snapshot(&self) -> Option<Aria2RuntimeInfo> {
        self.core.aria2_runtime_snapshot()
    }

    pub fn aria2_config(&self) -> Aria2Config {
        let mut config = self.base_aria2_config.clone();
        if let Some(runtime) = self.aria2_runtime_snapshot() {
            config.rpc_port = runtime.actual_port;
            config.rpc_secret = runtime.rpc_secret;
            config.session_path = runtime.aria2_session_path.clone();
            config.log_path = runtime.aria2_log_path.clone();
        }
        config
    }

    pub fn with_aria2_runtime_paths(&self, config: Aria2Config) -> Result<Aria2Config, String> {
        self.core.with_aria2_runtime_paths(config)
    }

    pub fn build_aria2_runtime_info(
        &self,
        pid: u32,
        config: &Aria2Config,
        source: crate::config::aria2::Aria2BinarySource,
        launch_args: Vec<String>,
    ) -> Aria2RuntimeInfo {
        self.core
            .build_aria2_runtime_info(pid, config, source, launch_args)
    }

    pub fn set_aria2_runtime(&self, runtime: Aria2RuntimeInfo) -> Result<(), String> {
        self.core.set_aria2_runtime(runtime)
    }

    pub fn clear_aria2_runtime(&self) {
        self.core.clear_aria2_runtime()
    }

    pub fn load_saved_aria2_runtime(&self) -> Option<Aria2RuntimeInfo> {
        self.core.load_saved_aria2_runtime()
    }

    pub fn mark_listeners_ready(&self) {
        self.listeners_ready.store(true, Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.listeners_ready.load(Ordering::Acquire) && !self.core.shutdown.is_exiting()
    }

    pub fn request_shutdown(&self, reason: impl Into<String>) {
        let reason = reason.into();
        if !self.core.shutdown.begin_shutdown() {
            self.core
                .debug_logs
                .info("runtime.exit", "服务退出流程已在执行，忽略重复退出请求");
            return;
        }

        self.listeners_ready.store(false, Ordering::Release);

        self.core.debug_logs.info("runtime.exit", &reason);
        let _ = self
            .runtime_events
            .send(RuntimeEvent::RuntimeExiting(RuntimeExitingPayload {
                reason,
                timestamp: current_timestamp_ms(),
            }));
    }
}

pub async fn bootstrap_http_app_state(
    runtime: &ServerRuntimeConfig,
) -> Result<Arc<HttpAppState>, String> {
    let database = connect_database(runtime.database_path.clone()).await?;
    let mut restored_tasks = list_download_tasks(&database.pool).await?;
    let accessible_paths = load_accessible_paths(&runtime.accessible_paths_path)?;
    migrate_legacy_owned_task_dirs(&mut restored_tasks, &accessible_paths);
    // 必须先用应用私有 metadata 目录对账恢复任务，再持久化修正后的状态，避免丢失目录在下次启动时继续伪装成可恢复任务。
    reconcile_magnet_metadata_dirs(&runtime.app_data_dir, &mut restored_tasks)?;
    persist_download_task_states(&database.pool, &restored_tasks).await?;
    let next_task_id = max_download_task_id(&database.pool)
        .await?
        .saturating_add(1);
    let state = ServerState::new(database, restored_tasks, next_task_id);
    let state = Arc::new(HttpAppState::new(state, runtime.clone()));
    crate::runtime::reconcile_unfinished_task_operations(&state).await?;

    Ok(state)
}

fn migrate_legacy_owned_task_dirs(tasks: &mut [DownloadTask], accessible_paths: &[String]) {
    let accessible_roots = accessible_paths
        .iter()
        .filter_map(|path| Path::new(path).canonicalize().ok())
        .collect::<Vec<_>>();

    if accessible_roots.is_empty() {
        return;
    }

    for task in tasks.iter_mut() {
        let has_owned_task_dir = task
            .owned_task_dir
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .is_some();
        if has_owned_task_dir || !should_migrate_legacy_owned_task_dir(task) {
            continue;
        }

        let candidate = Path::new(&task.save_dir);
        if let Ok(metadata) = fs::symlink_metadata(candidate) {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                continue;
            }
        }

        let Some(parent) = candidate.parent().and_then(|path| path.canonicalize().ok()) else {
            continue;
        };
        if accessible_roots.iter().any(|root| root == &parent) {
            task.owned_task_dir = Some(
                candidate
                    .canonicalize()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| candidate.display().to_string()),
            );
        }
    }
}

fn should_migrate_legacy_owned_task_dir(task: &DownloadTask) -> bool {
    if task.source_type == crate::tasks::DownloadTaskSourceType::Torrent {
        return true;
    }

    task.source_type == crate::tasks::DownloadTaskSourceType::Magnet
        && !task.confirmation_required
        && (task.metadata_torrent_path.is_some() || task.file_path.is_some())
}

fn reconcile_magnet_metadata_dirs(
    app_data_dir: &Path,
    tasks: &mut [DownloadTask],
) -> Result<(), String> {
    let metadata_root = app_data_dir.join("magnet-metadata");
    if !metadata_root.exists() {
        return Ok(());
    }
    if !metadata_root.is_dir() {
        return Err(format!(
            "磁链 metadata 根目录不是文件夹：{}",
            metadata_root.display()
        ));
    }

    // 清理范围固定在应用私有根目录；先收集仍被任务引用的目录，随后只删除其中未被引用的孤儿子目录。
    let mut referenced_dirs = std::collections::BTreeSet::new();
    for task in tasks.iter_mut() {
        let pending_metadata_dir = if is_pending_magnet_metadata_task(task) {
            Some(magnet_metadata_task_dir(app_data_dir, task.id))
        } else {
            None
        };

        if task.confirmation_required {
            let metadata_missing = task
                .metadata_torrent_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
                .filter(|path| path.is_file())
                .is_none();
            if metadata_missing {
                task.status = DownloadTaskStatus::Error;
                task.gid = None;
                task.confirmation_required = false;
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = Some("磁链 metadata 文件丢失，请重新添加磁链".to_string());
                task.metadata_torrent_path = None;
            }
        }

        if let Some(metadata_dir) = pending_metadata_dir {
            if metadata_dir.is_dir() {
                referenced_dirs.insert(metadata_dir);
            } else {
                task.status = DownloadTaskStatus::Error;
                task.gid = None;
                task.download_speed = 0;
                task.error_code = None;
                task.error_message = Some("磁链 metadata 临时目录丢失，请重新添加磁链".to_string());
            }
        } else if let Some(metadata_dir) = task
            .metadata_torrent_path
            .as_deref()
            .and_then(|path| Path::new(path).parent())
        {
            referenced_dirs.insert(metadata_dir.to_path_buf());
        }
    }

    for entry in fs::read_dir(&metadata_root).map_err(|error| {
        format!(
            "读取磁链 metadata 根目录失败：{}（{}）",
            metadata_root.display(),
            error
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "读取磁链 metadata 目录项失败：{}（{}）",
                metadata_root.display(),
                error
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if referenced_dirs.contains(&path) {
            continue;
        }
        fs::remove_dir_all(&path).map_err(|error| {
            format!(
                "清理孤儿磁链 metadata 目录失败：{}（{}）",
                path.display(),
                error
            )
        })?;
    }

    Ok(())
}

fn magnet_metadata_task_dir(app_data_dir: &Path, task_id: u64) -> PathBuf {
    app_data_dir
        .join("magnet-metadata")
        .join(format!("task-{task_id}"))
}

pub async fn run_server() -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    let _process_lock = ServerProcessLock::acquire(&runtime.app_data_dir)?;
    let state = bootstrap_http_app_state(&runtime).await?;
    let listeners = bind_http_listeners(&runtime).await?;
    state.mark_listeners_ready();
    crate::runtime::spawn_task_monitor(state.clone());

    state.core.debug_logs.info(
        "app",
        format!(
            "管理服务入口已初始化，监听地址 {}，数据目录 {}",
            state.runtime.http_addr,
            state.runtime.app_data_dir.display()
        ),
    );
    state.core.debug_logs.info(
        "app",
        format!(
            "JSON-RPC 专用入口已初始化，监听地址 {}",
            state.runtime.jsonrpc_addr
        ),
    );

    serve_http_listeners(state, listeners, wait_for_shutdown_signal()).await
}

pub async fn run_cli(args: &[String]) -> Result<(), String> {
    match args {
        [] => run_server().await,
        [command] if command == "reset-web-auth" => reset_web_auth().await,
        [command] if command == "database-check" => database_check().await,
        [command, output] if command == "database-backup" => database_backup(output).await,
        [command, before] if command == "database-cleanup-history" => {
            database_cleanup_history(before, false).await
        }
        [command, before, flag]
            if command == "database-cleanup-history" && flag == "--apply" =>
        {
            database_cleanup_history(before, true).await
        }
        _ => Err(
            "用法：motrix-fnos-server [reset-web-auth|database-check|database-backup <output>|database-cleanup-history <before_timestamp_ms> [--apply]]".to_string(),
        ),
    }
}

async fn database_check() -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    let _process_lock = ServerProcessLock::acquire(&runtime.app_data_dir)?;
    check_integrity(runtime.database_path.clone()).await?;
    println!("数据库完整性检查通过：{}", runtime.database_path.display());
    Ok(())
}

async fn database_backup(output: &str) -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    let _process_lock = ServerProcessLock::acquire(&runtime.app_data_dir)?;
    let output = PathBuf::from(output);
    backup_database(runtime.database_path, output.clone()).await?;
    println!("数据库备份已生成：{}", output.display());
    Ok(())
}

async fn database_cleanup_history(before: &str, apply: bool) -> Result<(), String> {
    let before = before
        .parse::<i64>()
        .map_err(|error| format!("清理时间必须是毫秒时间戳：{}", error))?;
    let runtime = ServerRuntimeConfig::from_env()?;
    let _process_lock = ServerProcessLock::acquire(&runtime.app_data_dir)?;
    let database = connect_database(runtime.database_path).await?;
    let report = cleanup_history(&database.pool, before, apply).await;
    database.pool.close().await;
    let report = report?;
    if report.applied {
        println!(
            "历史记录清理完成：删除历史 {} 条，删除错误 {} 条",
            report.history_count, report.error_count
        );
    } else {
        println!(
            "历史记录清理预览：可删除历史 {} 条，错误 {} 条；追加 --apply 才会删除",
            report.history_count, report.error_count
        );
    }
    Ok(())
}

async fn reset_web_auth() -> Result<(), String> {
    let runtime = ServerRuntimeConfig::from_env()?;
    reset_web_auth_with_runtime(&runtime).await
}

async fn reset_web_auth_with_runtime(runtime: &ServerRuntimeConfig) -> Result<(), String> {
    let _process_lock = ServerProcessLock::acquire(&runtime.app_data_dir)?;
    let database = connect_database(runtime.database_path.clone()).await?;
    AuthService::new(database.pool.clone())
        .reset()
        .await
        .map_err(|error| format!("重置 Web 鉴权失败：{error:?}"))?;
    database.pool.close().await;
    Ok(())
}

#[derive(Debug)]
struct HttpListeners {
    management: TcpListener,
    jsonrpc: TcpListener,
}

async fn bind_http_listeners(runtime: &ServerRuntimeConfig) -> Result<HttpListeners, String> {
    let management = TcpListener::bind(runtime.http_addr)
        .await
        .map_err(|error| format!("绑定管理监听地址失败：{}（{}）", runtime.http_addr, error))?;
    let jsonrpc = TcpListener::bind(runtime.jsonrpc_addr)
        .await
        .map_err(|error| {
            format!(
                "绑定 JSON-RPC 监听地址失败：{}（{}）",
                runtime.jsonrpc_addr, error
            )
        })?;

    Ok(HttpListeners {
        management,
        jsonrpc,
    })
}

enum HttpStopTrigger {
    Signal(Result<String, String>),
    Management(std::io::Result<()>),
    JsonRpc(std::io::Result<()>),
}

async fn serve_http_listeners<F>(
    state: Arc<HttpAppState>,
    listeners: HttpListeners,
    shutdown_signal: F,
) -> Result<(), String>
where
    F: Future<Output = Result<String, String>>,
{
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let management_server = axum::serve(
        listeners.management,
        crate::api::management_router(state.clone())
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(wait_for_http_shutdown(shutdown_receiver.clone()))
    .into_future();
    let jsonrpc_server = axum::serve(listeners.jsonrpc, crate::api::jsonrpc_router(state.clone()))
        .with_graceful_shutdown(wait_for_http_shutdown(shutdown_receiver))
        .into_future();

    tokio::pin!(management_server);
    tokio::pin!(jsonrpc_server);
    tokio::pin!(shutdown_signal);

    let trigger = tokio::select! {
        signal = &mut shutdown_signal => HttpStopTrigger::Signal(signal),
        result = &mut management_server => HttpStopTrigger::Management(result),
        result = &mut jsonrpc_server => HttpStopTrigger::JsonRpc(result),
    };

    let (reason, primary_error) = match &trigger {
        HttpStopTrigger::Signal(Ok(reason)) => (reason.clone(), None),
        HttpStopTrigger::Signal(Err(error)) => (
            "等待停止信号失败，准备关闭服务".to_string(),
            Some(error.clone()),
        ),
        HttpStopTrigger::Management(Ok(())) => (
            "管理 HTTP 服务意外停止".to_string(),
            Some("管理 HTTP 服务意外停止".to_string()),
        ),
        HttpStopTrigger::Management(Err(error)) => (
            "管理 HTTP 服务运行失败，准备关闭 JSON-RPC 服务".to_string(),
            Some(format!("管理 HTTP 服务运行失败：{}", error)),
        ),
        HttpStopTrigger::JsonRpc(Ok(())) => (
            "JSON-RPC HTTP 服务意外停止".to_string(),
            Some("JSON-RPC HTTP 服务意外停止".to_string()),
        ),
        HttpStopTrigger::JsonRpc(Err(error)) => (
            "JSON-RPC HTTP 服务运行失败，准备关闭管理服务".to_string(),
            Some(format!("JSON-RPC HTTP 服务运行失败：{}", error)),
        ),
    };

    state.request_shutdown(reason);
    crate::runtime::run_shutdown_cleanup(&state).await;
    let _ = shutdown_sender.send(true);

    let remaining_error = match trigger {
        HttpStopTrigger::Signal(_) => {
            let (management_result, jsonrpc_result) =
                tokio::join!(&mut management_server, &mut jsonrpc_server);
            combine_server_errors(management_result, jsonrpc_result)
        }
        HttpStopTrigger::Management(_) => jsonrpc_server
            .await
            .err()
            .map(|error| format!("JSON-RPC HTTP 服务停止失败：{}", error)),
        HttpStopTrigger::JsonRpc(_) => management_server
            .await
            .err()
            .map(|error| format!("管理 HTTP 服务停止失败：{}", error)),
    };

    match (primary_error, remaining_error) {
        (Some(primary), Some(remaining)) => Err(format!("{}；{}", primary, remaining)),
        (Some(error), None) | (None, Some(error)) => Err(error),
        (None, None) => Ok(()),
    }
}

fn combine_server_errors(
    management: std::io::Result<()>,
    jsonrpc: std::io::Result<()>,
) -> Option<String> {
    let management = management
        .err()
        .map(|error| format!("管理 HTTP 服务停止失败：{}", error));
    let jsonrpc = jsonrpc
        .err()
        .map(|error| format!("JSON-RPC HTTP 服务停止失败：{}", error));
    match (management, jsonrpc) {
        (Some(management), Some(jsonrpc)) => Some(format!("{}；{}", management, jsonrpc)),
        (Some(error), None) | (None, Some(error)) => Some(error),
        (None, None) => None,
    }
}

async fn wait_for_http_shutdown(mut receiver: watch::Receiver<bool>) {
    if *receiver.borrow() {
        return;
    }
    while receiver.changed().await.is_ok() {
        if *receiver.borrow() {
            return;
        }
    }
}

async fn wait_for_shutdown_signal() -> Result<String, String> {
    tokio::signal::ctrl_c()
        .await
        .map(|()| "收到停止信号".to_string())
        .map_err(|error| format!("等待停止信号失败：{}", error))
}

fn parse_trusted_proxy_ips() -> Result<Vec<IpAddr>, String> {
    let value = env::var(TRUSTED_PROXY_IPS_ENV).unwrap_or_default();
    let mut addresses = Vec::new();
    for item in value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let address = item
            .parse::<IpAddr>()
            .map_err(|error| format!("解析可信代理地址失败：{}（{}）", item, error))?;
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    Ok(addresses)
}

fn parse_bool_env(name: &str, default: bool) -> Result<bool, String> {
    let Some(value) = env::var(name).ok().filter(|value| !value.trim().is_empty()) else {
        return Ok(default);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!("解析布尔配置失败：{}={}", name, value)),
    }
}

fn default_local_app_data_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        return home_dir_fallback()
            .join("Library")
            .join("Application Support")
            .join("motrix-fnos");
    }

    if let Some(path) = env::var_os("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return path.join("motrix-fnos");
    }

    home_dir_fallback()
        .join(".local")
        .join("share")
        .join("motrix-fnos")
}

fn home_dir_fallback() -> PathBuf {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
