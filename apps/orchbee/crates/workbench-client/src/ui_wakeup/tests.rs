use super::*;
use std::sync::atomic::AtomicU8;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn notifications_coalesce_without_erasing_a_publication_after_consumption() {
    let wakeup = UiWakeup::current();
    assert!(!wakeup.take());
    wakeup.signal();
    wakeup.signal();
    assert!(wakeup.take());
    assert!(!wakeup.take());
    wakeup.signal();
    assert!(wakeup.take());
    assert!(!wakeup.take());
}

#[test]
fn publication_between_inspection_and_park_keeps_a_wake_token() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let phase = Arc::new(AtomicU8::new(0));
    let waiter_phase = Arc::clone(&phase);
    let waiter = thread::spawn(move || {
        let wakeup = UiWakeup::current();
        sender.send(wakeup.clone()).unwrap();
        assert!(!wakeup.take());
        waiter_phase.store(1, Ordering::Release);
        while waiter_phase.load(Ordering::Acquire) != 2 {
            thread::yield_now();
        }
        // Publish is forced after the negative inspection and before parking.
        // Do not recv on a channel here: std channel waiting can consume this
        // thread's park token and would test a different wait sequence.
        let began = std::time::Instant::now();
        thread::park_timeout(Duration::from_secs(2));
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "pre-published unpark token must interrupt the wait"
        );
        assert!(wakeup.take());
        assert!(!wakeup.take());
    });
    let wakeup = receiver.recv().unwrap();
    while phase.load(Ordering::Acquire) != 1 {
        thread::yield_now();
    }
    wakeup.signal();
    phase.store(2, Ordering::Release);
    waiter.join().unwrap();
}
