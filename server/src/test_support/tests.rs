use super::*;

#[tokio::test]
async fn owned_states_are_isolated_and_close_processes_and_database_before_removal() {
    let (first, second) = tokio::join!(
        TestAppState::with_rpc("resource-isolation", 1),
        TestAppState::new("resource-isolation"),
    );
    let first_root = first.runtime.app_data_dir.clone();
    let second_root = second.runtime.app_data_dir.clone();
    assert_ne!(first_root, second_root);
    let first_state = Arc::clone(&first);
    assert!(first_state.aria2_process.lock().unwrap().is_some());
    first.close().await;
    assert!(first_state.aria2_process.lock().unwrap().is_none());
    assert!(first_state.core.database.pool.is_closed());
    assert!(!first_root.exists());
    assert!(second_root.exists());
    assert!(!second.core.database.pool.is_closed());
    second.close().await;
    assert!(!second_root.exists());
}
