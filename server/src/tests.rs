use super::run_with_runtime;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn runtime_shutdown_does_not_wait_for_blocking_tasks() {
    let (started_sender, started_receiver) = mpsc::channel();
    let started_at = Instant::now();

    run_with_runtime(async move {
        let _blocking_task = tokio::task::spawn_blocking(move || {
            started_sender
                .send(())
                .expect("blocking task should report startup");
            std::thread::sleep(Duration::from_secs(2));
        });
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("blocking task should start");
    })
    .expect("runtime should run");

    assert!(
        started_at.elapsed() < Duration::from_secs(1),
        "runtime shutdown should not wait for the blocking task"
    );
}
