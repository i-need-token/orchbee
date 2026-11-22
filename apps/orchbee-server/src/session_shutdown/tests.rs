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
fn a_stop_still_failing_after_the_grace_becomes_an_exit_unless_a_retry_is_pending() {
    let state = SessionShutdown::default();
    assert!(!state.abandon_failed_stop(Duration::ZERO));
    state.request(&request(1, StopAction::Start));
    let first = attempt(&state);
    assert!(!state.abandon_failed_stop(Duration::ZERO));
    state.fail(
        first,
        StopPhase::BackgroundJobs,
        StopFailure::ProcessWait,
        0,
    );
    assert!(!state.abandon_failed_stop(Duration::from_secs(3600)));
    // A pending retry holds the stop; its failure keeps the first failure's time.
    state.request(&request(1, StopAction::Retry));
    assert!(!state.abandon_failed_stop(Duration::ZERO));
    let second = attempt(&state);
    assert!(!state.abandon_failed_stop(Duration::ZERO));
    state.fail(
        second,
        StopPhase::BackgroundJobs,
        StopFailure::ProcessWait,
        0,
    );
    assert!(state.abandon_failed_stop(Duration::ZERO));
    assert!(!state.abandon_failed_stop(Duration::ZERO));
    assert_eq!(state.take_within(Duration::ZERO), Some(Work::Exit));
    assert!(matches!(
        state.request(&request(1, StopAction::Status)).status,
        SessionStopStatus::Failed { .. }
    ));
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

#[test]
fn a_signal_begins_the_own_stop_or_retries_the_failed_one_whoever_recorded_it() {
    let own = StopId::new([7; 16]).unwrap();
    let state = SessionShutdown::default();
    assert!(!state.signalled());
    state.signal(own);
    assert!(state.signalled());
    assert_eq!(attempt(&state).id, own);

    // A Client's failed stop is retried under the Client's identity; a running one is left alone.
    let state = SessionShutdown::default();
    state.request(&request(1, StopAction::Start));
    let first = attempt(&state);
    state.signal(own);
    assert_eq!(state.take_within(Duration::ZERO), None);
    state.fail(first, StopPhase::HostExit, StopFailure::Refused, 0);
    state.signal(own);
    let retried = attempt(&state);
    assert_eq!(retried.id, first.id);
    assert_eq!(retried.number.get(), 2);
}

#[test]
fn an_idle_stop_begins_only_with_nothing_admitted() {
    let id = StopId::new([9; 16]).unwrap();
    // An admitted record or a drain each refuse the idle stop.
    let state = SessionShutdown::default();
    let record = state.enter().unwrap();
    assert!(!state.begin_idle(id));
    drop(record);
    let drained = SessionShutdown::default();
    drained.drain();
    assert!(!drained.begin_idle(id));
    // Once begun, the stop is the owner's next work and every later Client is refused.
    assert!(state.begin_idle(id));
    assert!(state.enter().is_none());
    assert_eq!(attempt(&state).id, id);
    // An idle stop never replaces a stop already recorded.
    assert!(!state.begin_idle(StopId::new([8; 16]).unwrap()));
}

#[test]
fn a_restart_hold_refuses_stops_intents_and_clients_that_do_not_reconnect() {
    let state = SessionShutdown::default();
    // A Client that did not offer `reconnect`, connected or still being admitted, refuses the hold,
    // which then holds nothing.
    let plain = state.enter_client(false).unwrap();
    assert_eq!(state.enter_hold(|| true), Err(HoldRefusal::Clients));
    drop(plain);
    assert_eq!(state.enter_hold(|| false), Err(HoldRefusal::Clients));
    assert!(state.enter_intent().is_some());
    let read = state.enter_read().unwrap();
    let intent = state.enter_intent().unwrap();
    let rejoining = state.enter_client(true).unwrap();
    assert_eq!(state.enter_hold(|| true), Ok(()));
    // Reads, terminal records and Clients that offered `reconnect` are still admitted; other
    // intents wait for the end of the hold, and so does a stop, which records nothing rather than
    // end with this Server. Any other Client tries again later.
    assert!(state.enter_intent().is_none());
    assert!(state.enter_client(false).is_none());
    drop(rejoining);
    assert!(state.enter_client(true).is_some());
    let terminal = state.enter().unwrap();
    assert!(state.enter_read().is_some());
    assert!(!state.begin_idle(StopId::new([9; 16]).unwrap()));
    let start = request(10, StopAction::Start);
    assert_eq!(state.request(&start).status, SessionStopStatus::NotRecorded);
    assert!(!state.stopping());
    assert_eq!(state.process_state(), ProcessState::Ready);
    drop(intent);
    assert!(!state.settle_intents(Instant::now()));
    drop(terminal);
    // A read in flight does not keep the capture waiting.
    assert!(state.settle_intents(Instant::now()));
    drop(read);
    state.lift_hold();
    assert!(state.enter_intent().is_some());
    assert!(state.enter_client(false).is_some());
    // A recorded stop refuses the next hold.
    assert!(matches!(
        state.request(&start).status,
        SessionStopStatus::Pending { .. }
    ));
    assert_eq!(state.enter_hold(|| true), Err(HoldRefusal::Busy));
}

#[test]
fn one_handover_step_waits_at_a_time_and_an_untaken_one_is_withdrawn() {
    let state = std::sync::Arc::new(SessionShutdown::default());
    let ticket = HandoverTicket::from_bytes([7; 16]);
    // Nobody takes the step before its deadline, so it is withdrawn and the slot is free again.
    assert_eq!(
        state.handover(HandoverStep::Prepare, ticket, Instant::now()),
        None
    );
    assert_eq!(state.take_handover(), None);
    let waiting = {
        let state = std::sync::Arc::clone(&state);
        std::thread::spawn(move || {
            state.handover(
                HandoverStep::Commit,
                ticket,
                Instant::now() + Duration::from_secs(30),
            )
        })
    };
    // The owner's wait ends early for the step, which it takes exactly once.
    let taken = loop {
        if state.take_within(Duration::from_secs(30)).is_none()
            && let Some(taken) = state.take_handover()
        {
            break taken;
        }
    };
    assert_eq!(taken, (HandoverStep::Commit, ticket));
    assert_eq!(state.take_handover(), None);
    assert_eq!(
        state.handover(HandoverStep::Abort, ticket, Instant::now()),
        Some(HandoverOutcome::Refused(HandoverRefusal::Busy))
    );
    state.answer_handover(HandoverOutcome::Committed);
    assert_eq!(waiting.join().unwrap(), Some(HandoverOutcome::Committed));
}
