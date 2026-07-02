pub mod aria2_process;
pub mod shutdown;
pub mod task_monitor;

pub use aria2_process::{
    ensure_aria2_ready, process_status, resolve_aria2_binary, start_process, stop_process,
    Aria2ProcessStatus, ManagedAria2Process, ResolvedAria2Binary,
};
pub use shutdown::run_shutdown_cleanup;
pub use task_monitor::{
    broadcast_tasks_snapshot, monitor_tasks_once, spawn_task_monitor, visible_tasks_snapshot,
};
