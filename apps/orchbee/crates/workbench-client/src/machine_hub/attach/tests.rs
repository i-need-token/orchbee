//! The cancellation that the owner's `close` relies on to start no hub.

use super::Cancel;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[test]
fn once_cancel_returns_no_start_is_under_way_and_none_begins() {
    let cancel = Arc::new(Cancel::default());
    let finished = Arc::new(AtomicBool::new(false));
    let (entered, started) = mpsc::channel();
    let starter = {
        let (cancel, finished) = (cancel.clone(), finished.clone());
        thread::spawn(move || {
            cancel.start(|| {
                entered.send(()).unwrap();
                // A start that takes a while, as spawning a process may.
                thread::sleep(Duration::from_millis(100));
                finished.store(true, Ordering::Release);
            })
        })
    };
    started.recv().unwrap();
    cancel.cancel();
    assert!(
        finished.load(Ordering::Acquire),
        "cancel returned while a start was under way"
    );
    assert!(starter.join().unwrap().is_some());
    assert!(cancel.start(|| ()).is_none(), "a start began after cancel");
}
