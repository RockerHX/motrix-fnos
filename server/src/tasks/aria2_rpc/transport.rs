use crate::config::aria2::Aria2Config;
use crate::tasks::Aria2TaskStatus;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct AddUriResponse {
    pub(super) result: Option<String>,
    pub(super) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GidResponse {
    pub(super) result: Option<String>,
    pub(super) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TellStatusResponse {
    pub(super) result: Option<Aria2TaskStatus>,
    pub(super) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TellManyResponse {
    pub(crate) result: Option<Vec<Aria2TaskStatus>>,
    pub(crate) error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcError {
    pub(crate) message: String,
}

pub(super) fn rpc_params(config: &Aria2Config) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params
}
