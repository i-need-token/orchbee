use std::{error::Error, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, LifecycleHostFailure, LifecycleInventoryCursor,
    LifecycleInventoryFailure, LifecycleInventoryPage, LifecycleRejection, LifecycleStepOrdinal,
    LifecycleTargetObservation, LifecycleTerminalPhase, LifecycleUnknownReason,
    TerminalLifecycleAction, TerminalLifecycleOutcome, TerminalLifecycleRequest,
    TerminalLifecycleResult, decode_message, encode_message, read_message,
};

type TestResult = Result<(), Box<dyn Error>>;

// Independent byte layout from the owning README, without the typed encoder.
fn request_payload(action: &[u8]) -> Vec<u8> {
    [
        &[9][..],
        &[0, 1],
        &[0x11; 16],
        &[0x22; 16],
        &1_u64.to_be_bytes(),
        &2_u64.to_be_bytes(),
        &[0x33; 16],
        &3_u64.to_be_bytes(),
        &[0x44; 16],
        &[0x55; 16],
        &4_u64.to_be_bytes(),
        &[5],
        &[0x66; 16],
        &[0x77; 16],
        &[0x88; 16],
        action,
    ]
    .concat()
}

fn frame(payload: &[u8]) -> Vec<u8> {
    [
        b"OBCN\0\x01".as_slice(),
        &u16::try_from(payload.len()).unwrap().to_be_bytes(),
        payload,
    ]
    .concat()
}

#[test]
fn hand_derived_lifecycle_request_and_acknowledgement_goldens_round_trip() -> TestResult {
    for action in 1..=3 {
        let mut payload = request_payload(&[action]);
        assert_eq!(payload.len(), 165);
        let request = frame(&payload);
        assert_eq!(encode_message(&decode_message(&request)?)?, request);
        payload[0] = 10;
        payload.push(action);
        let result = frame(&payload);
        assert_eq!(encode_message(&decode_message(&result)?)?, result);
    }
    Ok(())
}

#[test]
fn hand_derived_pre_handoff_unavailability_is_representable_for_every_action() -> TestResult {
    for action in [vec![1], vec![2], vec![3], query_action(None)] {
        let bytes = result_frame(&action, &[9]);
        let decoded = decode_message(&bytes)?;
        let CapabilityNodeMessage::TerminalLifecycleResult(result) = decoded else {
            panic!("expected lifecycle result")
        };
        assert_eq!(
            result.outcome(),
            TerminalLifecycleOutcome::HostUnavailableBeforeHandoff
        );
        assert_eq!(result.request(), request(&action));
        assert_eq!(
            TerminalLifecycleResult::new(
                request(&action),
                TerminalLifecycleOutcome::HostUnavailableBeforeHandoff
            )?,
            result
        );
        assert_eq!(encode_message(&decoded)?, bytes);
    }
    Ok(())
}

fn request(action: &[u8]) -> TerminalLifecycleRequest {
    let CapabilityNodeMessage::TerminalLifecycleRequest(request) =
        decode_message(&frame(&request_payload(action))).unwrap()
    else {
        panic!("expected lifecycle request");
    };
    request
}

fn query_action(cursor: Option<(u64, u64)>) -> Vec<u8> {
    match cursor {
        None => vec![4, 0],
        Some((generation, offset)) => [
            &[4, 1][..],
            &generation.to_be_bytes(),
            &offset.to_be_bytes(),
        ]
        .concat(),
    }
}

fn result_frame(action: &[u8], outcome: &[u8]) -> Vec<u8> {
    let mut payload = request_payload(action);
    payload[0] = 10;
    payload.extend(outcome);
    frame(&payload)
}

fn page_bytes(
    generation: u64,
    next: Option<(u64, u64)>,
    count: u16,
    phase: Option<&[u8]>,
) -> Vec<u8> {
    let mut bytes = vec![4];
    bytes.extend(generation.to_be_bytes());
    bytes.extend(&query_action(next)[1..]);
    bytes.extend(count.to_be_bytes());
    match phase {
        None => bytes.push(0),
        Some(phase) => {
            bytes.push(1);
            bytes.extend(u64::MAX.to_be_bytes());
            bytes.extend(phase);
        }
    }
    bytes
}

