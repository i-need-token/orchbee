use orchbee_capability_node_protocol::{
    AbsoluteCwd, CapabilityNodeMessage, GitReference, OperationId, WorkerGeneration, WorkerId,
    WorktreeCheckout, WorktreeCheckoutSelector, WorktreeMutation, WorktreeMutationCancelOutcome,
    WorktreeMutationCompletion, WorktreeMutationEffect, WorktreeMutationError,
    WorktreeMutationFailure, WorktreeMutationIntent, WorktreeMutationJobAction,
    WorktreeMutationOutcome, WorktreeMutationPollOutcome, WorktreeMutationRequest,
    WorktreeMutationResult, WorktreeMutationStage, WorktreeMutationStartOutcome, decode_message,
    encode_message,
};
use std::num::NonZeroU64;

fn path(value: &str) -> AbsoluteCwd {
    AbsoluteCwd::new(value).unwrap()
}

fn reference(value: &str) -> GitReference {
    GitReference::new(value).unwrap()
}

fn operation(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}

fn request(action: WorktreeMutationJobAction) -> CapabilityNodeMessage {
    CapabilityNodeMessage::WorktreeMutationRequest(WorktreeMutationRequest {
        operation_id: operation(1),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        action,
    })
}

fn result(outcome: WorktreeMutationOutcome) -> CapabilityNodeMessage {
    CapabilityNodeMessage::WorktreeMutationResult(WorktreeMutationResult {
        operation_id: operation(1),
        job_id: operation(4),
        outcome,
    })
}

fn completed(value: WorktreeMutationCompletion) -> CapabilityNodeMessage {
    result(WorktreeMutationOutcome::Poll(
        WorktreeMutationPollOutcome::Completed(value),
    ))
}

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = b"OBCN\0\x01".to_vec();
    frame.extend(u16::try_from(payload.len()).unwrap().to_be_bytes());
    frame.extend(payload);
    frame
}

fn conformance(message: CapabilityNodeMessage) -> Vec<u8> {
    let encoded = encode_message(&message).unwrap();
    assert_eq!(decode_message(&encoded).unwrap(), message);
    let payload = &encoded[8..];
    // Rebuild each frame header: these exercise payload truncation, not only
    // a mismatch between the outer declared size and available bytes.
    for length in 0..payload.len() {
        assert!(
            decode_message(&frame(&payload[..length])).is_err(),
            "prefix {length}"
        );
    }
    let mut extended = payload.to_vec();
    extended.push(0);
    assert!(decode_message(&frame(&extended)).is_err());
    encoded
}

#[test]
fn golden_open_request_and_cancel_result_are_independently_encoded() {
    let open = request(WorktreeMutationJobAction::Start(
        WorktreeMutationIntent::new(
            path("C:/r"),
            true,
            WorktreeMutation::Open(WorktreeCheckoutSelector::Path(path("C:/w"))),
        )
        .unwrap(),
    ));
    let mut expected = vec![21];
    expected.extend([1; 16]);
    expected.extend([2; 16]);
    // generation=3; Start=1; trust=1; Open=2; source length=4;
    // selector Path=1; target length=4.
    expected.extend(b"\0\0\0\0\0\0\0\x03\x01\x01\x02\0\x04C:/r\x01\0\x04C:/w");
    assert_eq!(conformance(open), frame(&expected));

    let mut expected = vec![22];
    expected.extend([1; 16]);
    expected.extend([4; 16]);
    expected.extend([3, 2]); // Cancel / AlreadyTerminal.
    assert_eq!(
        conformance(result(WorktreeMutationOutcome::Cancel(
            WorktreeMutationCancelOutcome::AlreadyTerminal
        ))),
        frame(&expected)
    );
}

