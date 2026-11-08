use super::*;

/// The queued stop attempt the lifecycle owner takes next.
fn attempt(state: &SessionShutdown) -> Attempt {
    match state.take_within(Duration::ZERO) {
        Some(Work::Stop(attempt)) => attempt,
        other => panic!("expected a stop attempt: {other:?}"),
    }
}

fn request(id: u8, action: StopAction) -> SessionStopRequest {
    SessionStopRequest {
        id: StopId::new([id; 16]).unwrap(),
        action,
    }
}

#[test]
fn stop_freezes_admission_without_losing_existing_record_ownership() {
    let state = SessionShutdown::default();
    assert!(!state.stopping());
    assert_eq!(
        state.request(&request(1, StopAction::Status)).status,
        SessionStopStatus::NotRecorded
    );
    assert_eq!(
        state.request(&request(1, StopAction::Retry)).status,
        SessionStopStatus::NotRecorded
    );
    let record = state.enter().unwrap();
    assert!(matches!(
        state.request(&request(1, StopAction::Start)).status,
        SessionStopStatus::Pending { .. }
    ));
    assert!(state.stopping());
    assert!(state.enter().is_none());
    // A Session stop keeps refusing reads as well, unlike a drain.
    assert!(state.enter_read().is_none());
    assert!(!state.settle_records(Instant::now()));
    drop(record);
    assert!(state.settle_records(Instant::now()));
    let attempt = attempt(&state);
    assert_eq!(attempt.number.get(), 1);
    state.phase(attempt, StopPhase::Worker);
    assert_eq!(
        state.request(&request(1, StopAction::Start)).status,
        SessionStopStatus::Pending {
            attempt: attempt.number,
            phase: StopPhase::Worker
        }
    );
    assert_eq!(
        state.request(&request(2, StopAction::Start)).status,
        SessionStopStatus::Busy {
            active_id: attempt.id
        }
    );
    assert_eq!(
        state.request(&request(2, StopAction::Status)).status,
        SessionStopStatus::NotRecorded
    );
}

#[test]
fn retries_are_explicit_fenced_and_bounded_and_preserve_failure_progress() {
    let state = SessionShutdown::default();
    state.request(&request(1, StopAction::Start));
    let first = attempt(&state);
    for number in 1..=64 {
        let attempt = if number == 1 {
            first
        } else {
            self::attempt(&state)
        };
        assert_eq!(attempt.number.get(), number);
        state.fail(first, StopPhase::Dispose, StopFailure::OutcomeUnknown, 9);
        if number > 1 {
            assert!(matches!(
                state.request(&request(1, StopAction::Status)).status,
                SessionStopStatus::Pending { .. }
            ));
        }
        state.fail(attempt, StopPhase::Dispose, StopFailure::OutcomeUnknown, 9);
        let failed = SessionStopStatus::Failed {
            attempt: attempt.number,
            phase: StopPhase::Dispose,
            failure: StopFailure::OutcomeUnknown,
            disposed_terminals: 9,
        };
        assert_eq!(state.request(&request(1, StopAction::Start)).status, failed);
        assert!(state.enter().is_none());
        let retry = state.request(&request(1, StopAction::Retry)).status;
        if number < 64 {
            assert!(matches!(retry, SessionStopStatus::Pending { .. }));
        } else {
            assert_eq!(retry, failed);
        }
    }
}

#[test]
fn drain_is_repeatable_and_closes_only_new_work() {
    let state = SessionShutdown::default();
    assert_eq!(state.offer_state(), ProcessState::Ready);
    let in_flight = state.enter().unwrap();
    assert_eq!(state.drain(), ProcessState::Draining);
    assert_eq!(state.drain(), ProcessState::Draining);
    assert_eq!(state.process_state(), ProcessState::Draining);
    // An offer meets a state other than `ready`, so the handshake refuses it with `stopping`.
    assert_eq!(state.offer_state(), ProcessState::Draining);
    assert!(state.enter().is_none());
    // Reads continue, so a waiting Client still observes its admitted operation's result.
    drop(state.enter_read().unwrap());
    // The admitted record still runs to its result; a drain alone gives the owner no work.
    assert!(!state.settle_records(Instant::now()));
    assert_eq!(state.take_within(Duration::ZERO), None);
    drop(in_flight);
    assert!(state.settle_records(Instant::now()));
}

#[test]
fn exit_follows_a_queued_stop_and_never_returns_to_serving() {
    let state = SessionShutdown::default();
    state.request(&request(1, StopAction::Start));
    let reply = state.exit();
    assert!(matches!(
        state.take_within(Duration::ZERO),
        Some(Work::Stop(_))
    ));
    assert_eq!(state.take_within(Duration::ZERO), Some(Work::Exit));
    assert_eq!(state.drain(), ProcessState::Stopping);
    assert_eq!(state.offer_state(), ProcessState::Stopping);
    assert!(state.enter().is_none());
    assert!(state.enter_read().is_none());
    drop(reply);

    let state = SessionShutdown::default();
    let _reply = state.exit();
    assert_eq!(state.process_state(), ProcessState::Stopping);
    assert_eq!(state.drain(), ProcessState::Stopping);
    assert_eq!(state.take_within(Duration::ZERO), Some(Work::Exit));
}

#[test]
fn the_process_ends_only_after_each_exit_requester_replies() {
    let state = SessionShutdown::default();
    assert!(state.end(Instant::now()));

    let state = std::sync::Arc::new(SessionShutdown::default());
    let reply_written = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let requester = {
        let state = std::sync::Arc::clone(&state);
        let reply_written = std::sync::Arc::clone(&reply_written);
        std::thread::spawn(move || {
            let reply = state.exit();
            assert!(reply.ended_before(Instant::now() + Duration::from_secs(5)));
            reply_written.store(true, std::sync::atomic::Ordering::Release);
        })
    };
    while state.take_within(Duration::from_millis(10)) != Some(Work::Exit) {}
    assert!(state.end(Instant::now() + Duration::from_secs(5)));
    assert!(reply_written.load(std::sync::atomic::Ordering::Acquire));
    requester.join().unwrap();

    // A requester that never replies delays the end only until the owner's deadline.
    let state = SessionShutdown::default();
    let _silent = state.exit();
    assert!(!state.end(Instant::now() + Duration::from_millis(20)));
}
