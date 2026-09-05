use orchbee_capability_node_protocol::*;
use std::{io::Cursor, num::NonZeroU64};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn viewer_controls_round_trip_every_closed_action_and_outcome() -> TestResult {
    let id = OperationId::from_bytes([1; 16])?;
    for action in [
        TerminalViewerAction::Attach {
            host_epoch: HostEpoch::from_bytes([3; 16])?,
            terminal_id: HostTerminalId::from_bytes([4; 16])?,
        },
        TerminalViewerAction::Poll,
        TerminalViewerAction::FrameApplied {
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 0,
        },
        TerminalViewerAction::Detach,
    ]
    .into_iter()
    .chain(
        [
            ViewerSnapshotReason::ActivityStale,
            ViewerSnapshotReason::BaseMismatch,
            ViewerSnapshotReason::DecoderRejected,
            ViewerSnapshotReason::DimensionsMismatch,
            ViewerSnapshotReason::GenerationMismatch,
            ViewerSnapshotReason::IdentityMismatch,
            ViewerSnapshotReason::SequenceGap,
            ViewerSnapshotReason::StoreReset,
        ]
        .map(|reason| TerminalViewerAction::NeedSnapshot {
            sequence: NonZeroU64::MIN,
            reason,
        }),
    ) {
        let message = CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
            request_id: id,
            attachment_id: OperationId::from_bytes([5; 16])?,
            worker_id: WorkerId::from_bytes([6; 16])?,
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action,
        });
        let encoded = encode_message(&message)?;
        assert!(encoded.starts_with(b"OBCN"));
        assert_eq!(decode_message(&encoded)?, message);
        assert_eq!(read_message(&mut Cursor::new(&encoded))?, message);
        let mut zero_identity = encoded.clone();
        zero_identity[9..25].fill(0);
        assert!(decode_message(&zero_identity).is_err());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_message(&trailing).is_err());
    }
    for outcome in [
        TerminalViewerOutcome::Attached,
        TerminalViewerOutcome::Empty,
        TerminalViewerOutcome::FeedbackAccepted,
        TerminalViewerOutcome::Detached,
    ]
    .into_iter()
    .chain(
        [
            ViewerRejection::WrongWorker,
            ViewerRejection::StaleWorkerGeneration,
            ViewerRejection::StaleHostEpoch,
            ViewerRejection::CapacityExceeded,
            ViewerRejection::AttachmentUnavailable,
            ViewerRejection::AttachmentAlreadyExists,
            ViewerRejection::FeedbackMismatch,
            ViewerRejection::HostRejected,
        ]
        .map(TerminalViewerOutcome::Rejected),
    ) {
        let message = CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
            request_id: id,
            attachment_id: id,
            outcome,
        });
        assert_eq!(decode_message(&encode_message(&message)?)?, message);
    }
    Ok(())
}

#[test]
fn binary_record_survives_streaming_beyond_control_limit_and_keeps_next_frame_aligned() -> TestResult
{
    let id = OperationId::from_bytes([1; 16])?;
    let record = (0..100_000).map(|index| (index % 251) as u8).collect();
    let message = CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
        request_id: id,
        attachment_id: id,
        outcome: TerminalViewerOutcome::Record(record),
    });
    let encoded = encode_message(&message)?;
    assert!(encoded.starts_with(b"OBNR\0\x01\0\0"));
    assert_eq!(decode_message(&encoded)?, message);
    let next = CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
        request_id: id,
        attachment_id: id,
        outcome: TerminalViewerOutcome::Empty,
    });
    let mut bytes = encoded;
    bytes.extend_from_slice(&encode_message(&next)?);
    let mut stream = Cursor::new(bytes);
    assert_eq!(read_message(&mut stream)?, message);
    assert_eq!(read_message(&mut stream)?, next);
    Ok(())
}

#[test]
fn binary_header_bounds_and_identity_fail_closed_before_payload_use() -> TestResult {
    let id = OperationId::from_bytes([1; 16])?;
    let message = CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
        request_id: id,
        attachment_id: id,
        outcome: TerminalViewerOutcome::Record(vec![0; 64]),
    });
    let encoded = encode_message(&message)?;
    for length in [0, 32, 95, u32::MAX] {
        let mut header = encoded[..12].to_vec();
        header[8..12].copy_from_slice(&length.to_be_bytes());
        assert!(matches!(
            read_message(&mut Cursor::new(header)),
            Err(ReadMessageError::Protocol(_))
        ));
    }
    for offset in [5, 6] {
        let mut bytes = encoded.clone();
        bytes[offset] = 7;
        assert!(decode_message(&bytes).is_err());
    }
    for range in [12..28, 28..44] {
        let mut bytes = encoded.clone();
        bytes[range].fill(0);
        assert!(decode_message(&bytes).is_err());
    }
    assert!(decode_message(&encoded[..encoded.len() - 1]).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());
    Ok(())
}
