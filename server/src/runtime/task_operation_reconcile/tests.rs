use super::*;
use crate::database::connect_database;
use crate::database::task_operations::{begin_task_operation, list_unfinished_task_operations};
use crate::tasks::{DownloadTaskFile, DownloadTaskSourceType, TaskOperationContext};
use std::collections::HashMap;

#[test]
fn prepared_operation_is_marked_failed_without_starting_recovery() {
    let operation = operation(
        TaskOperationType::Create,
        "prepared",
        None,
        None,
        Vec::new(),
    );

    assert_matches_fail(decide_reconcile_action(&operation, &[], &HashMap::new()));
}

#[test]
fn persisted_create_is_completed_when_aria2_gid_matches() {
    let operation = operation(
        TaskOperationType::Create,
        "task_persisted",
        None,
        Some("new-gid"),
        vec!["task_state_persisted"],
    );
    let task = task_with_gid("new-gid");
    let presence = HashMap::from([("new-gid".to_string(), Aria2TaskPresence::Present)]);

    assert!(matches!(
        decide_reconcile_action(&operation, &[task], &presence),
        ReconcileAction::Complete(_)
    ));
}

#[test]
fn unpersisted_aria2_task_is_scheduled_for_removal() {
    let operation = operation(
        TaskOperationType::Create,
        "aria2_created",
        None,
        Some("new-gid"),
        vec!["aria2_task_created"],
    );
    let presence = HashMap::from([("new-gid".to_string(), Aria2TaskPresence::Present)]);

    assert!(matches!(
        decide_reconcile_action(&operation, &[], &presence),
        ReconcileAction::RemoveUnpersistedAria2Task(gid) if gid == "new-gid"
    ));
}

#[test]
fn staged_user_files_always_require_manual_review() {
    let operation = operation(
        TaskOperationType::Redownload,
        "files_staged",
        Some("old-gid"),
        Some("new-gid"),
        vec!["old_files_staged"],
    );
    let presence = HashMap::from([("new-gid".to_string(), Aria2TaskPresence::Present)]);

    assert_matches_manual(decide_reconcile_action(
        &operation,
        &[task_with_gid("new-gid")],
        &presence,
    ));
}

#[test]
fn missing_persisted_gid_requires_manual_review() {
    let operation = operation(
        TaskOperationType::Restore,
        "task_restored",
        Some("old-gid"),
        Some("new-gid"),
        vec!["task_state_persisted"],
    );
    let presence = HashMap::from([("new-gid".to_string(), Aria2TaskPresence::Missing)]);

    assert_matches_manual(decide_reconcile_action(
        &operation,
        &[task_with_gid("new-gid")],
        &presence,
    ));
}

#[tokio::test]
async fn manual_review_persists_visible_task_error_with_operation() {
    let path = temp_path("manual-review");
    let database = connect_database(path.clone())
        .await
        .expect("database should connect");
    let mut operation = operation(
        TaskOperationType::Redownload,
        "files_staged",
        Some("old-gid"),
        Some("new-gid"),
        vec!["old_files_staged"],
    );
    let mut tasks = vec![task_with_gid("new-gid")];
    begin_task_operation(&database.pool, &operation)
        .await
        .expect("operation should persist");

    apply_reconcile_action(
        &database.pool,
        &mut tasks,
        &mut operation,
        ReconcileAction::ManualReview("用户文件已保留，需要人工处理".to_string()),
    )
    .await
    .expect("manual review should persist");

    assert_eq!(tasks[0].status, DownloadTaskStatus::Error);
    assert_eq!(
        tasks[0].error_message.as_deref(),
        Some("用户文件已保留，需要人工处理")
    );
    let unfinished = list_unfinished_task_operations(&database.pool)
        .await
        .expect("unfinished operations should list");
    assert_eq!(unfinished.len(), 1);
    assert_eq!(unfinished[0].status, TaskOperationStatus::ManualReview);

    database.pool.close().await;
    let _ = std::fs::remove_file(path);
}

fn assert_matches_fail(action: ReconcileAction) {
    assert!(matches!(action, ReconcileAction::Fail(_)));
}

fn assert_matches_manual(action: ReconcileAction) {
    assert!(matches!(action, ReconcileAction::ManualReview(_)));
}

fn operation(
    operation_type: TaskOperationType,
    phase: &str,
    old_gid: Option<&str>,
    new_gid: Option<&str>,
    completed_side_effects: Vec<&str>,
) -> TaskOperation {
    TaskOperation::with_id(
        "operation-1",
        1,
        operation_type,
        phase,
        TaskOperationContext {
            old_gid: old_gid.map(str::to_string),
            new_gid: new_gid.map(str::to_string),
            critical_paths: Vec::new(),
            completed_side_effects: completed_side_effects
                .into_iter()
                .map(str::to_string)
                .collect(),
            task_snapshot: None,
        },
    )
}

fn task_with_gid(gid: &str) -> DownloadTask {
    DownloadTask {
        id: 1,
        url: "https://example.com/archive.zip".to_string(),
        source_type: DownloadTaskSourceType::Url,
        file_name: "archive.zip".to_string(),
        save_dir: "/downloads".to_string(),
        owned_task_dir: None,
        category: "默认".to_string(),
        gid: Some(gid.to_string()),
        status: DownloadTaskStatus::Paused,
        total_length: 1024,
        completed_length: 512,
        download_speed: 0,
        error_code: None,
        error_message: None,
        file_path: Some("/downloads/archive.zip".to_string()),
        metadata_torrent_path: None,
        files_deleted: false,
        selected_file_indexes: Vec::new(),
        confirmation_required: false,
        files: Vec::<DownloadTaskFile>::new(),
        created_at: 1,
        updated_at: 1,
    }
}

fn temp_path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "motrix-fnos-operation-reconcile-{}-{}.sqlite",
        label,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos()
    ))
}
