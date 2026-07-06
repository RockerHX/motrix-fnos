use crate::config::aria2::{Aria2BinarySource, Aria2Config};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Aria2ConfigStatus {
    pub configured: bool,
    pub path: Option<String>,
    pub path_exists: bool,
    pub binary_source: Aria2BinarySource,
    pub sidecar_name: String,
    pub target_triple: String,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_secret_configured: bool,
    pub ca_certificate_path: Option<String>,
}

impl Aria2ConfigStatus {
    pub fn from_config(config: &Aria2Config) -> Self {
        let path_exists = config
            .aria2_path
            .as_deref()
            .map(|path| Path::new(path).is_file())
            .unwrap_or(false);

        Self {
            configured: config.aria2_path.is_some()
                || config.binary_source == Aria2BinarySource::Sidecar,
            path: config.aria2_path.clone(),
            path_exists,
            binary_source: config.binary_source.clone(),
            sidecar_name: config.sidecar_name.clone(),
            target_triple: config.target_triple.clone(),
            rpc_host: config.rpc_host.clone(),
            rpc_port: config.rpc_port,
            rpc_secret_configured: !config.rpc_secret.is_empty(),
            ca_certificate_path: super::detect_ca_certificate_path()
                .map(|path| path.display().to_string()),
        }
    }
}
