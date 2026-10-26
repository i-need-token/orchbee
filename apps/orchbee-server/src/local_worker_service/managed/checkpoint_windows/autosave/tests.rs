use super::*;

#[test]
fn debounce_tracks_the_last_commit_and_periodic_capture_needs_no_layout_change() {
    let now = Instant::now();
    let mut schedule = Schedule::new(now);
    schedule.observe(WorkbenchRevision::new(1), now);
    schedule.observe(WorkbenchRevision::new(2), now + Duration::from_millis(400));
    assert!(!schedule.ready(now + Duration::from_millis(899)));
    assert!(schedule.ready(now + Duration::from_millis(900)));
    schedule.captured(WorkbenchRevision::new(2), now + Duration::from_millis(900));
    assert!(!schedule.ready(now + PERIOD));
    assert!(schedule.ready(now + PERIOD + Duration::from_millis(900)));
}

#[test]
fn revision_changed_during_a_delayed_write_remains_dirty_and_failure_retries_are_bounded() {
    let now = Instant::now();
    let mut schedule = Schedule::new(now);
    schedule.observe(WorkbenchRevision::new(1), now);
    schedule.captured(WorkbenchRevision::new(1), now + DEBOUNCE);
    // Publication of revision 1 takes two seconds; a concurrent request commits revision 2.
    let finished = now + Duration::from_secs(2);
    schedule.observe(WorkbenchRevision::new(2), finished);
    assert!(!schedule.ready(finished));
    assert!(schedule.ready(finished + DEBOUNCE));
    schedule.failed(finished + DEBOUNCE);
    assert!(!schedule.ready(finished + DEBOUNCE + RETRY - Duration::from_millis(1)));
    assert!(schedule.ready(finished + DEBOUNCE + RETRY));
    schedule.captured(WorkbenchRevision::new(2), finished + DEBOUNCE + RETRY);
    assert!(!schedule.ready(finished + DEBOUNCE + RETRY));
}

#[test]
fn damaged_empty_baseline_is_suppressed_until_checkpoint_contents_change() {
    let mut suppressed = Some(b"empty-workspace".to_vec());
    assert!(!should_publish(
        SaveKind::Background,
        suppressed.as_deref() == Some(b"empty-workspace".as_slice())
    ));
    assert_eq!(suppressed, Some(b"empty-workspace".to_vec()));
    assert!(should_publish(
        SaveKind::Background,
        suppressed.as_deref() == Some(b"user-created-layout".as_slice())
    ));
    // Failed publication keeps the guard so reverting to the empty baseline stays suppressed.
    assert!(!should_publish(
        SaveKind::Background,
        suppressed.as_deref() == Some(b"empty-workspace".as_slice())
    ));
    suppressed = None;
    assert!(should_publish(
        SaveKind::Background,
        suppressed.as_deref() == Some(b"user-created-layout".as_slice())
    ));
    // Explicit clean stop confirms the current workspace and always permits its final capture.
    assert!(should_publish(SaveKind::Final, true));
}

#[test]
fn stop_timeout_retains_the_in_flight_writer_for_a_later_join() {
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    let mut saver = Saver::spawn(move |_stop| {
        entered.send(()).unwrap();
        release_rx.recv().unwrap();
    })
    .unwrap();
    entered_rx.recv().unwrap();
    assert_eq!(
        saver.stop_before(Instant::now()).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    release.send(()).unwrap();
    saver
        .stop_before(Instant::now() + Duration::from_secs(2))
        .unwrap();
    assert!(saver.join.is_none());
}