fn golden_frames() -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    for action in [
        vec![1],
        vec![2],
        vec![3],
        query_action(None),
        query_action(Some((7, 128))),
    ] {
        frames.push(frame(&request_payload(&action)));
        if action[0] <= 3 {
            frames.push(result_frame(&action, &[action[0]]));
        }
        for tag in 1..=8 {
            frames.push(result_frame(&action, &[5, tag]));
        }
        frames.push(result_frame(&action, &[6, 1]));
        frames.push(result_frame(&action, &[9]));
        for tag in 1..=3 {
            frames.push(result_frame(&action, &[8, tag]));
        }
    }
    frames.push(result_frame(&[3], &[6, 2]));
    for tag in 1..=4 {
        frames.push(result_frame(&query_action(None), &[7, tag]));
    }
    for phase in [
        vec![1],
        vec![2, 0],
        vec![2, 1],
        vec![3, 0],
        vec![3, 1],
        vec![4],
        vec![5, 0],
        vec![5, 1],
        vec![6],
    ] {
        frames.push(result_frame(
            &query_action(Some((7, 128))),
            &page_bytes(7, Some((7, 256)), 128, Some(&phase)),
        ));
    }
    frames.push(result_frame(
        &query_action(None),
        &page_bytes(7, None, 0, None),
    ));
    frames.push(result_frame(
        &query_action(Some((7, 128))),
        &page_bytes(7, Some((7, 129)), 1, None),
    ));
    frames
}

#[test]
fn complete_request_seal_and_six_host_phases_are_retained() -> TestResult {
    let request = request(&query_action(Some((7, 128))));
    let authorization = request.authorization;
    assert_eq!(authorization.capability_version.get(), 1);
    assert_eq!(authorization.terminal_id.as_bytes(), [0x11; 16]);
    assert_eq!(authorization.worker_id.as_bytes(), [0x22; 16]);
    assert_eq!(authorization.worker_generation.get(), 1);
    assert_eq!(authorization.binding_fence.get(), 2);
    assert_eq!(authorization.lease.id().as_bytes(), [0x33; 16]);
    assert_eq!(authorization.lease.expires_at().get(), 3);
    assert_eq!(request.close_operation_id.as_bytes(), [0x44; 16]);
    assert_eq!(request.request_id.as_bytes(), [0x55; 16]);
    assert_eq!(request.attempt_generation.get(), 4);
    assert_eq!(request.step_ordinal.get(), 5);
    assert_eq!(request.target.creation_id().as_bytes(), [0x66; 16]);
    assert_eq!(request.target.host_epoch().as_bytes(), [0x77; 16]);
    assert_eq!(request.target.terminal_id().as_bytes(), [0x88; 16]);
    for (wire, phase) in [
        (vec![1], LifecycleTerminalPhase::AwaitingStart),
        (
            vec![2, 0],
            LifecycleTerminalPhase::Running {
                terminate_requested: false,
            },
        ),
        (
            vec![2, 1],
            LifecycleTerminalPhase::Running {
                terminate_requested: true,
            },
        ),
        (
            vec![3, 0],
            LifecycleTerminalPhase::Finishing {
                terminate_requested: false,
            },
        ),
        (
            vec![3, 1],
            LifecycleTerminalPhase::Finishing {
                terminate_requested: true,
            },
        ),
        (vec![4], LifecycleTerminalPhase::ContainingFailure),
        (
            vec![5, 0],
            LifecycleTerminalPhase::Exited {
                terminate_requested: false,
            },
        ),
        (
            vec![5, 1],
            LifecycleTerminalPhase::Exited {
                terminate_requested: true,
            },
        ),
        (vec![6], LifecycleTerminalPhase::Failed),
    ] {
        let CapabilityNodeMessage::TerminalLifecycleResult(result) =
            decode_message(&result_frame(
                &query_action(Some((7, 128))),
                &page_bytes(7, Some((7, 256)), 128, Some(&wire)),
            ))?
        else {
            panic!("expected lifecycle result")
        };
        assert_eq!(result.request(), request);
        assert_eq!(
            result.outcome(),
            TerminalLifecycleOutcome::InventoryPage(LifecycleInventoryPage {
                inventory_generation: NonZeroU64::new(7).unwrap(),
                next_cursor: Some(LifecycleInventoryCursor {
                    inventory_generation: NonZeroU64::new(7).unwrap(),
                    offset: 256
                }),
                returned_count: 128,
                target: Some(LifecycleTargetObservation {
                    revision: u64::MAX,
                    phase
                }),
            })
        );
    }
    Ok(())
}

