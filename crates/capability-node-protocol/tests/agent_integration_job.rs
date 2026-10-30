use orchbee_capability_node_protocol::{
    AgentIntegrationJobAction as Action, AgentIntegrationJobCancelOutcome as Cancel,
    AgentIntegrationJobOutcome as Outcome, AgentIntegrationJobPollOutcome as Poll,
    AgentIntegrationJobRequest, AgentIntegrationJobResult,
    AgentIntegrationJobStartOutcome as Start, CapabilityNodeMessage as Message,
    MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES, MAXIMUM_AGENT_INTEGRATION_REQUEST_BYTES, OperationId,
    WorkerGeneration, WorkerId, decode_message, encode_message,
};
use std::num::NonZeroU64;

fn id(byte: u8) -> OperationId {
    OperationId::from_bytes([byte; 16]).unwrap()
}

fn request(action: Action) -> Message {
    Message::AgentIntegrationJobRequest(AgentIntegrationJobRequest {
        operation_id: id(1),
        worker_id: WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
        action,
    })
}

fn result(outcome: Outcome) -> Message {
    Message::AgentIntegrationJobResult(AgentIntegrationJobResult {
        operation_id: id(1),
        job_id: id(3),
        outcome,
    })
}

#[test]
fn every_action_and_outcome_round_trips_without_trailing_bytes() {
    let outcomes = [
        Start::Accepted,
        Start::Busy,
        Start::InvalidRequest,
        Start::SpawnThreadFailed,
        Start::Dead,
        Start::RejectedFencing,
    ]
    .map(Outcome::Start)
    .into_iter()
    .chain(
        [
            Poll::Pending,
            Poll::completed(b"{\"kind\":\"home-unavailable\"}".to_vec()).unwrap(),
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
        request(Action::start(b"{\"operation\":{\"kind\":\"list\"}}".to_vec()).unwrap()),
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
fn documents_stay_inside_the_contract_bounds_and_actions_admit_only_their_outcomes() {
    assert!(Action::start(Vec::new()).is_err());
    assert!(Action::start(vec![b' '; MAXIMUM_AGENT_INTEGRATION_REQUEST_BYTES + 1]).is_err());
    let largest =
        request(Action::start(vec![b' '; MAXIMUM_AGENT_INTEGRATION_REQUEST_BYTES]).unwrap());
    assert_eq!(
        decode_message(&encode_message(&largest).unwrap()).unwrap(),
        largest
    );
    assert!(Poll::completed(Vec::new()).is_err());
    assert!(Poll::completed(vec![b' '; MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES + 1]).is_err());
    let largest = result(Outcome::Poll(
        Poll::completed(vec![b' '; MAXIMUM_AGENT_INTEGRATION_REPORT_BYTES]).unwrap(),
    ));
    assert_eq!(
        decode_message(&encode_message(&largest).unwrap()).unwrap(),
        largest
    );
    assert!(Action::Poll { job_id: id(3) }.admits(&Outcome::Poll(Poll::Pending)));
    assert!(!Action::Poll { job_id: id(3) }.admits(&Outcome::Cancel(Cancel::Registered)));
    assert!(!Action::Cancel { job_id: id(3) }.admits(&Outcome::Start(Start::Accepted)));
}