#[test]
fn every_intent_and_job_action_round_trips_with_strict_prefixes() {
    for trust in [false, true] {
        for mutation in [
            WorktreeMutation::InspectSource,
            WorktreeMutation::Create {
                path: path("C:/目录/w"),
                branch: reference("topic/一"),
                base: reference("HEAD~1"),
            },
            WorktreeMutation::Open(WorktreeCheckoutSelector::Path(path("C:/w"))),
            WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(reference("topic"))),
            WorktreeMutation::InspectRemove { path: path("C:/w") },
            WorktreeMutation::Remove {
                path: path("C:/w"),
                expected_common_directory: path("C:/r/.git"),
                force: false,
            },
            WorktreeMutation::Remove {
                path: path("C:/w"),
                expected_common_directory: path("C:/r/.git"),
                force: true,
            },
        ] {
            conformance(request(WorktreeMutationJobAction::Start(
                WorktreeMutationIntent::new(path("C:/r"), trust, mutation).unwrap(),
            )));
        }
    }
    conformance(request(WorktreeMutationJobAction::Poll {
        job_id: operation(4),
    }));
    conformance(request(WorktreeMutationJobAction::Cancel {
        job_id: operation(4),
    }));
}

#[test]
fn every_job_outcome_round_trips_with_strict_prefixes() {
    for outcome in [
        WorktreeMutationStartOutcome::Accepted,
        WorktreeMutationStartOutcome::Existing,
        WorktreeMutationStartOutcome::Busy,
        WorktreeMutationStartOutcome::Conflict,
        WorktreeMutationStartOutcome::Full,
        WorktreeMutationStartOutcome::SpawnFailed,
        WorktreeMutationStartOutcome::PhysicalUnknown,
        WorktreeMutationStartOutcome::Unsupported,
        WorktreeMutationStartOutcome::RejectedFencing,
    ] {
        conformance(result(WorktreeMutationOutcome::Start(outcome)));
    }
    for outcome in [
        WorktreeMutationPollOutcome::Pending,
        WorktreeMutationPollOutcome::Unknown,
        WorktreeMutationPollOutcome::PhysicalUnknown,
        WorktreeMutationPollOutcome::RejectedFencing,
    ] {
        conformance(result(WorktreeMutationOutcome::Poll(outcome)));
    }
    for outcome in [
        WorktreeMutationCancelOutcome::Registered,
        WorktreeMutationCancelOutcome::AlreadyTerminal,
        WorktreeMutationCancelOutcome::Unknown,
        WorktreeMutationCancelOutcome::PhysicalUnknown,
        WorktreeMutationCancelOutcome::RejectedFencing,
    ] {
        conformance(result(WorktreeMutationOutcome::Cancel(outcome)));
    }
    for branch in [None, Some(reference("topic"))] {
        for dirty in [false, true] {
            conformance(completed(WorktreeMutationCompletion::Checkout(
                WorktreeCheckout::new(
                    path("C:/r"),
                    path("C:/r/.git"),
                    path("C:/w"),
                    branch.clone(),
                    dirty,
                )
                .unwrap(),
            )));
        }
    }
    conformance(completed(WorktreeMutationCompletion::Removed {
        path: path("C:/w"),
    }));
}

#[test]
fn failure_matrix_preserves_stage_effect_status_and_raw_diagnostics() {
    for stage in [
        WorktreeMutationStage::Probe,
        WorktreeMutationStage::Select,
        WorktreeMutationStage::CheckBranch,
        WorktreeMutationStage::DirtyCheck,
        WorktreeMutationStage::Mutate,
        WorktreeMutationStage::Verify,
    ] {
        for effect in [
            WorktreeMutationEffect::None,
            WorktreeMutationEffect::MayHaveChanged,
        ] {
            for error in [
                WorktreeMutationError::GitFailed(None),
                WorktreeMutationError::GitFailed(Some(-1)),
                WorktreeMutationError::GitFailed(Some(i32::MAX)),
                WorktreeMutationError::SpawnUnavailable,
                WorktreeMutationError::SpawnFailed,
                WorktreeMutationError::InvalidRepository,
                WorktreeMutationError::CheckoutNotFound,
                WorktreeMutationError::MainCheckout,
                WorktreeMutationError::Dirty,
                WorktreeMutationError::InvalidReference,
                WorktreeMutationError::TooLarge,
                WorktreeMutationError::Timeout,
                WorktreeMutationError::Cancelled,
                WorktreeMutationError::Io,
                WorktreeMutationError::RepositoryChanged,
                WorktreeMutationError::AmbiguousCheckout,
            ] {
                conformance(completed(WorktreeMutationCompletion::Failed(
                    WorktreeMutationFailure::new(stage, effect, error, vec![0, 255, b'\n'])
                        .unwrap(),
                )));
            }
        }
    }
}

