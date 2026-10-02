use super::*;
use crate::{CapabilityNodeMessage as Message, decode_message, encode_message};
use std::num::NonZeroU64;

fn request(action: WorkerHandoffAction) -> WorkerHandoffRequest {
    WorkerHandoffRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        target: WorkerHandoffTarget {
            handoff_id: WorkerHandoffId::from_bytes([4; 16]).unwrap(),
            cohort_sha256: [5; 32],
        },
        action,
    }
}

fn round_trip(value: Message) {
    let bytes = encode_message(&value).unwrap();
    assert_eq!(decode_message(&bytes).unwrap(), value);
    for end in 0..bytes.len() {
        assert!(decode_message(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut suffix = bytes.clone();
    suffix.push(0);
    assert!(decode_message(&suffix).is_err());
    let payload_length = u16::try_from(suffix.len() - 8).unwrap();
    suffix[6..8].copy_from_slice(&payload_length.to_be_bytes());
    assert!(decode_message(&suffix).is_err(), "closed payload suffix");
}

#[test]
fn handoff_golden_bytes_are_fixed_sized_and_direction_tagged() {
    let mut expected = vec![b'O', b'B', b'C', b'N', 0, 2, 0, 90, 26];
    expected.extend([1; 16]);
    expected.extend([2; 16]);
    expected.extend([0, 0, 0, 0, 0, 0, 0, 3]);
    expected.extend([4; 16]);
    expected.extend([5; 32]);
    expected.push(1);
    assert_eq!(
        encode_message(&Message::WorkerHandoffRequest(request(
            WorkerHandoffAction::Prepare
        )))
        .unwrap(),
        expected
    );
    for (tag, message) in [
        (
            28,
            Message::WorkerHandoffHello(request(WorkerHandoffAction::Prepare).peer()),
        ),
        (
            29,
            Message::WorkerHandoffReady(request(WorkerHandoffAction::Prepare).peer()),
        ),
        (
            30,
            Message::WorkerHandoffOwned(request(WorkerHandoffAction::Prepare).peer()),
        ),
    ] {
        let mut peer = vec![b'O', b'B', b'C', b'N', 0, 2, 0, 73, tag];
        peer.extend(&expected[25..97]);
        assert_eq!(encode_message(&message).unwrap(), peer);
        round_trip(message);
    }
}

#[test]
fn handoff_actions_outcomes_and_refusals_round_trip_with_closed_payloads() {
    use WorkerHandoffAction as Action;
    use WorkerHandoffOutcome as Outcome;
    use WorkerHandoffRefusal as Refusal;
    for (action, outcome) in [
        (Action::Prepare, Outcome::Prepared),
        (Action::Commit, Outcome::Committed),
        (Action::Abort, Outcome::Aborted),
    ] {
        round_trip(Message::WorkerHandoffRequest(request(action)));
        let value = WorkerHandoffResult::new(request(action), outcome).unwrap();
        assert_eq!(
            encode_message(&Message::WorkerHandoffResult(value))
                .unwrap()
                .len(),
            99
        );
        round_trip(Message::WorkerHandoffResult(value));
        for refusal in [
            Refusal::WrongWorker,
            Refusal::Unmanaged,
            Refusal::Busy,
            Refusal::TicketMismatch,
            Refusal::NotPrepared,
            Refusal::ReusedTicket,
            Refusal::Capacity,
            Refusal::TargetUnavailable,
            Refusal::Authentication,
            Refusal::AttachmentsBusy,
        ] {
            let value =
                WorkerHandoffResult::new(request(action), Outcome::Refused(refusal)).unwrap();
            assert_eq!(
                encode_message(&Message::WorkerHandoffResult(value))
                    .unwrap()
                    .len(),
                100
            );
            round_trip(Message::WorkerHandoffResult(value));
        }
    }
}

#[test]
fn handoff_zero_identity_generation_unknown_tags_and_contradictions_are_rejected() {
    let value = request(WorkerHandoffAction::Prepare);
    let bytes = encode_message(&Message::WorkerHandoffRequest(value)).unwrap();
    for range in [9..25, 25..41, 41..49, 49..65] {
        let mut invalid = bytes.clone();
        invalid[range].fill(0);
        assert!(decode_message(&invalid).is_err());
    }
    for tag in [0, 4, 255] {
        let mut invalid = bytes.clone();
        invalid[97] = tag;
        assert!(decode_message(&invalid).is_err());
    }
    for action in [
        WorkerHandoffAction::Prepare,
        WorkerHandoffAction::Commit,
        WorkerHandoffAction::Abort,
    ] {
        for outcome in [
            WorkerHandoffOutcome::Prepared,
            WorkerHandoffOutcome::Committed,
            WorkerHandoffOutcome::Aborted,
        ] {
            let valid = matches!(
                (action, outcome),
                (WorkerHandoffAction::Prepare, WorkerHandoffOutcome::Prepared)
                    | (WorkerHandoffAction::Commit, WorkerHandoffOutcome::Committed)
                    | (WorkerHandoffAction::Abort, WorkerHandoffOutcome::Aborted)
            );
            assert_eq!(
                WorkerHandoffResult::new(request(action), outcome).is_ok(),
                valid
            );
            if !valid {
                let mut invalid = bytes.clone();
                invalid[6..8].copy_from_slice(&91_u16.to_be_bytes());
                invalid[8] = 27;
                invalid[97] = match action {
                    WorkerHandoffAction::Prepare => 1,
                    WorkerHandoffAction::Commit => 2,
                    WorkerHandoffAction::Abort => 3,
                };
                invalid.push(match outcome {
                    WorkerHandoffOutcome::Prepared => 1,
                    WorkerHandoffOutcome::Committed => 2,
                    WorkerHandoffOutcome::Aborted => 3,
                    WorkerHandoffOutcome::Refused(_) => unreachable!(),
                });
                assert!(decode_message(&invalid).is_err());
            }
        }
    }
    let mut invalid = encode_message(&Message::WorkerHandoffResult(
        WorkerHandoffResult::new(
            value,
            WorkerHandoffOutcome::Refused(WorkerHandoffRefusal::Busy),
        )
        .unwrap(),
    ))
    .unwrap();
    for reason in [0, 11, 255] {
        invalid[99] = reason;
        assert!(decode_message(&invalid).is_err());
    }
}
