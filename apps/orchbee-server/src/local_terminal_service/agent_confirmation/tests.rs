use super::*;
use orchbee_capability_node_protocol::{
    OperationId, SendCleanupStatus, SendProgress, SendUnavailable, TerminalSendResult,
};

fn answered(outcome: TerminalSendOutcome) -> Result<WorkerOperationResult, WorkerRequestError> {
    Ok(WorkerOperationResult::Send(
        TerminalSendResult::new(OperationId::from_bytes([7; 16]).unwrap(), outcome).unwrap(),
    ))
}

/// A key is decided again only when its answer proves it never reached the Host. Any answer that
/// leaves that open counts as written, so one prompt is never pressed twice.
#[test]
fn only_a_key_that_provably_never_left_is_decided_again() {
    let none = SendProgress::none();
    let one = SendProgress::new(1, 1, 1).unwrap();
    let rejected = |reason| {
        answered(TerminalSendOutcome::Rejected {
            reason,
            progress: none,
            cleanup: SendCleanupStatus::NotNeeded,
        })
    };
    assert_eq!(
        outcome(answered(TerminalSendOutcome::Accepted {
            progress: one,
            cleanup: SendCleanupStatus::Confirmed,
        })),
        Outcome::Written
    );
    assert_eq!(
        outcome(rejected(SendRejection::ScreenRefused(
            SendScreenState::PersonInput
        ))),
        Outcome::Refused(Refusal::PersonInput)
    );
    assert_eq!(
        outcome(rejected(SendRejection::HostScreenGateUnsupported)),
        Outcome::Unsupported
    );
    assert_eq!(
        outcome(Err(WorkerRequestError::Declined)),
        Outcome::Unsupported
    );
    for proven in [
        rejected(SendRejection::TerminalUnavailable),
        rejected(SendRejection::HostInputBackpressure),
        answered(TerminalSendOutcome::Unavailable {
            reason: SendUnavailable::HostUnavailable,
            progress: none,
            cleanup: SendCleanupStatus::NotNeeded,
        }),
        Err(WorkerRequestError::Backpressure),
        Err(WorkerRequestError::WrongGeneration),
    ] {
        assert_eq!(outcome(proven), Outcome::NotSent);
    }
    for open in [
        TerminalSendOutcome::OutcomeUnknown {
            progress: none,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
        TerminalSendOutcome::Unavailable {
            reason: SendUnavailable::HostProtocolFault,
            progress: one,
            cleanup: SendCleanupStatus::Unconfirmed,
        },
    ] {
        assert_eq!(outcome(answered(open)), Outcome::Unknown);
    }
}
