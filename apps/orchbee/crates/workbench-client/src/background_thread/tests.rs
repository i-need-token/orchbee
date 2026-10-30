use std::{
    io,
    sync::mpsc,
    time::{Duration, Instant},
};

use super::BackgroundThread;

/// Injected fault: a thread that ignores its stop request until the test releases it.
fn stuck() -> (BackgroundThread, mpsc::Sender<()>) {
    let (release, released) = mpsc::channel::<()>();
    let thread = BackgroundThread::spawn("orchbee-background-test", move || {
        let _ = released.recv();
    })
    .unwrap();
    (thread, release)
}

#[test]
fn a_thread_running_at_the_deadline_times_out_and_stays_owned() {
    let (mut thread, release) = stuck();
    let error = thread
        .join_before(Instant::now() + Duration::from_millis(20))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(!thread.is_finished());
    release.send(()).unwrap();
    thread
        .join_before(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert!(thread.is_finished());
}

#[test]
fn dropping_a_running_thread_detaches_it_without_waiting() {
    let (thread, release) = stuck();
    let started = Instant::now();
    drop(thread);
    assert!(started.elapsed() < Duration::from_secs(1));
    release.send(()).unwrap();
}

#[test]
fn a_panicked_thread_reports_a_failure_instead_of_success() {
    let mut thread = BackgroundThread::spawn("orchbee-background-test", || {
        panic!("injected background failure");
    })
    .unwrap();
    let error = thread
        .join_before(Instant::now() + Duration::from_secs(5))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
}
