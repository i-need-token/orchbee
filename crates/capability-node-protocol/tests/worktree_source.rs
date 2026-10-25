use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES, OperationId, WorkerGeneration,
    WorkerId, WorktreeJobAction, WorktreeJobOutcome, WorktreeJobPollOutcome, WorktreeJobRequest,
    WorktreeJobResult, WorktreeJobStartOutcome, decode_message, encode_message,
};
use std::num::NonZeroU64;

fn operation() -> OperationId {
    OperationId::from_bytes([1; 16]).unwrap()
}
fn request(action: WorktreeJobAction) -> CapabilityNodeMessage {
    CapabilityNodeMessage::WorktreeJobRequest(WorktreeJobRequest {
        operation_id: operation(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::new(3).unwrap()),
        action,
    })
}
fn result(outcome: WorktreeJobOutcome) -> CapabilityNodeMessage {
    CapabilityNodeMessage::WorktreeJobResult(WorktreeJobResult {
        operation_id: operation(),
        job_id: operation(),
        outcome,
    })
}
fn frame(payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"OBCN\0\x01".to_vec();
    bytes.extend(u16::try_from(payload.len()).unwrap().to_be_bytes());
    bytes.extend(payload);
    bytes
}
fn conformance(message: CapabilityNodeMessage) -> Vec<u8> {
    let encoded = encode_message(&message).unwrap();
    assert_eq!(decode_message(&encoded).unwrap(), message);
    for length in 0..encoded.len() - 8 {
        assert!(decode_message(&frame(&encoded[8..8 + length])).is_err());
    }
    let mut extended = encoded[8..].to_vec();
    extended.push(0);
    assert!(decode_message(&frame(&extended)).is_err());
    encoded
}

#[test]
fn start_has_one_tag_and_strict_optional_trust() {
    for (trust, tag) in [(None, 0), (Some(false), 1), (Some(true), 2)] {
        let encoded = conformance(request(
            WorktreeJobAction::start(b"C:/r".to_vec(), trust).unwrap(),
        ));
        assert_eq!(&encoded[49..], &[1, tag, 0, 4, b'C', b':', b'/', b'r']);
        let mut malformed = encoded;
        malformed[50] = 3;
        assert!(decode_message(&malformed).is_err());
    }
    for directory in [
        b"relative".to_vec(),
        b"C:/r\0bad".to_vec(),
        vec![b'/'; 4097],
    ] {
        assert!(WorktreeJobAction::start(directory, Some(false)).is_err());
    }
}

#[test]
fn source_results_bound_all_four_fields_and_keep_unsupported_typed() {
    let completed = WorktreeJobPollOutcome::source_completed(
        b"C:/r/.git\n".to_vec(),
        b"C:/r/subtree\n".to_vec(),
        b"worktree C:/r\0bare\0\0".to_vec(),
        vec![0, 255],
    )
    .unwrap();
    let bytes = conformance(result(WorktreeJobOutcome::Poll(completed)));
    assert_eq!(bytes[42], 13);
    conformance(result(WorktreeJobOutcome::Start(
        WorktreeJobStartOutcome::Unsupported,
    )));
    let cap = MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES;
    let at_limit =
        WorktreeJobPollOutcome::source_completed(vec![1; cap], vec![], vec![], vec![]).unwrap();
    let encoded = encode_message(&result(WorktreeJobOutcome::Poll(at_limit))).unwrap();
    assert!(decode_message(&encoded).is_ok());
    assert!(
        WorktreeJobPollOutcome::source_completed(vec![1; cap], vec![2], vec![], vec![]).is_err()
    );
}

#[test]
fn status_action_and_terminal_receipt_are_separate_bounded_and_closed() {
    let start = conformance(request(
        WorktreeJobAction::start_status(b"C:/r".to_vec()).unwrap(),
    ));
    assert_eq!(&start[49..], b"\x05\x00\x04C:/r");
    assert!(WorktreeJobAction::start_status(b"relative".to_vec()).is_err());
    let completed =
        WorktreeJobPollOutcome::status_completed(b"# branch.head main\n".to_vec(), vec![]).unwrap();
    let frame = conformance(result(WorktreeJobOutcome::Poll(completed)));
    assert_eq!(frame[42], 14);
    assert!(
        WorktreeJobPollOutcome::status_completed(
            vec![0; MAXIMUM_WORKTREE_JOB_RESULT_FIELDS_BYTES],
            vec![1]
        )
        .is_err()
    );
}
