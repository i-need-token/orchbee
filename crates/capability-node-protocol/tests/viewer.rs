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
        TerminalViewerAction::ReadRows {
            start_row: 400,
            row_count: 100,
            revision: 7,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: 8 * 1024 * 1024,
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
    )
    .chain(
        [
            ViewerRowsRejectedReason::StaleRevision,
            ViewerRowsRejectedReason::StaleGeneration,
            ViewerRowsRejectedReason::HistoryEvicted,
            ViewerRowsRejectedReason::MissingResource,
            ViewerRowsRejectedReason::InvalidField,
            ViewerRowsRejectedReason::InvalidAcknowledgement,
            ViewerRowsRejectedReason::LimitExceeded,
            ViewerRowsRejectedReason::ResourceExhausted,
            ViewerRowsRejectedReason::InvalidPhase,
        ]
        .map(TerminalViewerOutcome::RowsRejected),
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

/// Tag 6 is additive: its exact byte layout is fixed, and tags 1 through 5
/// keep their pre-existing bytes.
#[test]
fn read_rows_action_tag_six_layout_is_fixed_and_older_tags_are_unchanged() -> TestResult {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        worker_id: WorkerId::from_bytes([6; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::ReadRows {
            start_row: 0x0102_0304_0506_0708,
            row_count: 4096,
            revision: 0x1112_1314_1516_1718,
            generation: NonZeroU64::new(0x2122_2324_2526_2728).expect("nonzero"),
            request_sequence: NonZeroU64::new(0x3132_3334_3536_3738).expect("nonzero"),
            maximum_bytes: 8 * 1024 * 1024,
        },
    };
    let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(request))?;
    // OBCN header (8) + kind (1) + request id (16) + worker id (16) +
    // generation (8) + attachment id (16) = action tag at offset 65.
    let action = &encoded[65..];
    let mut expected = vec![6_u8];
    expected.extend_from_slice(&0x0102_0304_0506_0708_u64.to_be_bytes());
    expected.extend_from_slice(&4096_u16.to_be_bytes());
    expected.extend_from_slice(&0x1112_1314_1516_1718_u64.to_be_bytes());
    expected.extend_from_slice(&0x2122_2324_2526_2728_u64.to_be_bytes());
    expected.extend_from_slice(&0x3132_3334_3536_3738_u64.to_be_bytes());
    expected.extend_from_slice(&(8 * 1024 * 1024_u32).to_be_bytes());
    assert_eq!(action, expected.as_slice());
    for (tag, action) in [
        (2_u8, TerminalViewerAction::Poll),
        (5_u8, TerminalViewerAction::Detach),
    ] {
        let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(
            TerminalViewerRequest { action, ..request },
        ))?;
        assert_eq!(encoded[65], tag);
    }
    Ok(())
}

/// Every semantic bound the tag-6 layout cannot express fails closed at
/// decode, alongside malformed lengths, trailing bytes, and unknown tags.
#[test]
fn read_rows_action_and_rows_rejected_reject_out_of_bounds_and_unknown_tags() -> TestResult {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        worker_id: WorkerId::from_bytes([6; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::ReadRows {
            start_row: 0,
            row_count: 1,
            revision: 0,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: 112,
        },
    };
    let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(request))?;
    // Field offsets inside the action body: tag at 65, start_row 66..74,
    // row_count 74..76, revision 76..84, generation 84..92, sequence 92..100,
    // maximum_bytes 100..104.
    for (offset, bytes) in [
        (74, 0_u16.to_be_bytes().to_vec()),
        (74, 4097_u16.to_be_bytes().to_vec()),
        (84, 0_u64.to_be_bytes().to_vec()),
        (92, 0_u64.to_be_bytes().to_vec()),
        (100, 111_u32.to_be_bytes().to_vec()),
        (100, (8 * 1024 * 1024_u32 + 1).to_be_bytes().to_vec()),
    ] {
        let mut mutated = encoded.clone();
        mutated[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(decode_message(&mutated).is_err());
    }
    let mut unknown_action = encoded.clone();
    unknown_action[65] = 7;
    assert!(decode_message(&unknown_action).is_err());
    let result = TerminalViewerResult {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        outcome: TerminalViewerOutcome::RowsRejected(ViewerRowsRejectedReason::StaleRevision),
    };
    let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerResult(result))?;
    // OBCN header (8) + kind (1) + request id (16) + attachment id (16) =
    // outcome tag at offset 41, reason tag at 42.
    assert_eq!(&encoded[41..], &[6, 1]);
    for reason in [0_u8, 10, u8::MAX] {
        let mut mutated = encoded.clone();
        mutated[42] = reason;
        assert!(decode_message(&mutated).is_err());
    }
    let mut unknown_outcome = encoded.clone();
    unknown_outcome[41] = 7;
    assert!(decode_message(&unknown_outcome).is_err());
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());
    assert!(decode_message(&encoded[..encoded.len() - 1]).is_err());
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
