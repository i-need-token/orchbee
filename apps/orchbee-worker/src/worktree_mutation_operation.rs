//! Worker identity fencing for semantic Git operations, before any slot access.
use orchbee_capability_node_protocol::{
    WorktreeMutationCancelOutcome as Cancel, WorktreeMutationJobAction as Action,
    WorktreeMutationOutcome as Outcome, WorktreeMutationPollOutcome as Poll,
    WorktreeMutationRequest, WorktreeMutationResult, WorktreeMutationStartOutcome as Start,
};
use orchbee_local_worker_protocol::WorkerId;
use std::num::NonZeroU64;

pub(crate) fn execute(
    request: WorktreeMutationRequest,
    worker: WorkerId,
    generation: NonZeroU64,
    slot: &crate::worktree_mutation_job::WorktreeMutationSlot,
) -> WorktreeMutationResult {
    if request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        return fenced(request);
    }
    let (job_id, outcome) = match request.action {
        Action::Start(intent) => (
            request.operation_id,
            Outcome::Start(slot.start(request.operation_id, intent)),
        ),
        Action::Poll { job_id } => (job_id, Outcome::Poll(slot.poll(job_id))),
        Action::Cancel { job_id } => (job_id, Outcome::Cancel(slot.cancel(job_id))),
    };
    WorktreeMutationResult {
        operation_id: request.operation_id,
        job_id,
        outcome,
    }
}

fn fenced(request: WorktreeMutationRequest) -> WorktreeMutationResult {
    let (job_id, outcome) = match request.action {
        Action::Start(_) => (request.operation_id, Outcome::Start(Start::RejectedFencing)),
        Action::Poll { job_id } => (job_id, Outcome::Poll(Poll::RejectedFencing)),
        Action::Cancel { job_id } => (job_id, Outcome::Cancel(Cancel::RejectedFencing)),
    };
    WorktreeMutationResult {
        operation_id: request.operation_id,
        job_id,
        outcome,
    }
}
