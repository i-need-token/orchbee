//! A bounded, read-only exchange; malformed correlation retires the connection.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWireError, WorkspaceGitRequest,
    WorkspaceGitResult, read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceGitError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}

impl LocalClient {
    pub(crate) fn workspace_git(
        &mut self,
        request: WorkspaceGitRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkspaceGitResult, WorkspaceGitError> {
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::WorkspaceGit(request.clone()))
            .map_err(|_| WorkspaceGitError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| WorkspaceGitError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(WorkspaceGitError::Unavailable)?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let error = match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::WorkspaceGit(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => WorkspaceGitError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => WorkspaceGitError::Timeout,
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::WriteFailed(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                WorkspaceGitError::Transport
            }
        };
        self.connection = None;
        Err(error)
    }
}