fn sized_path(length: usize) -> AbsoluteCwd {
    path(&format!("C:/{}", "a".repeat(length - 3)))
}

#[test]
fn exact_field_budget_fits_the_frame_and_one_more_byte_is_rejected() {
    let branch = reference(&"b".repeat(1024));
    let base = reference(&"c".repeat(1024));
    let intent = WorktreeMutationIntent::new(
        sized_path(4096),
        false,
        WorktreeMutation::Create {
            path: sized_path(1792),
            branch: branch.clone(),
            base: base.clone(),
        },
    )
    .unwrap();
    let encoded = encode_message(&request(WorktreeMutationJobAction::Start(intent))).unwrap();
    assert_eq!(encoded.len() - 8, 7988);
    assert!(decode_message(&encoded).is_ok());
    assert!(
        WorktreeMutationIntent::new(
            sized_path(4096),
            false,
            WorktreeMutation::Create {
                path: sized_path(1793),
                branch: branch.clone(),
                base,
            }
        )
        .is_err()
    );
    let value = WorktreeCheckout::new(
        sized_path(4096),
        sized_path(1024),
        sized_path(1792),
        Some(branch.clone()),
        true,
    )
    .unwrap();
    let encoded = encode_message(&completed(WorktreeMutationCompletion::Checkout(value))).unwrap();
    assert_eq!(encoded.len() - 8, 7982);
    assert!(decode_message(&encoded).is_ok());
    assert!(
        WorktreeCheckout::new(
            sized_path(4096),
            sized_path(1024),
            sized_path(1793),
            Some(branch),
            true
        )
        .is_err()
    );
    assert!(GitReference::new(&"x".repeat(1025)).is_err());
    let failure = WorktreeMutationFailure::new(
        WorktreeMutationStage::Mutate,
        WorktreeMutationEffect::MayHaveChanged,
        WorktreeMutationError::Io,
        vec![255; 2048],
    )
    .unwrap();
    assert!(
        decode_message(
            &encode_message(&completed(WorktreeMutationCompletion::Failed(failure))).unwrap()
        )
        .is_ok()
    );
    assert!(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::Io,
            vec![0; 2049]
        )
        .is_err()
    );
}

#[test]
fn malformed_fences_tags_booleans_utf8_and_reference_options_are_rejected() {
    let message = request(WorktreeMutationJobAction::Start(
        WorktreeMutationIntent::new(
            path("C:/r"),
            true,
            WorktreeMutation::Open(WorktreeCheckoutSelector::Branch(reference("topic"))),
        )
        .unwrap(),
    ));
    let payload = encode_message(&message).unwrap()[8..].to_vec();
    // Byte locations come from the specified wire layout, not the decoder.
    for (offset, value) in [
        (41, 0),
        (42, 2),
        (43, 0),
        (46, 255),
        (50, 3),
        (53, b'-'),
        (54, 0),
    ] {
        let mut malformed = payload.clone();
        malformed[offset] = value;
        assert!(
            decode_message(&frame(&malformed)).is_err(),
            "offset {offset}"
        );
    }
    for range in [1..17, 17..33, 33..41] {
        let mut malformed = payload.clone();
        malformed[range].fill(0);
        assert!(decode_message(&frame(&malformed)).is_err());
    }
    for bad in ["", "-b", "--force", "a\nb", "a\0b", "a\u{7f}b"] {
        assert!(GitReference::new(bad).is_err());
    }
    for (action, outcome) in [(0, 1), (1, 9), (2, 5), (3, 5)] {
        let mut payload = vec![22];
        payload.extend([1; 16]);
        payload.extend([4; 16]);
        payload.extend([action, outcome]);
        assert!(decode_message(&frame(&payload)).is_err());
    }
}

