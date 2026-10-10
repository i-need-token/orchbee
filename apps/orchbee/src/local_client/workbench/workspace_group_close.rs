//! One correlated group Close exchange. Only caller-driven reads follow an accepted operation.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWorkspaceGroupCloseRequest,
    WorkbenchWorkspaceGroupCloseResult, read_workbench_response, write_workbench_request,
};
use std::time::Instant;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GroupCloseExchangeError {
    /// False proves no transport handoff; true means the request may have been accepted.
    pub(crate) submitted: bool,
    pub(crate) reason: &'static str,
}
impl LocalClient {
    pub(crate) fn submit_workspace_group_close(
        &mut self,
        request: WorkbenchWorkspaceGroupCloseRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkbenchWorkspaceGroupCloseResult, GroupCloseExchangeError> {
        let not_submitted = |reason| GroupCloseExchangeError {
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
            &WorkbenchRequest::WorkspaceGroupClose(request.clone()),
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
            ExchangeAttempt::Result(WorkbenchResponse::WorkspaceGroupClose(result))
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
        Err(GroupCloseExchangeError {
            submitted: true,
            reason,
        })
    }
}

fn matches_action(
    action: &orchbee_workbench_protocol::WorkspaceGroupCloseAction,
    outcome: &orchbee_workbench_protocol::WorkspaceGroupCloseOutcome,
) -> bool {
    use orchbee_workbench_protocol::{
        WorkspaceGroupCloseAction as A, WorkspaceGroupCloseOutcome as O,
    };
    match (action, outcome) {
        (
            A::Start { project_id, .. },
            O::Available {
                project_id: actual_project,
                ..
            },
        ) => project_id == actual_project,
        (
            A::Resume {
                expected_attempt, ..
            },
            O::Available { attempt, .. },
        ) => expected_attempt.checked_add(1) == Some(*attempt),
        _ => true,
    }
}
