//! One bounded `workbench.workspace-idle` read, for a command that asks whether closing a Workspace
//! would ask first; a malformed or uncorrelated answer retires the connection.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWireError, WorkspaceIdleRequest,
    WorkspaceIdleResult, read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceIdleReadError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}

impl LocalClient {
    /// Asks once, before `deadline`, whether closing a Workspace would ask first. Send it only to
    /// a Session whose handshake answer listed `workspace-idle`; the Server reads for up to four
    /// seconds, so `deadline` leaves it that long.
    pub fn workspace_idle_before(
        &mut self,
        request: WorkspaceIdleRequest,
        deadline: Instant,
    ) -> Result<WorkspaceIdleResult, WorkspaceIdleReadError> {
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::WorkspaceIdle(request.clone()),
        )
        .map_err(|_| WorkspaceIdleReadError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| WorkspaceIdleReadError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(WorkspaceIdleReadError::Unavailable)?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            None,
            &frame,
            |io| read_workbench_response(io),
        );
        let error = match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::WorkspaceIdle(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => WorkspaceIdleReadError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => WorkspaceIdleReadError::Timeout,
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                WorkspaceIdleReadError::Transport
            }
            ExchangeAttempt::WriteFailed(error) => {
                self.forget_connection_after_unknown(Some(&error));
                return Err(WorkspaceIdleReadError::Transport);
            }
        };
        self.connection = None;
        Err(error)
    }
}
