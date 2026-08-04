mod control;
mod create;
pub(crate) mod query;
mod transport;

#[cfg(test)]
pub(crate) use control::build_change_option_request;
pub(crate) use control::is_aria2_outcome_unknown_error;
pub(crate) use control::{build_gid_control_request, send_gid_control_request};
pub use control::{
    change_task_options, change_task_options_with_request_id, pause_task,
    pause_task_with_request_id, remove_task, remove_task_with_request_id, unpause_task,
    unpause_task_with_request_id, Aria2TaskOptionError,
};
pub use create::{add_torrent_to_aria2, add_uri_to_aria2, Aria2TaskCreationError};
#[cfg(test)]
pub(crate) use create::{build_add_torrent_request, build_add_uri_request};
pub(crate) use create::{build_add_torrent_request_with_id, build_add_uri_request_with_id};
pub(crate) use query::{
    build_tell_many_request, build_tell_status_request, task_exists, tell_active_task_activity,
    tell_status,
};
