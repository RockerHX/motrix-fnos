use crate::config::aria2::{Aria2BinarySource, Aria2Config};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAria2Runtime {
    pub pid: u32,
    pub actual_port: u16,
    pub rpc_secret: String,
    pub binary_source: Aria2BinarySource,
    pub sidecar_name: Option<String>,
    pub app_data_dir: Option<String>,
    pub aria2_session_path: Option<String>,
    pub aria2_log_path: Option<String>,
}

pub fn runtime_config(base: &Aria2Config, actual_port: u16, rpc_secret: String) -> Aria2Config {
    let mut config = base.clone();
    config.rpc_port = actual_port;
    config.rpc_secret = rpc_secret;
    config
}
