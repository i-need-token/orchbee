use super::*;
use orchbee_local_client_protocol::ClientHello;

fn request(id: u8, action: StopAction) -> SessionStopRequest {
    SessionStopRequest {
        hello: ClientHello::new("cohort").unwrap(),
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
    assert!(!state.settle_records(Instant::now()));
    drop(record);
    assert!(state.settle_records(Instant::now()));
    let attempt = state.take();
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
    let first = state.take();
    for number in 1..=64 {
        let attempt = if number == 1 { first } else { state.take() };
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
