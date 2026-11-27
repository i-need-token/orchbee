use orchbee_capability_node_protocol::{
    CapabilityNodeMessage as Message, FilesEndOutcome as End, FilesEndpoint,
    FilesProcessAction as Action, FilesProcessIdentity, FilesProcessOutcome as Outcome,
    FilesProcessRequest, FilesProcessResult, FilesStartOutcome as Start, FilesToken,
    MAXIMUM_FILES_ENDPOINT_BYTES, OperationId, WorkerGeneration, WorkerId, decode_message,
    encode_message,
};
use std::num::NonZeroU64;

const PROCESS: FilesProcessIdentity = FilesProcessIdentity {
    process_id: 0x0102_0304,
    start: 0x0506_0708_090a_0b0c,
};

fn request(action: Action) -> Message {
    Message::FilesProcessRequest(FilesProcessRequest {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    })
}

fn result(outcome: Outcome) -> Message {
    Message::FilesProcessResult(FilesProcessResult {
        operation_id: OperationId::from_bytes([1; 16]).unwrap(),
        outcome,
    })
}

fn start(address: &str) -> Action {
    Action::Start {
        endpoint: FilesEndpoint::new(address.to_owned()).unwrap(),
        token: FilesToken::from_bytes([0xab; 32]),
    }
}

#[test]
fn every_action_and_outcome_round_trips_without_trailing_bytes() {
    let outcomes = [
        Start::Started(PROCESS),
        Start::Missing,
        Start::Unverified,
        Start::Failed,
        Start::RejectedFencing,
    ]
    .map(Outcome::Start)
    .into_iter()
    .chain([End::Ended, End::Unknown, End::Failed, End::RejectedFencing].map(Outcome::End));
    let messages = [
        request(start(
            "/tmp/ob-dev-0123456789abcdef-1000/0011223344556677-files.sock",
        )),
        request(start(r"\\.\pipe\orchbee-0011223344556677-files")),
        request(Action::End { process: PROCESS }),
    ]
    .into_iter()
    .chain(outcomes.map(result));
    for message in messages {
        let bytes = encode_message(&message).unwrap();
        assert_eq!(decode_message(&bytes).unwrap(), message);
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_message(&trailing).is_err());
    }
}

#[test]
fn endpoints_tokens_and_tags_stay_inside_their_domain() {
    let longest = format!("/{}", "a".repeat(MAXIMUM_FILES_ENDPOINT_BYTES - 1));
    let largest = request(start(&longest));
    assert_eq!(
        decode_message(&encode_message(&largest).unwrap()).unwrap(),
        largest
    );
    // An address is absolute, at most 512 bytes, and carries no NUL, so no start names a path
    // relative to the Worker's working directory.
    for address in [
        String::new(),
        "relative/files.sock".to_owned(),
        r"C:\files.sock".to_owned(),
        "/files\0.sock".to_owned(),
        format!("{longest}a"),
    ] {
        assert!(FilesEndpoint::new(address.clone()).is_err(), "{address:?}");
    }
    // The token never reaches a debug rendering, such as a log line that formats the request.
    assert!(!format!("{largest:?}").contains("171"));
    assert!(!format!("{:?}", FilesToken::from_bytes([0xab; 32])).contains("ab"));
    let mut unknown_action = encode_message(&request(Action::End { process: PROCESS })).unwrap();
    // Frame header (8), kind (1), operation ID (16), Worker ID (16) and generation (8) precede it.
    unknown_action[49] = 3;
    assert!(decode_message(&unknown_action).is_err());
    let mut unknown_outcome = encode_message(&result(Outcome::End(End::Ended))).unwrap();
    // Frame header (8), kind (1), operation ID (16) and the action tag precede it.
    unknown_outcome[26] = 6;
    assert!(decode_message(&unknown_outcome).is_err());
    assert!(start("/a").admits(&Outcome::Start(Start::Missing)));
    assert!(!start("/a").admits(&Outcome::End(End::Ended)));
    assert!(!Action::End { process: PROCESS }.admits(&Outcome::Start(Start::Started(PROCESS))));
}
