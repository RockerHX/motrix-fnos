use crate::config::aria2::Aria2Config;
pub(super) fn rpc_params(config: &Aria2Config) -> Vec<serde_json::Value> {
    let mut params = Vec::new();
    if !config.rpc_secret.is_empty() {
        params.push(serde_json::json!(format!("token:{}", config.rpc_secret)));
    }
    params
}
