use serde::Serialize;
use std::env;

pub const ARIA2_PATH_ENV: &str = "MOTRIX_FNOS_ARIA2_PATH";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Aria2Config {
    pub aria2_path: Option<String>,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_secret: String,
}

impl Aria2Config {
    pub fn from_env() -> Self {
        Self {
            aria2_path: env::var(ARIA2_PATH_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty()),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 6800,
            rpc_secret: String::new(),
        }
    }

    pub fn rpc_url(&self) -> String {
        format!("http://{}:{}/jsonrpc", self.rpc_host, self.rpc_port)
    }
}
