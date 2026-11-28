use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, OperationId, TerminalMoveToken as Token,
    TerminalMoveTurnOutcome as Outcome, TerminalMoveTurnRequest, TerminalMoveTurnResult,
    WorkerGeneration, WorkerId, decode_message, encode_message,
};
use std::num::NonZeroU64;

fn frame(payload: &[u8]) -> Vec<u8> {
    [
        b"OBCN\0\x01".as_slice(),
        &u16::try_from(payload.len()).unwrap().to_be_bytes(),
        payload,
    ]
    .concat()
}

/// A result frame for the request of ID 1, Worker ID 2 and generation 1, with `outcome`'s bytes.
fn result_frame(outcome: &[u8]) -> Vec<u8> {
    frame(&[&[44], &[1; 16][..], &[2; 16], &1_u64.to_be_bytes(), outcome].concat())
}

fn result(outcome: Outcome) -> Message {
    Message::TerminalMoveTurnResult(TerminalMoveTurnResult {
        request: TerminalMoveTurnRequest {
            request_id: OperationId::from_bytes([1; 16]).unwrap(),
            worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        },
        outcome,
    })
}

fn stopped(step: &str, reason: &str) -> Outcome {
    Outcome::Stopped {
        step: Token::new(step).unwrap(),
        reason: Token::new(reason).unwrap(),
    }
}

#[test]
fn hand_derived_stopped_results_round_trip_and_bound_their_tokens() {
    let longest = "a".repeat(Token::MAXIMUM_BYTES);
    for (outcome, bytes) in [
        (Outcome::Waiting, vec![2]),
        (Outcome::Done, vec![3]),
        (
            stopped("restore", "missing-resource"),
            [&[4, 7][..], b"restore", &[16], b"missing-resource"].concat(),
        ),
        (
            stopped("host-start", &longest),
            [&[4, 10][..], b"host-start", &[32], longest.as_bytes()].concat(),
        ),
    ] {
        let golden = result_frame(&bytes);
        assert_eq!(encode_message(&result(outcome)).unwrap(), golden);
        assert_eq!(decode_message(&golden).unwrap(), result(outcome));
    }
    // A token is 1 to 32 bytes of lowercase ASCII letters, digits and `-`, and a result ends with
    // its reason.
    for outcome in [
        [&[4, 0, 5][..], b"other"].concat(),
        [&[4, 33][..], "a".repeat(33).as_bytes(), &[5], b"other"].concat(),
        [&[4, 7][..], b"Restore", &[5], b"other"].concat(),
        [&[4, 7][..], b"restore", &[5], b"oth r"].concat(),
        [&[4, 7][..], b"restore", &[6], b"other"].concat(),
        [&[4, 7][..], b"restore", &[5], b"other", &[0]].concat(),
        vec![5],
    ] {
        assert!(decode_message(&result_frame(&outcome)).is_err());
    }
    for token in ["", "Restore", "a b", &"a".repeat(Token::MAXIMUM_BYTES + 1)] {
        assert!(Token::new(token).is_err());
    }
}
