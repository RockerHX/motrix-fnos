pub mod aria2_process;

pub use aria2_process::{
    ensure_aria2_ready, process_status, resolve_aria2_binary, start_process, stop_process,
    Aria2ProcessStatus, ManagedAria2Process, ResolvedAria2Binary,
};
