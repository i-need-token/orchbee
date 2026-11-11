use orchbee_capability_node_protocol::{
    AgentProbeJobAction as Action, AgentProbeJobCancelOutcome as Cancel,
    AgentProbeJobOutcome as Outcome, AgentProbeJobPollOutcome as Poll, AgentProbeJobRequest,
    AgentProbeJobResult, AgentProbeJobStartOutcome as Start, AgentProbeResult as Probe,
    CapabilityNodeMessage as Message, MAXIMUM_AGENT_PROBE_COMMAND_BYTES,
    MAXIMUM_AGENT_PROBE_COMMANDS, OperationId, WorkerGeneration, WorkerId, decode_message,
    encode_message,
};
use std::num::NonZeroU64;

fn id(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}

fn request(action: Action) -> Message {
    Message::AgentProbeJobRequest(AgentProbeJobRequest {
        operation_id: id(1),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    })
}

fn result(outcome: Outcome) -> Message {
    Message::AgentProbeJobResult(AgentProbeJobResult {
        operation_id: id(1),
        job_id: id(3),
        outcome,
    })
}

fn commands(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn every_action_and_outcome_round_trips_without_trailing_bytes() {
    let outcomes = [
        Start::Accepted,
        Start::Busy,
        Start::Unsupported,
        Start::SpawnThreadFailed,
        Start::Dead,
        Start::RejectedFencing,
    ]
    .map(Outcome::Start)
    .into_iter()
    .chain(
        [
            Poll::Pending,
            Poll::completed(vec![Probe::Found, Probe::Missing, Probe::Unavailable]).unwrap(),
            Poll::Abandoned,
            Poll::Unknown,
            Poll::RejectedFencing,
        ]
        .map(Outcome::Poll),
    )
    .chain(
        [
            Cancel::Registered,
            Cancel::AlreadyTerminal,
            Cancel::Unknown,
            Cancel::RejectedFencing,
        ]
        .map(Outcome::Cancel),
    );
    let messages = [
        request(Action::start(commands(&["codex", "kiro-cli", "qwen"])).unwrap()),
        request(Action::Poll { job_id: id(3) }),
        request(Action::Cancel { job_id: id(3) }),
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
fn commands_and_results_stay_inside_their_bounds_and_actions_admit_only_their_outcomes() {
    assert!(Action::start(Vec::new()).is_err());
    let longest = "c".repeat(MAXIMUM_AGENT_PROBE_COMMAND_BYTES);
    let largest =
        request(Action::start(vec![longest.clone(); MAXIMUM_AGENT_PROBE_COMMANDS]).unwrap());
    assert_eq!(
        decode_message(&encode_message(&largest).unwrap()).unwrap(),
        largest
    );
    assert!(Action::start(vec![longest.clone(); MAXIMUM_AGENT_PROBE_COMMANDS + 1]).is_err());
    assert!(Action::start(vec![format!("{longest}c")]).is_err());
    // A name is one bare word: no whitespace, quote, control or non-ASCII byte, and never empty.
    for name in [
        "",
        "claude code",
        "codex\t",
        "\"codex\"",
        "'codex'",
        "co\u{7f}dex",
        "cødex",
    ] {
        assert!(Action::start(commands(&[name])).is_err(), "{name:?}");
    }
    assert!(Poll::completed(Vec::new()).is_err());
    assert!(Poll::completed(vec![Probe::Found; MAXIMUM_AGENT_PROBE_COMMANDS + 1]).is_err());
    let largest = result(Outcome::Poll(
        Poll::completed(vec![Probe::Missing; MAXIMUM_AGENT_PROBE_COMMANDS]).unwrap(),
    ));
    assert_eq!(
        decode_message(&encode_message(&largest).unwrap()).unwrap(),
        largest
    );
    assert!(Action::Poll { job_id: id(3) }.admits(&Outcome::Poll(Poll::Pending)));
    assert!(!Action::Poll { job_id: id(3) }.admits(&Outcome::Cancel(Cancel::Registered)));
    assert!(!Action::Cancel { job_id: id(3) }.admits(&Outcome::Start(Start::Accepted)));
}
