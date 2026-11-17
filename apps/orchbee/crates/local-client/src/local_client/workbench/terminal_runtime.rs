//! One bounded `workbench.terminal-runtime` read outside a stream, for a command that reports the
//! Session's state once; a malformed or uncorrelated answer retires the connection.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    TerminalRuntimeRequest, TerminalRuntimeResult, WorkbenchRequest, WorkbenchResponse,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalRuntimeReadError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}

impl LocalClient {
    /// Asks for the Session's terminal-runtime state once, before `deadline`. Send it only to a
    /// Session whose handshake answer listed `terminal-runtime`.
    pub fn terminal_runtime_before(
        &mut self,
        request: TerminalRuntimeRequest,
        deadline: Instant,
    ) -> Result<TerminalRuntimeResult, TerminalRuntimeReadError> {
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::TerminalRuntime(request.clone()),
        )
        .map_err(|_| TerminalRuntimeReadError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| TerminalRuntimeReadError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(TerminalRuntimeReadError::Unavailable)?;
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
            ExchangeAttempt::Result(WorkbenchResponse::TerminalRuntime(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => TerminalRuntimeReadError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => TerminalRuntimeReadError::Timeout,
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::WriteFailed(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                TerminalRuntimeReadError::Transport
            }
        };
        self.connection = None;
        Err(error)
    }
}
