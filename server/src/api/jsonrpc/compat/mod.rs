mod model;
mod params;

use super::auth::ensure_compat_token;
use super::types::RpcFault;
use super::JsonRpcAccess;
use crate::app::HttpAppState;
use model::Aria2GlobalStat;
use params::{parse_method, CompatCommand, ControlOperation};
use serde_json::Value;
use std::sync::Arc;

pub(super) async fn dispatch(
    state: &Arc<HttpAppState>,
    access: JsonRpcAccess,
    method: &str,
    params: &Value,
) -> Result<Value, RpcFault> {
    ensure_compat_token(state, access, params).await?;
    let command = parse_method(method, params)?;

    match command {
        CompatCommand::GlobalStat => Ok(Aria2GlobalStat::empty().to_value()),
        CompatCommand::Tell { keys, .. } => Ok(model::serialize_tasks(&[], &keys)),
        CompatCommand::Control { operation, gid } => {
            let Some(gid) = gid.as_deref() else {
                return Err(RpcFault::server_error(control_not_ready_message(
                    operation, None,
                )));
            };
            let task = crate::api::tasks::task_service(state)
                .get_download_task_by_gid(gid)
                .map_err(RpcFault::server_error)?;
            if task.is_none() {
                return Err(RpcFault::gid_not_found(gid));
            }
            Err(RpcFault::server_error(control_not_ready_message(
                operation,
                Some(gid),
            )))
        }
    }
}

fn control_not_ready_message(operation: ControlOperation, gid: Option<&str>) -> String {
    let operation = match operation {
        ControlOperation::Pause => "pause",
        ControlOperation::Unpause => "unpause",
        ControlOperation::Remove => "remove",
        ControlOperation::RemoveDownloadResult => "removeDownloadResult",
        ControlOperation::PauseAll => "pauseAll",
        ControlOperation::UnpauseAll => "unpauseAll",
        ControlOperation::PurgeDownloadResult => "purgeDownloadResult",
    };
    match gid {
        Some(gid) => format!("Motrix Extension 兼容方法 {operation} 尚未接入任务服务（GID {gid}）"),
        None => format!("Motrix Extension 兼容方法 {operation} 尚未接入任务服务"),
    }
}

#[cfg(test)]
mod tests;
