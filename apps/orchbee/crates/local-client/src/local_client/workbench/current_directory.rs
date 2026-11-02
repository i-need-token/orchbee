//! A bounded, read-only exchange; malformed correlation retires the connection.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    CurrentDirectoryRequest, CurrentDirectoryResult, WorkbenchRequest, WorkbenchResponse,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentDirectoryError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}

impl LocalClient {
    pub fn current_directory(
        &mut self,
        request: CurrentDirectoryRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<CurrentDirectoryResult, CurrentDirectoryError> {
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::CurrentDirectory(request.clone()),
        )
        .map_err(|_| CurrentDirectoryError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| CurrentDirectoryError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(CurrentDirectoryError::Unavailable)?;
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
            ExchangeAttempt::Result(WorkbenchResponse::CurrentDirectory(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => CurrentDirectoryError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => CurrentDirectoryError::Timeout,
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::WriteFailed(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                CurrentDirectoryError::Transport
            }
        };
        self.connection = None;
        Err(error)
    }
}
