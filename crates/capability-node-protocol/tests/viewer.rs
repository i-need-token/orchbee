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
        TerminalViewerAction::ReadProcessInfo,
        TerminalViewerAction::ReadLifecycle {
            target: HostTerminalIdentity::new(
                HostCreationId::from_bytes([2; 16])?,
                HostEpoch::from_bytes([3; 16])?,
                HostTerminalId::from_bytes([4; 16])?,
            ),
        },
        TerminalViewerAction::FrameApplied {
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 0,
        },
        TerminalViewerAction::ReadRows {
            detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
            detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
            origin: ViewportRowOrigin::Tail { rows_before: 30 },
            row_count: 24,
            revision: 7,
            generation: NonZeroU64::MIN,
            request_sequence: NonZeroU64::MIN,
            maximum_bytes: 8 * 1024 * 1024,
        },
        TerminalViewerAction::ReadViewportRows {
            detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
        TerminalViewerAction::SelectionGesture {
            generation: NonZeroU64::MIN,
            gesture: ViewerSelectionGestureAction::Start {
                click_count: 2,
                point: ViewerSelectionViewportPoint { column: 3, row: 4 },
                position: ViewerSelectionGesturePosition { x: -5, y: 6 },
                select_output: true,
            },
        },
        TerminalViewerAction::SelectionGesture {
            generation: NonZeroU64::MIN,
            gesture: ViewerSelectionGestureAction::Update {
                geometry: ViewerSelectionGestureGeometry {
                    cell_width: 8,
                    columns: 80,
                    padding_left: 2,
                    screen_height: 480,
                },
                point: ViewerSelectionViewportPoint { column: 7, row: 8 },
                position: ViewerSelectionGesturePosition { x: 9, y: -10 },
                rectangle: false,
            },
        },
        TerminalViewerAction::SelectionGesture {
            generation: NonZeroU64::MIN,
            gesture: ViewerSelectionGestureAction::End {
                point: ViewerSelectionViewportPoint {
                    column: 11,
                    row: 12,
                },
            },
        },
        TerminalViewerAction::SelectionGesture {
            generation: NonZeroU64::MIN,
            gesture: ViewerSelectionGestureAction::Cancel,
        },
        TerminalViewerAction::SelectionCommand {
            generation: NonZeroU64::MIN,
            revision: 0,
            command: ViewerSelectionCommand::Clear,
        },
        TerminalViewerAction::SelectionCommand {
            generation: NonZeroU64::MIN,
            revision: 7,
            command: ViewerSelectionCommand::SelectAll,
        },
        TerminalViewerAction::SelectionCommand {
            generation: NonZeroU64::MIN,
            revision: 7,
            command: ViewerSelectionCommand::SetRange {
                end: ViewerSelectionPoint {
                    column: 4095,
                    row: u64::MAX,
                },
                rectangle: true,
                start: ViewerSelectionPoint { column: 0, row: 0 },
            },
        },
        TerminalViewerAction::SelectionCommand {
            generation: NonZeroU64::MIN,
            revision: 7,
            command: ViewerSelectionCommand::SelectOutput {
                point: ViewerSelectionPoint { column: 3, row: 12 },
            },
        },
        TerminalViewerAction::SelectionExport {
            export_sequence: NonZeroU64::MIN,
            format: ViewerSelectionExportFormat::Plain,
        },
        TerminalViewerAction::SelectionExportChunkApplied {
            export_sequence: NonZeroU64::MIN,
            chunk_sequence: 0,
        },
        TerminalViewerAction::SelectionExportCancel {
            export_sequence: NonZeroU64::MIN,
            reason: ViewerSelectionExportCancelReason::User,
        },
        TerminalViewerAction::Detach,
    ]
    .into_iter()
    .chain(
        [
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
        TerminalViewerOutcome::LifecycleObserved {
            target: HostTerminalIdentity::new(
                HostCreationId::from_bytes([2; 16])?,
                HostEpoch::from_bytes([3; 16])?,
                HostTerminalId::from_bytes([4; 16])?,
            ),
            revision: 9,
            settled: false,
        },
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

#[test]
fn lifecycle_viewer_pins_all_host_identity_fields_and_rejects_non_boolean_settlement() -> TestResult
{
    let identity = HostTerminalIdentity::new(
        HostCreationId::from_bytes([2; 16])?,
        HostEpoch::from_bytes([3; 16])?,
        HostTerminalId::from_bytes([4; 16])?,
    );
    let message = CapabilityNodeMessage::TerminalViewerResult(TerminalViewerResult {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        outcome: TerminalViewerOutcome::LifecycleObserved {
            target: identity,
            revision: 42,
            settled: true,
        },
    });
    let encoded = encode_message(&message)?;
    assert_eq!(decode_message(&encoded)?, message);
    for end in 0..encoded.len() {
        assert!(decode_message(&encoded[..end]).is_err());
    }
    let mut invalid = encoded.clone();
    *invalid.last_mut().unwrap() = 2;
    assert!(decode_message(&invalid).is_err());
    invalid = encoded;
    invalid.push(0);
    assert!(decode_message(&invalid).is_err());
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
            detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
    let mut render = request;
    let TerminalViewerAction::ReadRows { detail, .. } = &mut render.action else {
        unreachable!()
    };
    *detail = ViewerRowDetail::Render;
    let message = CapabilityNodeMessage::TerminalViewerRequest(render);
    let render_bytes = encode_message(&message)?;
    let mut expected_render = encoded.clone();
    expected_render[65] = 13;
    assert_eq!(render_bytes, expected_render);
    assert_eq!(decode_message(&render_bytes)?, message);
    for end in 0..render_bytes.len() {
        assert!(decode_message(&render_bytes[..end]).is_err());
    }
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
            detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
                detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
                detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
    let mut expected = vec![14_u8, 3];
    expected.extend_from_slice(&0x0102_0304_0506_0708_u64.to_be_bytes());
    expected.extend_from_slice(&24_u16.to_be_bytes());
    expected.extend_from_slice(&7_u64.to_be_bytes());
    expected.extend_from_slice(&1_u64.to_be_bytes());
    expected.extend_from_slice(&1_u64.to_be_bytes());
    expected.extend_from_slice(&112_u32.to_be_bytes());
    assert_eq!(
        &encode(
            request,
            TerminalViewerAction::ReadViewportRows {
                detail: orchbee_capability_node_protocol::ViewerRowDetail::Render,
                origin: ViewportRowOrigin::Absolute {
                    row: 0x0102_0304_0506_0708,
                },
                row_count: 24,
                revision: 7,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 112,
            },
        )?[65..],
        expected.as_slice()
    );
    // The committed delta is a signed big-endian i32, the tail distance an
    // unsigned u32 and the absolute row an unsigned u64; all round-trip across
    // their full ranges.
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
        ViewportRowOrigin::Absolute { row: 0 },
        ViewportRowOrigin::Absolute { row: u64::MAX },
    ] {
        let message = CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
            action: TerminalViewerAction::ReadViewportRows {
                detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
                detail: orchbee_capability_node_protocol::ViewerRowDetail::Text,
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
        (66, 4_u8.to_be_bytes().to_vec()),
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
    unknown_action[65] = 13;
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

#[test]
fn viewport_render_preserves_the_text_body_and_uses_its_own_tag() -> TestResult {
    let mut bytes = Vec::new();
    for detail in [ViewerRowDetail::Text, ViewerRowDetail::Render] {
        let message = CapabilityNodeMessage::TerminalViewerRequest(TerminalViewerRequest {
            request_id: OperationId::from_bytes([1; 16])?,
            attachment_id: OperationId::from_bytes([2; 16])?,
            worker_id: WorkerId::from_bytes([3; 16])?,
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action: TerminalViewerAction::ReadViewportRows {
                detail,
                origin: ViewportRowOrigin::Committed { delta: -3 },
                row_count: 24,
                revision: 4,
                generation: NonZeroU64::MIN,
                request_sequence: NonZeroU64::MIN,
                maximum_bytes: 8388608,
            },
        });
        let encoded = encode_message(&message)?;
        assert_eq!(decode_message(&encoded)?, message);
        assert_eq!(
            encoded[65],
            if detail == ViewerRowDetail::Text {
                9
            } else {
                14
            }
        );
        assert!(decode_message(&encoded[..encoded.len() - 1]).is_err());
        let mut unknown = encoded.clone();
        unknown[65] = 15;
        assert!(decode_message(&unknown).is_err());
        bytes.push(encoded);
    }
    assert_eq!(&bytes[0][..65], &bytes[1][..65]);
    assert_eq!(&bytes[0][66..], &bytes[1][66..]);
    Ok(())
}

fn raw_viewer_action_frame(action: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let request = TerminalViewerRequest {
        request_id: OperationId::from_bytes([1; 16])?,
        attachment_id: OperationId::from_bytes([5; 16])?,
        worker_id: WorkerId::from_bytes([6; 16])?,
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action: TerminalViewerAction::Detach,
    };
    let mut frame = encode_message(&CapabilityNodeMessage::TerminalViewerRequest(request))?;
    // OBCN header (8) + request kind (1) + request id (16) + worker id
    // (16) + worker generation (8) + attachment id (16).
    frame.truncate(65);
    frame.extend_from_slice(action);
    let payload_length = u16::try_from(frame.len() - 8)?;
    frame[6..8].copy_from_slice(&payload_length.to_be_bytes());
    Ok(frame)
}

/// Tags 16 through 20 add the viewer selection control plane without moving
/// attachment identity into an action. Accepted raw peers must round-trip to
/// the same bytes so no field is silently reordered, widened, or defaulted.
#[test]
fn selection_action_tags_sixteen_through_twenty_have_fixed_round_trip_bytes() -> TestResult {
    let mut start = vec![16];
    start.extend_from_slice(&9_u64.to_be_bytes());
    start.push(1); // start
    start.push(2); // double-click
    start.extend_from_slice(&3_u16.to_be_bytes());
    start.extend_from_slice(&4_u16.to_be_bytes());
    start.extend_from_slice(&(-5_i32).to_be_bytes());
    start.extend_from_slice(&6_i32.to_be_bytes());
    start.push(1); // select output

    let mut update = vec![16];
    update.extend_from_slice(&9_u64.to_be_bytes());
    update.push(2); // update
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&80_u16.to_be_bytes());
    update.extend_from_slice(&2_u16.to_be_bytes());
    update.extend_from_slice(&480_u16.to_be_bytes());
    update.extend_from_slice(&7_u16.to_be_bytes());
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&9_i32.to_be_bytes());
    update.extend_from_slice(&(-10_i32).to_be_bytes());
    update.push(0); // linear selection

    let mut end = vec![16];
    end.extend_from_slice(&9_u64.to_be_bytes());
    end.push(3); // end
    end.extend_from_slice(&11_u16.to_be_bytes());
    end.extend_from_slice(&12_u16.to_be_bytes());

    let mut cancel_gesture = vec![16];
    cancel_gesture.extend_from_slice(&9_u64.to_be_bytes());
    cancel_gesture.push(4); // cancel

    let mut export = vec![17];
    export.extend_from_slice(&10_u64.to_be_bytes());
    export.push(1); // plain

    let mut chunk_applied = vec![18];
    chunk_applied.extend_from_slice(&10_u64.to_be_bytes());
    chunk_applied.extend_from_slice(&42_u32.to_be_bytes());

    let mut cancel_export = vec![19];
    cancel_export.extend_from_slice(&10_u64.to_be_bytes());
    cancel_export.push(1); // clipboard-failed

    let mut clear = vec![20];
    clear.extend_from_slice(&9_u64.to_be_bytes());
    clear.extend_from_slice(&7_u64.to_be_bytes());
    clear.push(1); // clear

    let mut select_all = vec![20];
    select_all.extend_from_slice(&9_u64.to_be_bytes());
    select_all.extend_from_slice(&0_u64.to_be_bytes());
    select_all.push(2); // select all

    let mut set_range = vec![20];
    set_range.extend_from_slice(&9_u64.to_be_bytes());
    set_range.extend_from_slice(&7_u64.to_be_bytes());
    set_range.push(3); // set range
    set_range.extend_from_slice(&0_u16.to_be_bytes());
    set_range.extend_from_slice(&0_u64.to_be_bytes());
    set_range.extend_from_slice(&4095_u16.to_be_bytes());
    set_range.extend_from_slice(&u64::MAX.to_be_bytes());
    set_range.push(1); // rectangle

    let mut select_output = vec![20];
    select_output.extend_from_slice(&9_u64.to_be_bytes());
    select_output.extend_from_slice(&7_u64.to_be_bytes());
    select_output.push(4); // select output
    select_output.extend_from_slice(&3_u16.to_be_bytes());
    select_output.extend_from_slice(&12_u64.to_be_bytes());

    for action in [
        start,
        update,
        end,
        cancel_gesture,
        export,
        chunk_applied,
        cancel_export,
        clear,
        select_all,
        set_range,
        select_output,
    ] {
        let frame = raw_viewer_action_frame(&action)?;
        assert_eq!(&frame[65..], action.as_slice());
        let decoded = decode_message(&frame)?;
        assert_eq!(encode_message(&decoded)?, frame);
        assert_eq!(read_message(&mut Cursor::new(&frame))?, decoded);
    }

    // Every closed cancel-reason code is stable on the same tag.
    for reason in 1..=4 {
        let mut action = vec![19];
        action.extend_from_slice(&10_u64.to_be_bytes());
        action.push(reason);
        let frame = raw_viewer_action_frame(&action)?;
        assert_eq!(encode_message(&decode_message(&frame)?)?, frame);
    }
    Ok(())
}

/// The CNP carrier mirrors the Terminal Protocol's hard gesture bounds and
/// closed export vocabularies. It rejects malformed bytes before Worker Host
/// access, including a declared trailing byte inside an otherwise valid action.
#[test]
fn selection_actions_reject_every_semantic_boundary_and_open_discriminant() -> TestResult {
    let mut start = vec![16];
    start.extend_from_slice(&9_u64.to_be_bytes());
    start.push(1);
    start.push(2);
    start.extend_from_slice(&3_u16.to_be_bytes());
    start.extend_from_slice(&4_u16.to_be_bytes());
    start.extend_from_slice(&(-5_i32).to_be_bytes());
    start.extend_from_slice(&6_i32.to_be_bytes());
    start.push(1);
    let start = raw_viewer_action_frame(&start)?;
    for (offset, bytes) in [
        (66, 0_u64.to_be_bytes().to_vec()),
        (74, vec![0]),
        (74, vec![5]),
        (75, vec![0]),
        (75, vec![4]),
        (76, 4096_u16.to_be_bytes().to_vec()),
        (78, 4096_u16.to_be_bytes().to_vec()),
        (80, 65_536_i32.to_be_bytes().to_vec()),
        (84, (-65_536_i32).to_be_bytes().to_vec()),
        (88, vec![2]),
    ] {
        let mut invalid = start.clone();
        invalid[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(
            decode_message(&invalid).is_err(),
            "accepted mutation at {offset}"
        );
    }

    let mut update = vec![16];
    update.extend_from_slice(&9_u64.to_be_bytes());
    update.push(2);
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&80_u16.to_be_bytes());
    update.extend_from_slice(&2_u16.to_be_bytes());
    update.extend_from_slice(&480_u16.to_be_bytes());
    update.extend_from_slice(&7_u16.to_be_bytes());
    update.extend_from_slice(&8_u16.to_be_bytes());
    update.extend_from_slice(&9_i32.to_be_bytes());
    update.extend_from_slice(&(-10_i32).to_be_bytes());
    update.push(0);
    let update = raw_viewer_action_frame(&update)?;
    for (offset, bytes) in [
        (75, 0_u16.to_be_bytes().to_vec()),
        (75, 4097_u16.to_be_bytes().to_vec()),
        (77, 0_u16.to_be_bytes().to_vec()),
        (77, 4097_u16.to_be_bytes().to_vec()),
        (81, 0_u16.to_be_bytes().to_vec()),
        (83, 4096_u16.to_be_bytes().to_vec()),
        (85, 4096_u16.to_be_bytes().to_vec()),
        (87, 65_536_i32.to_be_bytes().to_vec()),
        (91, (-65_536_i32).to_be_bytes().to_vec()),
        (95, vec![2]),
    ] {
        let mut invalid = update.clone();
        invalid[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(
            decode_message(&invalid).is_err(),
            "accepted mutation at {offset}"
        );
    }

    for (tag, expected_length, mutations) in [
        (
            17_u8,
            75_usize,
            vec![(66, 0_u64.to_be_bytes().to_vec()), (74, vec![2])],
        ),
        (18, 78, vec![(66, 0_u64.to_be_bytes().to_vec())]),
        (
            19,
            75,
            vec![
                (66, 0_u64.to_be_bytes().to_vec()),
                (74, vec![0]),
                (74, vec![5]),
            ],
        ),
    ] {
        let mut action = vec![tag];
        action.extend_from_slice(&10_u64.to_be_bytes());
        if tag == 18 {
            action.extend_from_slice(&42_u32.to_be_bytes());
        } else {
            action.push(1);
        }
        let frame = raw_viewer_action_frame(&action)?;
        assert_eq!(frame.len(), expected_length);
        for (offset, bytes) in mutations {
            let mut invalid = frame.clone();
            invalid[offset..offset + bytes.len()].copy_from_slice(&bytes);
            assert!(decode_message(&invalid).is_err());
        }
    }

    let mut set_range = vec![20];
    set_range.extend_from_slice(&9_u64.to_be_bytes());
    set_range.extend_from_slice(&7_u64.to_be_bytes());
    set_range.push(3);
    set_range.extend_from_slice(&0_u16.to_be_bytes());
    set_range.extend_from_slice(&0_u64.to_be_bytes());
    set_range.extend_from_slice(&80_u16.to_be_bytes());
    set_range.extend_from_slice(&12_u64.to_be_bytes());
    set_range.push(0);
    let set_range = raw_viewer_action_frame(&set_range)?;
    assert!(decode_message(&set_range).is_ok());
    for (offset, bytes) in [
        (66, 0_u64.to_be_bytes().to_vec()),
        (82, vec![0]),
        (82, vec![5]),
        (83, 4096_u16.to_be_bytes().to_vec()),
        (93, 4096_u16.to_be_bytes().to_vec()),
        (103, vec![2]),
    ] {
        let mut invalid = set_range.clone();
        invalid[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(
            decode_message(&invalid).is_err(),
            "accepted mutation at {offset}"
        );
    }

    let mut unknown = start.clone();
    unknown[65] = 21;
    assert!(decode_message(&unknown).is_err());
    assert!(decode_message(&start[..start.len() - 1]).is_err());
    let mut declared_trailing = start;
    declared_trailing.push(0);
    let payload_length = u16::try_from(declared_trailing.len() - 8)?;
    declared_trailing[6..8].copy_from_slice(&payload_length.to_be_bytes());
    assert!(decode_message(&declared_trailing).is_err());
    Ok(())
}

/// The hint carries the attachment and nothing else, so its layout is fixed.
#[test]
fn record_ready_layout_is_fixed_and_refuses_trailing_bytes() -> TestResult {
    let message = CapabilityNodeMessage::TerminalViewerRecordReady(TerminalViewerRecordReady {
        attachment_id: OperationId::from_bytes([5; 16])?,
    });
    let encoded = encode_message(&message)?;
    assert_eq!(decode_message(&encoded)?, message);
    assert_eq!(read_message(&mut Cursor::new(&encoded))?, message);
    // OBCN header (8) + kind (1) + attachment id (16).
    assert_eq!(encoded.len(), 25);
    assert_eq!(encoded[8], 33);
    assert_eq!(&encoded[9..], &[5; 16]);
    let mut zero_identity = encoded.clone();
    zero_identity[9..25].fill(0);
    assert!(decode_message(&zero_identity).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_message(&trailing).is_err());
    Ok(())
}
