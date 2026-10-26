//! One correlated Worktree exchange. Only caller-driven reads follow an accepted operation.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWorktreeRequest, WorkbenchWorktreeResult,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorktreeExchangeError {
    /// False proves no transport handoff; true means the request may have been accepted.
    pub submitted: bool,
    pub reason: &'static str,
}
impl LocalClient {
    pub fn submit_worktree_operation(
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
                    && request.action.admits(&result.outcome) =>
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
