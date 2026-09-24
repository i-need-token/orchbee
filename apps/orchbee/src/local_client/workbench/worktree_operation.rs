//! One correlated Worktree exchange. Only caller-driven reads follow an accepted operation.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWorktreeRequest, WorkbenchWorktreeResult,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeExchangeError {
    /// False proves no transport handoff; true means the request may have been accepted.
    pub(crate) submitted: bool,
    pub(crate) reason: &'static str,
}
impl LocalClient {
    pub(crate) fn submit_worktree_operation(
        &mut self,
        request: WorkbenchWorktreeRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkbenchWorktreeResult, WorktreeExchangeError> {
        let not_submitted = |reason| WorktreeExchangeError {
            submitted: false,
            reason,
        };
        if self.connection.is_none() {
            return Err(not_submitted("session-unavailable"));
        }
        let request_id = request.request_id.clone();
        let operation_id = request.action.operation_id().clone();
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::WorktreeOperation(request.clone()),
        )
        .map_err(|_| not_submitted("request-encoding"))?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| not_submitted("request-deadline"))?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| not_submitted("session-unavailable"))?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let reason = match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::WorktreeOperation(result))
                if result.request_id == request_id
                    && result.operation_id == operation_id
                    && matches_action(&request.action, &result.outcome) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_) => "response-correlation",
            ExchangeAttempt::NotSubmitted(_) => return Err(not_submitted("request-not-submitted")),
            ExchangeAttempt::WriteFailed(_) => "request-write",
            ExchangeAttempt::ReadFailed(_) => "response-read",
            ExchangeAttempt::ReadDeadline(_) => "response-deadline",
        };
        self.connection = None;
        Err(WorktreeExchangeError {
            submitted: true,
            reason,
        })
    }
}

fn matches_action(
    action: &orchbee_workbench_protocol::WorkbenchWorktreeAction,
    outcome: &orchbee_workbench_protocol::WorkbenchWorktreeOutcome,
) -> bool {
    use orchbee_workbench_protocol::{
        WorkbenchWorktreeAction as A, WorkbenchWorktreeOutcome as O, WorktreeIntent as I,
        WorktreeOperationPhase as P, WorktreeOperationStatus as S,
    };
    match (action, outcome) {
        (
            A::Start {
                project_id, intent, ..
            },
            O::Available {
                project_id: actual,
                status,
                ..
            },
        ) => {
            if actual != project_id {
                return false;
            }
            match (intent, status.as_ref()) {
                (I::Remove { .. }, S::Opened { .. })
                | (I::Create { .. } | I::Open { .. }, S::Removed { .. }) => false,
                (I::Remove { .. }, S::Pending { phase, .. }) => {
                    matches!(phase, P::InspectingRemove | P::Closing | P::Removing)
                }
                (I::Create { .. } | I::Open { .. }, S::Pending { phase, .. }) => matches!(
                    phase,
                    P::ObservingSource | P::Creating | P::Opening | P::Attaching
                ),
                _ => true,
            }
        }
        (A::Start { .. } | A::Resume { .. }, O::NotFound {}) => matches!(action, A::Resume { .. }),
        (
            A::Resume {
                expected_attempt, ..
            },
            O::Available { attempt, .. },
        ) => expected_attempt.checked_add(1) == Some(*attempt),
        _ => true,
    }
}
