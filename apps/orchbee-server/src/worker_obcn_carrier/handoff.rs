//! The old Server queue's exclusive handoff fence, settled by its sole wire owner.
use super::{
    AdmitOutcome, CarrierQueue, PendingRequest, WorkerOperationRequest, WorkerOperationResult,
    WorkerRequestError, send_completion,
};
use orchbee_capability_node_protocol::{
    WorkerHandoffAction as Action, WorkerHandoffOutcome as Outcome,
    WorkerHandoffRefusal as Refusal, WorkerHandoffRequest, WorkerHandoffTarget,
};

#[derive(Debug)]
pub(super) struct Admission {
    target: WorkerHandoffTarget,
    pending: bool,
}

impl CarrierQueue {
    pub(super) fn admit_handoff(&self, request: PendingRequest) -> AdmitOutcome {
        let WorkerOperationRequest::Handoff(handoff) = request.request else {
            unreachable!("handoff queue entry");
        };
        let drained = {
            let mut state = self.lock();
            if state.closed || state.stopping {
                return AdmitOutcome::Closed;
            }
            match &mut state.handoff {
                None => {
                    if handoff.action != Action::Prepare {
                        return AdmitOutcome::Full;
                    }
                    state.handoff = Some(Admission {
                        target: handoff.target,
                        pending: true,
                    });
                }
                Some(admission) => {
                    if admission.pending
                        || admission.target != handoff.target
                        || handoff.action == Action::Prepare
                    {
                        return AdmitOutcome::Full;
                    }
                    admission.pending = true;
                }
            }
            let drained = state.drain_queued();
            state.high.push_back(request);
            self.publish_change();
            drained
        };
        // No queue lock is held while delivering completions to ordinary owners.
        for request in drained {
            let _delivered = send_completion(request, Err(WorkerRequestError::Unavailable));
        }
        AdmitOutcome::Admitted
    }

    pub(super) fn complete_handoff(
        &self,
        request: WorkerHandoffRequest,
        result: &Result<WorkerOperationResult, WorkerRequestError>,
    ) {
        let Ok(WorkerOperationResult::Handoff(result)) = result else {
            return;
        };
        let committed = result.outcome() == Outcome::Committed;
        {
            let mut state = self.lock();
            let Some(admission) = &mut state.handoff else {
                return;
            };
            if admission.target != request.target {
                return;
            }
            admission.pending = false;
            let reopen = matches!(
                result.outcome(),
                Outcome::Aborted | Outcome::Refused(Refusal::NotPrepared)
            ) || (request.action == Action::Prepare
                && matches!(result.outcome(), Outcome::Refused(reason) if !matches!(reason, Refusal::Busy | Refusal::TicketMismatch)))
                || (request.action == Action::Commit
                    && result.outcome() == Outcome::Refused(Refusal::TargetUnavailable));
            if reopen {
                state.handoff = None;
            }
            if committed {
                state.closed = true;
            }
            self.publish_change();
        }
        if committed {
            for request in self.close_and_drain() {
                let _delivered = send_completion(request, Err(WorkerRequestError::Unavailable));
            }
        }
    }
}

#[cfg(test)]
mod tests;
