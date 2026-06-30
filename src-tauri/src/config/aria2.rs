use serde::Serialize;
use std::env;

pub const ARIA2_PATH_ENV: &str = "MOTRIX_FNOS_ARIA2_PATH";
pub const ARIA2_SIDECAR_NAME: &str = "aria2-next";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Aria2BinarySource {
    ExternalPath,
    Sidecar,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2Config {
    pub aria2_path: Option<String>,
    pub binary_source: Aria2BinarySource,
    pub sidecar_name: String,
    pub target_triple: String,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_secret: String,
}

impl Aria2Config {
    pub fn from_env() -> Self {
        let aria2_path = env::var(ARIA2_PATH_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            binary_source: if aria2_path.is_some() {
                Aria2BinarySource::ExternalPath
            } else {
                Aria2BinarySource::Sidecar
            },
            aria2_path,
            sidecar_name: ARIA2_SIDECAR_NAME.to_string(),
            target_triple: current_target_triple().to_string(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: String::new(),
        }
    }

    pub fn rpc_url(&self) -> String {
        format!("http://{}:{}/jsonrpc", self.rpc_host, self.rpc_port)
    }
}

fn current_target_triple() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "aarch64-unknown-linux-gnu"
    } else {
        "unknown"
    }
}
