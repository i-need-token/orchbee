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
        TerminalViewerAction::SetProjectionDemand {
            demand: ViewerProjectionDemand::Active,
        },
        TerminalViewerAction::SetProjectionDemand {
            demand: ViewerProjectionDemand::Paused,
        },
        TerminalViewerAction::SetHistoryDemand {
            demand: ViewerHistoryDemand::FollowingBottom,
        },
        TerminalViewerAction::SetHistoryDemand {
            demand: ViewerHistoryDemand::ViewingHistory,
        },
        TerminalViewerAction::ReadViewportRows {
            origin: ViewportRowOrigin::Tail { rows_before: 30 },
            row_count: 24,
            revision: 7,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: 8 * 1024 * 1024,
        },
        TerminalViewerAction::ReadViewportRows {
            origin: ViewportRowOrigin::Committed { delta: -12 },
            row_count: 24,
            revision: 7,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: 112,
        },
        TerminalViewerAction::RowsApplied {
            request_sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 0,
        },
        TerminalViewerAction::HistoryObserved {
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

/// Tags 7 through 11 are additive history-viewport actions: their exact byte
/// layouts are fixed, `i32` is the only signed field, and the action tag sits
/// at the same offset 65 every earlier tag uses.
#[test]
fn history_viewport_action_tags_seven_through_eleven_layout_is_fixed() -> TestResult {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        worker_id: WorkerId::from_bytes([6; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Detach,
    };
    fn encode(
        request: TerminalViewerRequest,
        action: TerminalViewerAction,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Ok(encode_message(
            &CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
                action,
                ..request
            }),
        )?)
    }

    for (action, expected) in [
        (
            TerminalViewerAction::SetProjectionDemand {
                demand: ViewerProjectionDemand::Active,
            },
            [7, 1],
        ),
        (
            TerminalViewerAction::SetProjectionDemand {
                demand: ViewerProjectionDemand::Paused,
            },
            [7, 2],
        ),
        (
            TerminalViewerAction::SetHistoryDemand {
                demand: ViewerHistoryDemand::FollowingBottom,
            },
            [8, 1],
        ),
        (
            TerminalViewerAction::SetHistoryDemand {
                demand: ViewerHistoryDemand::ViewingHistory,
            },
            [8, 2],
        ),
    ] {
        assert_eq!(&encode(request, action)?[65..], expected.as_slice());
    }

    let mut expected = vec![9_u8, 1];
    expected.extend_from_slice(&0x0102_0304_u32.to_be_bytes());
    expected.extend_from_slice(&4096_u16.to_be_bytes());
    expected.extend_from_slice(&0x1112_1314_1516_1718_u64.to_be_bytes());
    expected.extend_from_slice(&0x2122_2324_2526_2728_u64.to_be_bytes());
    expected.extend_from_slice(&0x3132_3334_3536_3738_u64.to_be_bytes());
    expected.extend_from_slice(&(8 * 1024 * 1024_u32).to_be_bytes());
    assert_eq!(
        &encode(
            request,
            TerminalViewerAction::ReadViewportRows {
                origin: ViewportRowOrigin::Tail {
                    rows_before: 0x0102_0304,
                },
                row_count: 4096,
                revision: 0x1112_1314_1516_1718,
                generation: NonZeroU64::new(0x2122_2324_2526_2728).expect("nonzero"),
                request_sequence: NonZeroU64::new(0x3132_3334_3536_3738).expect("nonzero"),
                maximum_bytes: 8 * 1024 * 1024,
            },
        )?[65..],
        expected.as_slice()
    );

    let mut expected = vec![9_u8, 2];
    expected.extend_from_slice(&(-12_i32).to_be_bytes());
    expected.extend_from_slice(&24_u16.to_be_bytes());
    expected.extend_from_slice(&0_u64.to_be_bytes());
    expected.extend_from_slice(&1_u64.to_be_bytes());
    expected.extend_from_slice(&1_u64.to_be_bytes());
    expected.extend_from_slice(&112_u32.to_be_bytes());
    assert_eq!(
        &encode(
            request,
            TerminalViewerAction::ReadViewportRows {
                origin: ViewportRowOrigin::Committed { delta: -12 },
                row_count: 24,
                revision: 0,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 112,
            },
        )?[65..],
        expected.as_slice()
    );
    // The committed delta is a signed big-endian i32 and the tail distance an
    // unsigned u32; both round-trip across their full ranges.
    for origin in [
        ViewportRowOrigin::Tail { rows_before: 0 },
        ViewportRowOrigin::Tail {
            rows_before: u32::MAX,
        },
        ViewportRowOrigin::Committed { delta: i32::MIN },
        ViewportRowOrigin::Committed { delta: -1 },
        ViewportRowOrigin::Committed { delta: 0 },
        ViewportRowOrigin::Committed { delta: 1 },
        ViewportRowOrigin::Committed { delta: i32::MAX },
    ] {
        let message = CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
            action: TerminalViewerAction::ReadViewportRows {
                origin,
                row_count: 24,
                revision: 0,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 112,
            },
            ..request
        });
        assert_eq!(decode_message(&encode_message(&message)?)?, message);
    }

    let mut expected = vec![10_u8];
    expected.extend_from_slice(&0x3132_3334_3536_3738_u64.to_be_bytes());
    expected.extend_from_slice(&0x2122_2324_2526_2728_u64.to_be_bytes());
    expected.extend_from_slice(&0x1112_1314_1516_1718_u64.to_be_bytes());
    assert_eq!(
        &encode(
            request,
            TerminalViewerAction::RowsApplied {
                request_sequence: NonZeroU64::new(0x3132_3334_3536_3738).expect("nonzero"),
                generation: NonZeroU64::new(0x2122_2324_2526_2728).expect("nonzero"),
                revision: 0x1112_1314_1516_1718,
            },
        )?[65..],
        expected.as_slice()
    );

    let mut expected = vec![11_u8];
    expected.extend_from_slice(&0x4142_4344_4546_4748_u64.to_be_bytes());
    expected.extend_from_slice(&0x2122_2324_2526_2728_u64.to_be_bytes());
    expected.extend_from_slice(&0x1112_1314_1516_1718_u64.to_be_bytes());
    assert_eq!(
        &encode(
            request,
            TerminalViewerAction::HistoryObserved {
                sequence: NonZeroU64::new(0x4142_4344_4546_4748).expect("nonzero"),
                generation: NonZeroU64::new(0x2122_2324_2526_2728).expect("nonzero"),
                revision: 0x1112_1314_1516_1718,
            },
        )?[65..],
        expected.as_slice()
    );
    Ok(())
}

