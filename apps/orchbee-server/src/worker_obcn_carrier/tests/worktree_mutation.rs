use super::super::{
    WorkerOperationRequest, WorkerOperationResult, WorkerResponseFailure, validate_response,
};
use orchbee_capability_node_protocol::{
    AbsoluteCwd, CapabilityNodeMessage, OperationId, WorkerGeneration, WorkerId, WorktreeMutation,
    WorktreeMutationCancelOutcome as Cancel, WorktreeMutationIntent,
    WorktreeMutationJobAction as Action, WorktreeMutationOutcome as Outcome,
    WorktreeMutationPollOutcome as Poll, WorktreeMutationRequest as Request,
    WorktreeMutationResult as Reply, WorktreeMutationStartOutcome as Start,
};
use std::num::NonZeroU64;

#[test]
fn mutation_result_must_match_exchange_job_and_action_independently() {
    let operation = OperationId::from_bytes([1; 16]).unwrap();
    let retained = OperationId::from_bytes([2; 16]).unwrap();
    let wrong = OperationId::from_bytes([3; 16]).unwrap();
    let source = WorktreeMutationIntent::new(
        AbsoluteCwd::new("C:/repo").unwrap(),
        false,
        WorktreeMutation::InspectSource,
    )
    .unwrap();
    for (action, job_id, outcome) in [
        (
            Action::Start(source),
            operation,
            Outcome::Start(Start::Accepted),
        ),
        (
            Action::Poll { job_id: retained },
            retained,
            Outcome::Poll(Poll::Pending),
        ),
        (
            Action::Cancel { job_id: retained },
            retained,
            Outcome::Cancel(Cancel::Registered),
        ),
    ] {
        let request = WorkerOperationRequest::WorktreeMutation(Request {
            operation_id: operation,
            worker_id: WorkerId::from_bytes([4; 16]).unwrap(),
            worker_generation: WorkerGeneration::new(NonZeroU64::MIN),
            action,
        });
        let reply = Reply {
            operation_id: operation,
            job_id,
            outcome,
        };
        assert_eq!(
            validate_response(
                &request,
                CapabilityNodeMessage::WorktreeMutationResult(reply.clone())
            ),
            Ok(WorkerOperationResult::WorktreeMutation(reply.clone()))
        );
        for invalid in [
            Reply {
                operation_id: wrong,
                ..reply.clone()
            },
            Reply {
                job_id: wrong,
                ..reply.clone()
            },
        ] {
            assert_eq!(
                validate_response(
                    &request,
                    CapabilityNodeMessage::WorktreeMutationResult(invalid)
                ),
                Err(WorkerResponseFailure::MismatchedCorrelation)
            );
        }
        let changed = match reply.outcome {
            Outcome::Start(_) => Outcome::Poll(Poll::Pending),
            _ => Outcome::Start(Start::Accepted),
        };
        assert_eq!(
            validate_response(
                &request,
                CapabilityNodeMessage::WorktreeMutationResult(Reply {
                    outcome: changed,
                    ..reply
                })
            ),
            Err(WorkerResponseFailure::WrongResultKind)
        );
    }
}
