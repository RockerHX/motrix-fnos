mod args;
mod config_status;
mod log_mode;
mod logging;
mod options;
mod ports;
mod process_probe;
mod rpc;
mod runtime_file;
mod session;
mod sidecar;

pub use args::{process_args, process_args_with_log_level, summarize_args};
pub use config_status::Aria2ConfigStatus;
pub use log_mode::{Aria2LogLevel, Aria2LogModeCoordinator, Aria2LogModeStatus};
pub(crate) use log_mode::{Aria2LogModeChange, Aria2LogModeWorkerAction};
pub use logging::{
    ARIA2_DETAILED_LOG_LEVEL, ARIA2_LOG_LEVEL, ARIA2_LOG_MAX_BYTES, ARIA2_LOG_MAX_FILES,
    ARIA2_LOG_MAX_SIZE_MIB,
};
pub use options::{apply_global_options, global_options_from_values, Aria2GlobalOptions};
pub use ports::{
    rpc_port_candidates, rpc_ports_exhausted_message, select_available_rpc_port,
    select_rpc_port_with_saved_runtime,
};
pub(crate) use rpc::change_global_log_level;
pub(crate) use rpc::Aria2RpcError;
pub use rpc::{ping_rpc, Aria2RpcClient, Aria2RpcStatus};
pub use runtime_file::{runtime_config, SavedAria2Runtime};
pub use session::save_session;
pub use sidecar::{classify_saved_sidecar, cleanup_saved_sidecar_if_owned, SidecarOwnership};

use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_rpc_secret() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("motrix-fnos-{nanos}-{}", std::process::id())
}

#[cfg(test)]
mod tests;