/// Every semantic bound the new layouts cannot express fails closed at
/// decode: unknown tags, unknown demand or origin values, zero positive
/// integers, out-of-range bounds, and trailing bytes all reject.
#[test]
fn history_viewport_actions_reject_out_of_bounds_and_unknown_values() -> TestResult {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        worker_id: WorkerId::from_bytes([6; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Detach,
    };
    let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(
        TerminalViewerRequest {
            action: TerminalViewerAction::ReadViewportRows {
                origin: ViewportRowOrigin::Committed { delta: -12 },
                row_count: 24,
                revision: 0,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 112,
            },
            ..request
        },
    ))?;
    // Field offsets inside the tag-9 action body: tag 65, origin 66, delta
    // 67..71, row_count 71..73, revision 73..81, generation 81..89,
    // request_sequence 89..97, maximum_bytes 97..101.
    for (offset, bytes) in [
        (66, 0_u8.to_be_bytes().to_vec()),
        (66, 3_u8.to_be_bytes().to_vec()),
        (71, 0_u16.to_be_bytes().to_vec()),
        (71, 4097_u16.to_be_bytes().to_vec()),
        (81, 0_u64.to_be_bytes().to_vec()),
        (89, 0_u64.to_be_bytes().to_vec()),
        (97, 111_u32.to_be_bytes().to_vec()),
        (97, (8 * 1024 * 1024_u32 + 1).to_be_bytes().to_vec()),
    ] {
        let mut mutated = encoded.clone();
        mutated[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(decode_message(&mutated).is_err());
    }
    let mut truncated = encoded.clone();
    truncated.pop();
    assert!(decode_message(&truncated).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());

    // Demand byte sits at offset 66 for tags 7 and 8.
    for action in [
        TerminalViewerAction::SetProjectionDemand {
            demand: ViewerProjectionDemand::Active,
        },
        TerminalViewerAction::SetHistoryDemand {
            demand: ViewerHistoryDemand::FollowingBottom,
        },
    ] {
        let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(
            TerminalViewerRequest { action, ..request },
        ))?;
        for demand in [0_u8, 3, u8::MAX] {
            let mut mutated = encoded.clone();
            mutated[66] = demand;
            assert!(decode_message(&mutated).is_err());
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_message(&trailing).is_err());
    }

    // Tags 10 and 11 reject zero in each positive u64 field: tag 65, first
    // positive u64 66..74, generation 74..82, revision 82..90.
    for action in [
        TerminalViewerAction::RowsApplied {
            request_sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 0,
        },
        TerminalViewerAction::HistoryObserved {
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 0,
        },
    ] {
        let encoded = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(
            TerminalViewerRequest { action, ..request },
        ))?;
        for offset in [66, 74] {
            let mut mutated = encoded.clone();
            mutated[offset..offset + 8].fill(0);
            assert!(decode_message(&mutated).is_err());
        }
    }

    let mut unknown_action =
        encode_message(&CapabilityNodeMessage::TerminalViewerRequest(request))?;
    unknown_action[65] = 12;
    assert!(decode_message(&unknown_action).is_err());
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
