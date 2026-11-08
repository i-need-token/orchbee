use super::*;
use crate::{CapabilityNodeMessage, decode_message, encode_message};
use std::num::NonZeroU64;

fn request() -> WorkerShutdownRequest {
    WorkerShutdownRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        budget: ShutdownBudget::new(15_000).unwrap(),
        terminals: ShutdownTerminals::Dispose,
    }
}

fn round_trip(message: CapabilityNodeMessage) {
    let bytes = encode_message(&message).unwrap();
    assert_eq!(decode_message(&bytes).unwrap(), message);
    for end in 0..bytes.len() {
        assert!(decode_message(&bytes[..end]).is_err());
    }
    let mut trailing = bytes;
    trailing.push(0);
    let size = (trailing.len() - 8) as u16;
    trailing[6..8].copy_from_slice(&size.to_be_bytes());
    assert!(decode_message(&trailing).is_err());
}

#[test]
fn closed_shutdown_wire_preserves_identity_progress_and_failure() {
    round_trip(CapabilityNodeMessage::WorkerShutdownRequest(request()));
    round_trip(CapabilityNodeMessage::WorkerShutdownRequest(
        WorkerShutdownRequest {
            terminals: ShutdownTerminals::Keep,
            ..request()
        },
    ));
    let epoch = HostEpoch::from_bytes([4; 16]).unwrap();
    let result = |outcome| {
        CapabilityNodeMessage::WorkerShutdownResult(WorkerShutdownResult {
            request: request(),
            outcome,
        })
    };
    for outcome in [
        WorkerShutdownOutcome::NoHost,
        WorkerShutdownOutcome::Stopped {
            host_epoch: epoch,
            disposed_terminals: u32::MAX,
        },
        WorkerShutdownOutcome::Refused(ShutdownRefusal::WrongWorker),
        WorkerShutdownOutcome::Refused(ShutdownRefusal::Unmanaged),
        WorkerShutdownOutcome::Released { host_epoch: epoch },
    ] {
        round_trip(result(outcome));
    }
    for stage in [
        ShutdownStage::BackgroundJobs,
        ShutdownStage::Inventory,
        ShutdownStage::Force,
        ShutdownStage::Dispose,
        ShutdownStage::HostExit,
    ] {
        for failure in [
            ShutdownFailure::Deadline,
            ShutdownFailure::Transport,
            ShutdownFailure::Protocol,
            ShutdownFailure::HostRefused,
            ShutdownFailure::ProcessWait,
        ] {
            for host_epoch in [None, Some(epoch)] {
                round_trip(result(WorkerShutdownOutcome::Incomplete {
                    host_epoch,
                    disposed_terminals: 9,
                    stage,
                    failure,
                }));
            }
        }
    }
    let bytes = encode_message(&CapabilityNodeMessage::WorkerShutdownRequest(request())).unwrap();
    for range in [9..25, 25..41, 41..49, 49..53] {
        let mut zero = bytes.clone();
        zero[range].fill(0);
        assert!(decode_message(&zero).is_err());
    }
    for terminals in [0, 3] {
        let mut invalid = bytes.clone();
        invalid[53] = terminals;
        assert!(decode_message(&invalid).is_err());
    }
    let mut over = bytes;
    over[49..53].copy_from_slice(&15_001_u32.to_be_bytes());
    assert!(decode_message(&over).is_err());
    assert!(ShutdownBudget::new(0).is_err());
    assert!(ShutdownBudget::new(15_001).is_err());
    assert_eq!(ShutdownBudget::new(1).unwrap().milliseconds(), 1);
    let bytes = encode_message(&result(WorkerShutdownOutcome::Incomplete {
        host_epoch: None,
        disposed_terminals: 0,
        stage: ShutdownStage::Inventory,
        failure: ShutdownFailure::Deadline,
    }))
    .unwrap();
    for index in [54, 55, 60, 61] {
        let mut invalid = bytes.clone();
        invalid[index] = 255;
        assert!(decode_message(&invalid).is_err());
    }
}
