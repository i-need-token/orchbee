use super::*;
use orchbee_capability_node_protocol::{OperationId, WorkerGeneration};

#[test]
fn editor_command_rejects_wrong_incarnation_before_preference_resolution_and_echoes_the_request() {
    let worker = WorkerId::from_bytes([2; 16]);
    let current = Request {
        request_id: OperationId::from_bytes([1; 16]).unwrap(),
        worker_id: orchbee_capability_node_protocol::WorkerId::from_bytes([2; 16]).unwrap(),
        worker_generation: WorkerGeneration::new(NonZeroU64::MAX),
    };
    let mut wrong = current;
    wrong.worker_id = orchbee_capability_node_protocol::WorkerId::from_bytes([3; 16]).unwrap();
    let result = execute_with(wrong, worker, NonZeroU64::MAX, || {
        panic!("wrong Worker consulted preferences")
    });
    assert_eq!(result.request, wrong);
    assert_eq!(result.outcome, Outcome::Refused(Refusal::WrongWorker));
    let mut stale = current;
    stale.worker_generation = WorkerGeneration::new(NonZeroU64::MIN);
    let result = execute_with(stale, worker, NonZeroU64::MAX, || {
        panic!("stale Worker consulted preferences")
    });
    assert_eq!(result.request, stale);
    assert_eq!(
        result.outcome,
        Outcome::Refused(Refusal::StaleWorkerGeneration)
    );
    let result = execute_with(current, worker, NonZeroU64::MAX, || {
        Err(Refusal::InvalidCommand)
    });
    assert_eq!(result.request, current);
    assert_eq!(result.outcome, Outcome::Refused(Refusal::InvalidCommand));
}
