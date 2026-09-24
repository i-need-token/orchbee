//! A bounded, read-only exchange; malformed correlation retires the connection.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkbenchWireError, WorkbenchWorktreeQuery,
    WorkbenchWorktreeQueryOutcome, WorkbenchWorktreeQueryResult, WorktreeSource,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeQueryError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}

impl LocalClient {
    pub(crate) fn query_worktrees(
        &mut self,
        request: WorkbenchWorktreeQuery,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkbenchWorktreeQueryResult, WorktreeQueryError> {
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::WorktreeQuery(request.clone()),
        )
        .map_err(|_| WorktreeQueryError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| WorktreeQueryError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(WorktreeQueryError::Unavailable)?;
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
            ExchangeAttempt::Result(WorkbenchResponse::WorktreeQuery(result))
                if correlated(&request, &result) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => WorktreeQueryError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => WorktreeQueryError::Timeout,
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::WriteFailed(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                WorktreeQueryError::Transport
            }
        };
        self.connection = None;
        Err(error)
    }
}

fn correlated(request: &WorkbenchWorktreeQuery, result: &WorkbenchWorktreeQueryResult) -> bool {
    if result.request_id != request.request_id
        || result.protocol_version != request.protocol_version
    {
        return false;
    }
    match &result.outcome {
        WorkbenchWorktreeQueryOutcome::Found {
            site_id, source, ..
        } => {
            *site_id == request.site_id
                && match &request.source {
                    WorktreeSource::Workspace { workspace_id } => {
                        source.workspace_id.as_ref() == Some(workspace_id)
                    }
                    WorktreeSource::Directory { path } => {
                        source.workspace_id.is_none() && source.requested_path == *path
                    }
                }
        }
        WorkbenchWorktreeQueryOutcome::Failed { .. } => true,
    }
}
