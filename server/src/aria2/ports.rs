use super::runtime_file::SavedAria2Runtime;
use super::sidecar::{classify_saved_sidecar, cleanup_saved_sidecar_if_owned, SidecarOwnership};
use crate::config::aria2::Aria2Config;
use crate::debug_logs::DebugLogStore;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

fn rpc_port_in_use(config: &Aria2Config) -> bool {
    let Ok(addresses) = (config.rpc_host.as_str(), config.rpc_port).to_socket_addrs() else {
        return false;
    };

    addresses
        .into_iter()
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(200)).is_ok())
}

pub fn rpc_ports_exhausted_message() -> String {
    "Aria2 RPC 端口范围 6800, 16800-16820 均被占用，无法启动内置引擎".to_string()
}

pub fn rpc_port_candidates() -> Vec<u16> {
    std::iter::once(6800).chain(16800..=16820).collect()
}

pub fn select_available_rpc_port(config: &Aria2Config) -> Option<u16> {
    select_available_rpc_port_from(config, rpc_port_candidates())
}

pub fn select_rpc_port_with_saved_runtime(
    config: &Aria2Config,
    saved: Option<&SavedAria2Runtime>,
    debug_logs: &DebugLogStore,
) -> Option<u16> {
    for port in rpc_port_candidates() {
        let mut candidate_config = config.clone();
        candidate_config.rpc_port = port;
        if !rpc_port_in_use(&candidate_config) {
            return Some(port);
        }

        match classify_saved_sidecar(saved, port, debug_logs) {
            SidecarOwnership::OwnSidecar => {
                if !cleanup_saved_sidecar_if_owned(saved, port, debug_logs) {
                    debug_logs.error(
                        "aria2.cleanup",
                        format!(
                            "检测到本应用残留 sidecar 占用端口 {}，但清理失败，停止启动新 Aria2 避免继续下载",
                            port
                        ),
                    );
                    return None;
                }

                std::thread::sleep(Duration::from_millis(300));
                if !rpc_port_in_use(&candidate_config) {
                    return Some(port);
                }

                debug_logs.warn(
                    "aria2.cleanup",
                    format!("清理本应用残留 sidecar 后端口 {} 仍被占用", port),
                );
                return None;
            }
            SidecarOwnership::ExternalOrUnknown => {
                debug_logs.info(
                    "aria2.cleanup",
                    format!(
                        "端口 {} 已被占用但未确认属于本应用 sidecar，跳过该端口",
                        port
                    ),
                );
            }
        }
    }

    None
}

fn select_available_rpc_port_from(
    config: &Aria2Config,
    candidates: impl IntoIterator<Item = u16>,
) -> Option<u16> {
    candidates.into_iter().find(|port| {
        let mut candidate_config = config.clone();
        candidate_config.rpc_port = *port;
        !rpc_port_in_use(&candidate_config)
    })
}
