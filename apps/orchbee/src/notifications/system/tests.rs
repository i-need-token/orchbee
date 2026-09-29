use super::*;
#[test]
fn notification_balloon_utf16_bounds_never_split_surrogates() {
    assert_eq!(bounded_utf16("😀😀a", 4), "😀");
    assert_eq!(bounded_utf16("😀😀a", 5), "😀😀");
    assert_eq!(
        bounded_utf16(&"😀".repeat(80), 64).encode_utf16().count(),
        62
    );
}
#[test]
fn notification_owner_drop_signals_shutdown_releases_channel_and_joins() {
    let (sender, receiver) = mpsc::sync_channel(1);
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let (done, finished) = mpsc::sync_channel(1);
    let task = thread::spawn(move || {
        assert!(receiver.recv().is_err());
        assert!(flag.load(Ordering::Acquire));
        done.send(()).unwrap();
    });
    drop(SystemNotifications {
        sender: Some(sender),
        stop,
        task: Some(task),
    });
    finished.try_recv().unwrap();
}
