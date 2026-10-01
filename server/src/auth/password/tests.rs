use super::*;
use std::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn aborted_request_keeps_hash_slot_until_blocking_work_finishes() {
    let slots = PasswordHashSlots::new(1);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let request = {
        let slots = slots.clone();
        tokio::spawn(async move {
            slots
                .run(move || {
                    let _ = started_tx.send(());
                    let _ = release_rx.recv();
                    Ok(())
                })
                .await
        })
    };

    started_rx.await.expect("blocking work should start");
    request.abort();
    assert!(matches!(
        slots.run(|| Ok(())).await,
        Err(AuthError::PasswordHashBusy)
    ));

    release_tx
        .send(())
        .expect("blocking work should be released");
    let permit = tokio::time::timeout(
        Duration::from_secs(5),
        Arc::clone(&slots.semaphore).acquire_owned(),
    )
    .await
    .expect("slot should be released after work finishes")
    .expect("semaphore should remain open");
    drop(permit);
    assert_eq!(slots.run(|| Ok(())).await, Ok(()));
}