#[test]
fn all_reason_tags_keep_their_closed_meaning() -> TestResult {
    for (group, reasons, action) in [
        (
            5,
            vec![
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::WrongWorker),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::StaleWorkerGeneration),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::UnsupportedCapability),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::LeaseExpired),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::BindingUnavailable),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::BindingFenceMismatch),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::HostIdentityMismatch),
                TerminalLifecycleOutcome::Rejected(LifecycleRejection::LifecycleConflict),
            ],
            vec![1],
        ),
        (
            6,
            vec![
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::HostRefused),
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::CleanupIncomplete),
            ],
            vec![3],
        ),
        (
            7,
            vec![
                TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::StaleCursor),
                TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::PageRejected),
                TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage),
                TerminalLifecycleOutcome::InventoryFailure(
                    LifecycleInventoryFailure::IdentityConflict,
                ),
            ],
            query_action(None),
        ),
        (
            8,
            vec![
                TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::TransportLost),
                TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::Deadline),
                TerminalLifecycleOutcome::OutcomeUnknown(LifecycleUnknownReason::InvalidResponse),
            ],
            vec![1],
        ),
    ] {
        for (index, expected) in reasons.into_iter().enumerate() {
            let tag = u8::try_from(index + 1)?;
            let CapabilityNodeMessage::TerminalLifecycleResult(result) =
                decode_message(&result_frame(&action, &[group, tag]))?
            else {
                panic!("expected lifecycle result")
            };
            assert_eq!(result.outcome(), expected);
        }
    }
    Ok(())
}

#[test]
fn every_golden_round_trips_and_streams_without_consuming_the_next_frame() -> TestResult {
    let frames = golden_frames();
    let mut stream = std::io::Cursor::new(frames.concat());
    for frame in &frames {
        let expected = decode_message(frame)?;
        assert_eq!(encode_message(&expected)?, *frame);
        assert_eq!(read_message(&mut stream)?, expected);
        for length in 0..frame.len() {
            assert!(
                decode_message(&frame[..length]).is_err(),
                "accepted prefix {length}"
            );
        }
        let mut extra = frame.clone();
        extra.push(0);
        assert!(decode_message(&extra).is_err());
        let payload = &frame[8..];
        let mut trailing = payload.to_vec();
        trailing.push(0);
        assert!(decode_message(&self::frame(&trailing)).is_err());
    }
    assert_eq!(usize::try_from(stream.position())?, stream.get_ref().len());
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame[8] == 9)
            .map(Vec::len)
            .max(),
        Some(190)
    );
    assert_eq!(
        frames
            .iter()
            .filter(|frame| frame[8] == 10)
            .map(Vec::len)
            .max(),
        Some(229)
    );
    Ok(())
}

#[test]
fn malformed_seal_tags_and_positive_fields_are_rejected() {
    // Offsets are in the payload, independently derived from the documented layout.
    for (offset, length) in [
        (1, 2),
        (3, 16),
        (19, 16),
        (35, 8),
        (43, 8),
        (51, 16),
        (67, 8),
        (75, 16),
        (91, 16),
        (107, 8),
        (115, 1),
        (116, 16),
        (132, 16),
        (148, 16),
    ] {
        let mut bytes = request_payload(&[1]);
        bytes[offset..offset + length].fill(0);
        assert!(
            decode_message(&frame(&bytes)).is_err(),
            "zero field {offset}"
        );
    }
    for ordinal in [0, 33, 255] {
        assert!(LifecycleStepOrdinal::new(ordinal).is_err());
    }
    for ordinal in [1, 32] {
        assert_eq!(LifecycleStepOrdinal::new(ordinal).unwrap().get(), ordinal);
    }
    for action in [vec![0], vec![5], vec![4, 2], query_action(Some((0, 0)))] {
        assert!(decode_message(&frame(&request_payload(&action))).is_err());
    }
    for (group, last) in [(5, 8), (6, 2), (7, 4), (8, 3)] {
        for reason in [0, last + 1, 255] {
            assert!(decode_message(&result_frame(&query_action(None), &[group, reason])).is_err());
        }
    }
    for tag in [0, 10, 255] {
        assert!(decode_message(&result_frame(&[1], &[tag])).is_err());
    }
    for phase in [vec![0], vec![7], vec![2, 2], vec![3, 2], vec![5, 2]] {
        assert!(
            decode_message(&result_frame(
                &query_action(None),
                &page_bytes(7, None, 1, Some(&phase))
            ))
            .is_err()
        );
    }
}

