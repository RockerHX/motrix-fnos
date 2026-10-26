use serde::{Deserialize, Serialize};
use std::env;

pub const ARIA2_PATH_ENV: &str = "MOTRIX_FNOS_ARIA2_PATH";
pub const ARIA2_SIDECAR_NAME: &str = "aria2-next";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum EngineGeneration {
    #[serde(rename = "legacy-v1")]
    LegacyV1,
    #[serde(rename = "native-v2")]
    NativeV2,
    #[default]
    #[serde(rename = "unknown")]
    Unknown,
}

impl EngineGeneration {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::LegacyV1 => "legacy-v1",
            Self::NativeV2 => "native-v2",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "legacy-v1" => Self::LegacyV1,
            "native-v2" => Self::NativeV2,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub session_path: Option<String>,
    pub state_dir: Option<String>,
    pub log_path: Option<String>,
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
            session_path: None,
            state_dir: None,
            log_path: None,
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
