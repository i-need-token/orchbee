//! Worker identity fencing for semantic Git operations, before any slot access.
use orchbee_capability_node_protocol::{
    WorktreeMutationCancelOutcome as Cancel, WorktreeMutationJobAction as Action,
    WorktreeMutationOutcome as Outcome, WorktreeMutationPollOutcome as Poll,
    WorktreeMutationRequest, WorktreeMutationResult, WorktreeMutationStartOutcome as Start,
};
use orchbee_local_site_protocol::WorkerId;
use std::num::NonZeroU64;

#[cfg(windows)]
pub(crate) fn execute(
    request: WorktreeMutationRequest,
    worker: WorkerId,
    generation: NonZeroU64,
    slot: &crate::worktree_mutation_job::WorktreeMutationSlot,
) -> WorktreeMutationResult {
    if request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get()
    {
        return rejected(request, true);
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

#[cfg(not(windows))]
pub(crate) fn execute(
    request: WorktreeMutationRequest,
    worker: WorkerId,
    generation: NonZeroU64,
) -> WorktreeMutationResult {
    let fenced = request.worker_id.as_bytes() != worker.as_bytes()
        || request.worker_generation.get() != generation.get();
    rejected(request, fenced)
}

fn rejected(request: WorktreeMutationRequest, fenced: bool) -> WorktreeMutationResult {
    let (job_id, outcome) = match request.action {
        Action::Start(_) => (
            request.operation_id,
            Outcome::Start(if fenced {
                Start::RejectedFencing
            } else {
                Start::Unsupported
            }),
        ),
        Action::Poll { job_id } => (
            job_id,
            Outcome::Poll(if fenced {
                Poll::RejectedFencing
            } else {
                Poll::Unknown
            }),
        ),
        Action::Cancel { job_id } => (
            job_id,
            Outcome::Cancel(if fenced {
                Cancel::RejectedFencing
            } else {
                Cancel::Unknown
            }),
        ),
    };
    WorktreeMutationResult {
        operation_id: request.operation_id,
        job_id,
        outcome,
    }
}
