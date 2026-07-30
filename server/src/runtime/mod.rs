pub mod aria2_process;
pub mod lifecycle;
pub mod shutdown;
pub mod task_monitor;
mod task_operation_reconcile;

pub use aria2_process::{
    ensure_aria2_ready, process_status, resolve_aria2_binary, start_aria2, start_process,
    stop_aria2, stop_process, Aria2ProcessStatus, ManagedAria2Process, ResolvedAria2Binary,
};
pub use lifecycle::{
    Aria2Lease, Aria2LeaseKind, Aria2LifecycleCoordinator, Aria2LifecycleCoordinatorSnapshot,
    Aria2LifecyclePhase, Aria2LifecyclePolicy, Aria2LifecycleSnapshot,
};
pub use shutdown::run_shutdown_cleanup;
pub(crate) use task_monitor::current_tasks_snapshot;
pub use task_monitor::{
    broadcast_tasks_snapshot, monitor_tasks_once, spawn_task_monitor, visible_tasks_snapshot,
};
pub use task_operation_reconcile::reconcile_unfinished_task_operations;
