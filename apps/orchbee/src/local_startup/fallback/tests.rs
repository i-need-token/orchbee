use super::with_start_recorded;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

// The TUI never waits for its start record, and its process does not go on before that record is
// written: the record here can finish only after the TUI has run.
#[test]
fn the_start_record_never_delays_the_tui_and_finishes_before_it_returns() {
    let (release, released) = mpsc::channel::<()>();
    let recorded = Arc::new(AtomicBool::new(false));
    let written = Arc::clone(&recorded);
    let outcome = with_start_recorded(
        move || {
            released.recv_timeout(Duration::from_secs(10)).unwrap();
            written.store(true, Ordering::SeqCst);
        },
        || {
            release.send(()).unwrap();
            "closed"
        },
    );
    assert_eq!(outcome, "closed");
    assert!(recorded.load(Ordering::SeqCst));
}
