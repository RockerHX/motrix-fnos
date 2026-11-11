use crate::tasks::{DownloadTask, DownloadTaskSourceType, DownloadTaskStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskFileAvailability {
    Available,
    TaskNotComplete,
    FilesDeleted,
    PathMissing,
    PartialMissing,
    StorageUnavailable,
    PathUnauthorized,
    UnsupportedLayout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskFileActions {
    pub availability: TaskFileAvailability,
    pub file_manager_path: Option<String>,
    pub open_file_path: Option<String>,
    pub detail_paths: Vec<String>,
}

impl TaskFileActions {
    fn unavailable(availability: TaskFileAvailability) -> Self {
        Self {
            availability,
            file_manager_path: None,
            open_file_path: None,
            detail_paths: Vec::new(),
        }
    }
}

pub fn task_file_actions(task: &DownloadTask, accessible_roots: &[String]) -> TaskFileActions {
    if task.status != DownloadTaskStatus::Complete {
        return TaskFileActions::unavailable(TaskFileAvailability::TaskNotComplete);
    }
    if task.files_deleted {
        return TaskFileActions::unavailable(TaskFileAvailability::FilesDeleted);
    }

    match task.source_type {
        DownloadTaskSourceType::Url => url_task_actions(task, accessible_roots),
        DownloadTaskSourceType::Torrent | DownloadTaskSourceType::Magnet => {
            bt_task_actions(task, accessible_roots)
        }
    }
}

fn url_task_actions(task: &DownloadTask, accessible_roots: &[String]) -> TaskFileActions {
    let Some(target) = task
        .file_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return TaskFileActions::unavailable(TaskFileAvailability::PathMissing);
    };
    match validate_target(target, accessible_roots, TargetKind::File) {
        Ok(path) => TaskFileActions {
            availability: TaskFileAvailability::Available,
            file_manager_path: Some(path.clone()),
            open_file_path: Some(path.clone()),
            detail_paths: vec![path],
        },
        Err(availability) => TaskFileActions::unavailable(availability),
    }
}

fn bt_task_actions(task: &DownloadTask, accessible_roots: &[String]) -> TaskFileActions {
    let Some(target) = task
        .owned_task_dir
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return TaskFileActions::unavailable(TaskFileAvailability::UnsupportedLayout);
    };
    let path = match validate_target(target, accessible_roots, TargetKind::Directory) {
        Ok(path) => path,
        Err(availability) => return TaskFileActions::unavailable(availability),
    };
    let mut present = 0;
    let mut missing = 0;
    for file in task.files.iter().filter(|file| file.selected) {
        let relative = Path::new(&file.path).strip_prefix(target);
        if !relative.is_ok_and(|path| {
            !path.as_os_str().is_empty()
                && path
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
        }) {
            return TaskFileActions::unavailable(TaskFileAvailability::UnsupportedLayout);
        }
        match validate_target(&file.path, std::slice::from_ref(&path), TargetKind::File) {
            Ok(_) => present += 1,
            Err(TaskFileAvailability::PathMissing) => missing += 1,
            Err(availability) => return TaskFileActions::unavailable(availability),
        }
    }
    if missing > 0 && present == 0 {
        return TaskFileActions::unavailable(TaskFileAvailability::PathMissing);
    }
    TaskFileActions {
        availability: if missing > 0 {
            TaskFileAvailability::PartialMissing
        } else {
            TaskFileAvailability::Available
        },
        file_manager_path: Some(path.clone()),
        open_file_path: None,
        detail_paths: vec![path],
    }
}

#[derive(Clone, Copy)]
enum TargetKind {
    File,
    Directory,
}

fn validate_target(
    target: &str,
    accessible_roots: &[String],
    kind: TargetKind,
) -> Result<String, TaskFileAvailability> {
    let original_target = target.to_string();
    let target = Path::new(target);
    if !target.is_absolute() {
        return Err(TaskFileAvailability::UnsupportedLayout);
    }
    let candidate_roots = accessible_roots
        .iter()
        .map(PathBuf::from)
        .filter(|root| target.starts_with(root))
        .collect::<Vec<_>>();
    if candidate_roots.is_empty() {
        return Err(TaskFileAvailability::PathUnauthorized);
    }

    let roots = candidate_roots
        .iter()
        .filter_map(|root| root.canonicalize().ok().filter(|path| path.is_dir()))
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err(TaskFileAvailability::StorageUnavailable);
    }
    let metadata = std::fs::symlink_metadata(target).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            TaskFileAvailability::PathMissing
        } else {
            TaskFileAvailability::StorageUnavailable
        }
    })?;
    if metadata.file_type().is_symlink()
        || !match kind {
            TargetKind::File => metadata.is_file(),
            TargetKind::Directory => metadata.is_dir(),
        }
    {
        return Err(TaskFileAvailability::UnsupportedLayout);
    }

    let target = target
        .canonicalize()
        .map_err(|_| TaskFileAvailability::StorageUnavailable)?;
    let authorized = roots.iter().any(|root| target.starts_with(root));
    if !authorized {
        return Err(TaskFileAvailability::PathUnauthorized);
    }

    Ok(original_target)
}

#[cfg(test)]
mod tests;
