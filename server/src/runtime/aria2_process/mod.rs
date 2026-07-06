mod resolve;
mod start;
mod status;
mod stop;
mod types;

pub use resolve::resolve_aria2_binary;
pub use start::{ensure_aria2_ready, start_process};
pub use status::process_status;
pub use stop::stop_process;
pub use types::{Aria2ProcessStatus, ManagedAria2Process, ResolvedAria2Binary};

#[cfg(test)]
mod tests;