#[test]
fn decoder_enforces_aggregate_and_diagnostic_limits_independently_of_constructors() {
    fn append(payload: &mut Vec<u8>, value: &[u8]) {
        payload.extend(u16::try_from(value.len()).unwrap().to_be_bytes());
        payload.extend(value);
    }
    // Every field is individually valid, but their sum exceeds the intent bound.
    let mut payload = vec![21];
    payload.extend([1; 16]);
    payload.extend([2; 16]);
    payload.extend(3_u64.to_be_bytes());
    payload.extend([1, 0, 1]);
    append(&mut payload, sized_path(4096).as_str().as_bytes());
    append(&mut payload, sized_path(1793).as_str().as_bytes());
    append(&mut payload, &vec![b'b'; 1024]);
    append(&mut payload, &vec![b'c'; 1024]);
    assert!(payload.len() < 8192);
    assert!(decode_message(&frame(&payload)).is_err());

    let mut payload = vec![22];
    payload.extend([1; 16]);
    payload.extend([4; 16]);
    payload.extend([2, 2, 1]); // Poll / Completed / Checkout.
    append(&mut payload, sized_path(4096).as_str().as_bytes());
    append(&mut payload, sized_path(1024).as_str().as_bytes());
    append(&mut payload, sized_path(1793).as_str().as_bytes());
    payload.push(1);
    append(&mut payload, &vec![b'b'; 1024]);
    payload.push(0);
    assert!(payload.len() < 8192);
    assert!(decode_message(&frame(&payload)).is_err());

    let mut payload = vec![22];
    payload.extend([1; 16]);
    payload.extend([4; 16]);
    payload.extend([2, 2, 3, 5, 1, 12]); // Failed at Mutate, MayHaveChanged, Io.
    append(&mut payload, &vec![0; 2049]);
    assert!(decode_message(&frame(&payload)).is_err());
}

#[test]
fn malformed_completion_options_and_failure_tags_do_not_decode() {
    let checkout = completed(WorktreeMutationCompletion::Checkout(
        WorktreeCheckout::new(path("C:/r"), path("C:/g"), path("C:/w"), None, false).unwrap(),
    ));
    let payload = encode_message(&checkout).unwrap()[8..].to_vec();
    for (offset, value) in [(35, 0), (54, 2), (55, 2)] {
        let mut malformed = payload.clone();
        malformed[offset] = value;
        assert!(decode_message(&frame(&malformed)).is_err());
    }
    let failure = completed(WorktreeMutationCompletion::Failed(
        WorktreeMutationFailure::new(
            WorktreeMutationStage::Mutate,
            WorktreeMutationEffect::MayHaveChanged,
            WorktreeMutationError::GitFailed(None),
            vec![],
        )
        .unwrap(),
    ));
    let payload = encode_message(&failure).unwrap()[8..].to_vec();
    for (offset, value) in [(36, 0), (36, 7), (37, 2), (38, 0), (38, 15), (39, 2)] {
        let mut malformed = payload.clone();
        malformed[offset] = value;
        assert!(decode_message(&frame(&malformed)).is_err());
    }
    for range in [1..17, 17..33] {
        let mut malformed = payload.clone();
        malformed[range].fill(0);
        assert!(decode_message(&frame(&malformed)).is_err());
    }
}
