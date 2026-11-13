//! How a run reads each end of its exchanges, which a real hub reaches only by chance: a far reset
//! before and after submission, each answer of the Session at `start` and while watching, and a
//! run that its carrier or a closing hub ends. One process case in
//! `tests/cli_windows_machine_hub.rs` stops a real far Session through the stand-in.

use super::{Next, Seen, Stage, after_start, after_status, allowance_passed, carrier_lost, seen};
use orchbee_local_client_protocol::{
    ConnectClientError, RejectionReason, SessionStopExchangeError, SessionStopResult,
    SessionStopStatus, StopAttempt, StopFailure, StopId, StopPhase,
};
use orchbee_machine_protocol::{ResetReason, hub::StopSessionOutcome};
use std::io;

fn attempt() -> StopAttempt {
    StopAttempt::new(1).unwrap()
}

fn pending() -> SessionStopStatus {
    SessionStopStatus::Pending {
        attempt: attempt(),
        phase: StopPhase::Clients,
    }
}

fn id() -> StopId {
    StopId::new([7; 16]).unwrap()
}

fn lost(submitted: bool) -> SessionStopExchangeError {
    SessionStopExchangeError {
        failure: ConnectClientError::Io(io::ErrorKind::UnexpectedEof.into()),
        submitted,
    }
}

/// Every answer a Session gives, other than `pending`.
fn other_answers() -> [SessionStopStatus; 4] {
    [
        SessionStopStatus::NotRecorded,
        SessionStopStatus::Failed {
            attempt: attempt(),
            phase: StopPhase::Dispose,
            failure: StopFailure::Deadline,
            disposed_terminals: 0,
        },
        SessionStopStatus::Busy { active_id: id() },
        SessionStopStatus::Unmanaged,
    ]
}

#[test]
fn a_far_reset_states_that_nothing_ran_only_before_submission() {
    let reset = Some(ResetReason::SessionNotRunning);
    assert_eq!(
        seen(Err(lost(false)), reset, false),
        Seen::Reset(ResetReason::SessionNotRunning)
    );
    // Once a byte of the request was committed, the reset proves nothing about the Session.
    assert_eq!(
        seen(Err(lost(false)), reset, true),
        Seen::Broken { submitted: true }
    );
    assert_eq!(
        seen(Err(lost(true)), None, true),
        Seen::Broken { submitted: true }
    );
    // A refusal or rejection is the Session's own statement that it did not act.
    let rejected = SessionStopExchangeError {
        failure: ConnectClientError::Rejected(RejectionReason::Stopping),
        submitted: true,
    };
    assert_eq!(seen(Err(rejected), None, true), Seen::Refused);
    let answered = SessionStopResult {
        id: id(),
        status: pending(),
    };
    assert_eq!(seen(Ok(answered), reset, true), Seen::Answered(pending()));
}

#[test]
fn a_start_fails_without_effect_until_the_session_may_have_it() {
    assert_eq!(
        after_start(Seen::Reset(ResetReason::SessionNotRunning)),
        Next::Done(StopSessionOutcome::NotRunning)
    );
    assert_eq!(
        after_start(Seen::Answered(pending())),
        Next::Watch(Stage::Recorded)
    );
    // A request that may have arrived is watched, never reported as not done.
    assert_eq!(
        after_start(Seen::Broken { submitted: true }),
        Next::Watch(Stage::Unanswered)
    );
    let failed = Next::Done(StopSessionOutcome::Failed);
    for status in other_answers() {
        assert_eq!(after_start(Seen::Answered(status)), failed);
    }
    for seen in [
        Seen::Refused,
        Seen::Reset(ResetReason::Unavailable),
        Seen::Reset(ResetReason::InvalidSession),
        Seen::Broken { submitted: false },
    ] {
        assert_eq!(after_start(seen), failed);
    }
}

#[test]
fn watching_ends_only_on_the_sessions_word_or_its_end() {
    for stage in [Stage::Recorded, Stage::Unanswered] {
        assert_eq!(
            after_status(stage, Seen::Reset(ResetReason::SessionNotRunning)),
            Next::Done(StopSessionOutcome::Stopped)
        );
        assert_eq!(
            after_status(stage, Seen::Answered(pending())),
            Next::Watch(Stage::Recorded)
        );
        for status in other_answers().into_iter().skip(1) {
            assert_eq!(
                after_status(stage, Seen::Answered(status)),
                Next::Done(StopSessionOutcome::Failed)
            );
        }
        // Nothing the far end or the transport says otherwise decides the stop.
        for seen in [
            Seen::Refused,
            Seen::Reset(ResetReason::Unavailable),
            Seen::Broken { submitted: true },
            Seen::Broken { submitted: false },
        ] {
            assert_eq!(after_status(stage, seen), Next::Watch(stage));
        }
    }
    // Only a Server that recorded the stop and is gone answers `not-recorded` for it.
    let not_recorded = Seen::Answered(SessionStopStatus::NotRecorded);
    assert_eq!(
        after_status(Stage::Recorded, not_recorded),
        Next::Done(StopSessionOutcome::Stopped)
    );
    assert_eq!(
        after_status(Stage::Unanswered, not_recorded),
        Next::Done(StopSessionOutcome::Unknown)
    );
}

#[test]
fn a_run_ended_from_outside_is_unknown_once_submitted() {
    assert_eq!(carrier_lost(true), StopSessionOutcome::Unknown);
    assert_eq!(carrier_lost(false), StopSessionOutcome::NotOnline);
    assert_eq!(allowance_passed(true), StopSessionOutcome::Unknown);
    assert_eq!(allowance_passed(false), StopSessionOutcome::Stopping);
}