#[test]
fn maximal_unsigned_seal_values_remain_lossless_without_reinterpreting_authority() -> TestResult {
    let mut payload = request_payload(&[1]);
    payload[1..115].fill(0xff);
    payload[115] = 32;
    payload[116..164].fill(0xff);
    let bytes = frame(&payload);
    let decoded = decode_message(&bytes)?;
    let CapabilityNodeMessage::TerminalLifecycleRequest(request) = decoded else {
        panic!("expected lifecycle request")
    };
    assert_eq!(request.authorization.capability_version.get(), u16::MAX);
    assert_eq!(request.authorization.worker_generation.get(), u64::MAX);
    assert_eq!(request.authorization.binding_fence.get(), u64::MAX);
    assert_eq!(request.authorization.lease.expires_at().get(), u64::MAX);
    assert_eq!(request.attempt_generation.get(), u64::MAX);
    assert_eq!(request.step_ordinal.get(), 32);
    assert_eq!(encode_message(&decoded)?, bytes);
    Ok(())
}

#[test]
fn page_and_action_contradictions_fail_at_construction_and_decoding() {
    for action in [
        TerminalLifecycleAction::Terminate,
        TerminalLifecycleAction::Force,
        TerminalLifecycleAction::Dispose,
        TerminalLifecycleAction::InventoryPage { cursor: None },
    ] {
        let request = TerminalLifecycleRequest {
            action,
            ..request(&[1])
        };
        for (outcome, allowed) in [
            (
                TerminalLifecycleOutcome::TerminateAccepted,
                matches!(action, TerminalLifecycleAction::Terminate),
            ),
            (
                TerminalLifecycleOutcome::ForceAccepted,
                matches!(action, TerminalLifecycleAction::Force),
            ),
            (
                TerminalLifecycleOutcome::Disposed,
                matches!(action, TerminalLifecycleAction::Dispose),
            ),
            (
                TerminalLifecycleOutcome::HostFailure(LifecycleHostFailure::CleanupIncomplete),
                matches!(action, TerminalLifecycleAction::Dispose),
            ),
            (
                TerminalLifecycleOutcome::InventoryFailure(LifecycleInventoryFailure::InvalidPage),
                matches!(action, TerminalLifecycleAction::InventoryPage { .. }),
            ),
        ] {
            assert_eq!(
                TerminalLifecycleResult::new(request, outcome).is_ok(),
                allowed
            );
        }
    }
    for (cursor, generation, next, count, phase) in [
        (None, 0, None, 0, None),
        (None, 7, None, 129, None),
        (None, 7, None, 0, Some(vec![1])),
        (None, 7, Some((7, 0)), 0, None),
        (None, 7, Some((8, 1)), 1, None),
        (None, 7, Some((7, 2)), 1, None),
        (Some((8, 0)), 7, None, 0, None),
        (Some((7, u64::MAX)), 7, None, 1, None),
    ] {
        assert!(
            decode_message(&result_frame(
                &query_action(cursor),
                &page_bytes(generation, next, count, phase.as_deref())
            ))
            .is_err()
        );
    }
    for (action, outcome) in [
        (vec![1], vec![2]),
        (vec![2], vec![3]),
        (vec![3], vec![1]),
        (vec![1], vec![6, 2]),
        (vec![1], vec![7, 1]),
    ] {
        assert!(decode_message(&result_frame(&action, &outcome)).is_err());
    }
}
