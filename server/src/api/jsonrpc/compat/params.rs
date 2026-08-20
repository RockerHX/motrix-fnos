use super::model::SUPPORTED_KEYS;
use crate::api::jsonrpc::types::{positional_params, strip_token_param, RpcFault};
use serde_json::Value;

pub(super) const MAX_PAGE_SIZE: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskLane {
    Active,
    Waiting,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlOperation {
    Pause,
    Unpause,
    Remove,
    RemoveDownloadResult,
    PauseAll,
    UnpauseAll,
    PurgeDownloadResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CompatCommand {
    GlobalStat,
    Tell {
        lane: TaskLane,
        offset: i64,
        num: u64,
        keys: Vec<String>,
    },
    Control {
        operation: ControlOperation,
        gid: Option<String>,
    },
}

pub(super) fn parse_method(method: &str, params: &Value) -> Result<CompatCommand, RpcFault> {
    match method {
        "aria2.getGlobalStat" => {
            parse_no_args(params, method)?;
            Ok(CompatCommand::GlobalStat)
        }
        "aria2.tellActive" => Ok(CompatCommand::Tell {
            lane: TaskLane::Active,
            offset: 0,
            num: MAX_PAGE_SIZE,
            keys: parse_keys(params, method)?,
        }),
        "aria2.tellWaiting" => {
            let (offset, num, keys) = parse_page(params, method)?;
            Ok(CompatCommand::Tell {
                lane: TaskLane::Waiting,
                offset,
                num,
                keys,
            })
        }
        "aria2.tellStopped" => {
            let (offset, num, keys) = parse_page(params, method)?;
            Ok(CompatCommand::Tell {
                lane: TaskLane::Stopped,
                offset,
                num,
                keys,
            })
        }
        "aria2.pause" => Ok(CompatCommand::Control {
            operation: ControlOperation::Pause,
            gid: Some(parse_gid(params, method)?),
        }),
        "aria2.unpause" => Ok(CompatCommand::Control {
            operation: ControlOperation::Unpause,
            gid: Some(parse_gid(params, method)?),
        }),
        "aria2.remove" => Ok(CompatCommand::Control {
            operation: ControlOperation::Remove,
            gid: Some(parse_gid(params, method)?),
        }),
        "aria2.removeDownloadResult" => Ok(CompatCommand::Control {
            operation: ControlOperation::RemoveDownloadResult,
            gid: Some(parse_gid(params, method)?),
        }),
        "aria2.pauseAll" => {
            parse_no_args(params, method)?;
            Ok(CompatCommand::Control {
                operation: ControlOperation::PauseAll,
                gid: None,
            })
        }
        "aria2.unpauseAll" => {
            parse_no_args(params, method)?;
            Ok(CompatCommand::Control {
                operation: ControlOperation::UnpauseAll,
                gid: None,
            })
        }
        "aria2.purgeDownloadResult" => {
            parse_no_args(params, method)?;
            Ok(CompatCommand::Control {
                operation: ControlOperation::PurgeDownloadResult,
                gid: None,
            })
        }
        _ => Err(RpcFault::method_not_found(format!(
            "Method not found: {method}"
        ))),
    }
}

fn parse_no_args(params: &Value, method: &str) -> Result<(), RpcFault> {
    let params = strip_token_param(positional_params(params)?);
    if params.is_empty() {
        Ok(())
    } else {
        Err(RpcFault::invalid_params(format!(
            "{method} does not accept parameters"
        )))
    }
}

fn parse_gid(params: &Value, method: &str) -> Result<String, RpcFault> {
    let params = strip_token_param(positional_params(params)?);
    if params.len() != 1 {
        return Err(RpcFault::invalid_params(format!(
            "{method} requires exactly one GID"
        )));
    }
    params[0]
        .as_str()
        .map(str::trim)
        .filter(|gid| !gid.is_empty())
        .map(str::to_string)
        .ok_or_else(|| RpcFault::invalid_params(format!("{method} requires a non-empty GID")))
}

fn parse_keys(params: &Value, method: &str) -> Result<Vec<String>, RpcFault> {
    let params = strip_token_param(positional_params(params)?);
    match params {
        [] => Ok(all_keys()),
        [keys] => parse_keys_value(keys, method),
        _ => Err(RpcFault::invalid_params(format!(
            "{method} accepts at most one keys array"
        ))),
    }
}

fn parse_page(params: &Value, method: &str) -> Result<(i64, u64, Vec<String>), RpcFault> {
    let params = strip_token_param(positional_params(params)?);
    match params {
        [] => Ok((0, MAX_PAGE_SIZE, all_keys())),
        [offset, num] => Ok((
            parse_offset(offset, method)?,
            parse_num(num, method)?,
            all_keys(),
        )),
        [offset, num, keys] => Ok((
            parse_offset(offset, method)?,
            parse_num(num, method)?,
            parse_keys_value(keys, method)?,
        )),
        _ => Err(RpcFault::invalid_params(format!(
            "{method} requires offset, num and optional keys"
        ))),
    }
}

fn parse_offset(value: &Value, method: &str) -> Result<i64, RpcFault> {
    value
        .as_i64()
        .ok_or_else(|| RpcFault::invalid_params(format!("{method} offset must be an integer")))
}

fn parse_num(value: &Value, method: &str) -> Result<u64, RpcFault> {
    let num = value.as_u64().ok_or_else(|| {
        RpcFault::invalid_params(format!("{method} num must be a non-negative integer"))
    })?;
    if num > MAX_PAGE_SIZE {
        return Err(RpcFault::invalid_params(format!(
            "{method} num exceeds the server limit of {MAX_PAGE_SIZE}"
        )));
    }
    Ok(num)
}

fn parse_keys_value(value: &Value, method: &str) -> Result<Vec<String>, RpcFault> {
    let Some(keys) = value.as_array() else {
        return Err(RpcFault::invalid_params(format!(
            "{method} keys must be an array"
        )));
    };
    if keys.is_empty() {
        return Ok(all_keys());
    }

    let mut parsed = Vec::with_capacity(keys.len());
    for key in keys {
        let Some(key) = key.as_str() else {
            return Err(RpcFault::invalid_params(format!(
                "{method} keys must contain strings"
            )));
        };
        if !SUPPORTED_KEYS.contains(&key) {
            return Err(RpcFault::invalid_params(format!(
                "{method} does not support key {key}"
            )));
        }
        if !parsed.iter().any(|item| item == key) {
            parsed.push(key.to_string());
        }
    }
    Ok(parsed)
}

fn all_keys() -> Vec<String> {
    SUPPORTED_KEYS
        .iter()
        .map(|key| (*key).to_string())
        .collect()
}
