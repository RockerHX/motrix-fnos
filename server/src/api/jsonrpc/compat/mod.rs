mod control;
mod model;
mod params;
mod read;

use super::auth::ensure_compat_token;
use super::types::RpcFault;
use super::JsonRpcAccess;
use crate::app::HttpAppState;
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
        CompatCommand::GlobalStat => read::global_stat(state),
        CompatCommand::Tell {
            lane,
            offset,
            num,
            keys,
        } => read::tell(state, lane, offset, num, &keys),
        CompatCommand::Control { operation, gid } => match gid.as_deref() {
            Some(gid) => control::execute(state, operation, gid).await,
            None if matches!(
                operation,
                ControlOperation::PauseAll
                    | ControlOperation::UnpauseAll
                    | ControlOperation::PurgeDownloadResult
            ) =>
            {
                control::execute_batch(state, operation).await
            }
            None => Err(RpcFault::server_error(control_not_ready_message(
                operation, None,
            ))),
        },
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
