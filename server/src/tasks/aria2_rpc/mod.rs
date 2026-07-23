mod control;
mod create;
mod query;
mod transport;

pub(crate) use control::{
    build_change_option_request, build_gid_control_request, send_gid_control_request,
};
pub use control::{change_task_options, pause_task, remove_task, unpause_task};
pub use create::{add_torrent_to_aria2, add_uri_to_aria2};
pub(crate) use create::{build_add_torrent_request, build_add_uri_request};
pub(crate) use query::{
    build_tell_many_request, build_tell_status_request, task_exists, tell_status,
};
pub(crate) use transport::TellManyResponse;
