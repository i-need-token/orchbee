//! One exchange, exact correlation, and retirement on an unknown transport outcome.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewOutcome, WorkbenchClientViewRequest, WorkbenchClientViewResult,
    WorkbenchRequest, WorkbenchResponse, WorkbenchWireError, read_workbench_response,
    write_workbench_request,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientViewExchangeError {
    Unavailable,
    Encoding,
    Protocol,
    Transport,
    Timeout,
}
impl LocalClient {
    pub fn exchange_client_view(
        &mut self,
        request: WorkbenchClientViewRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkbenchClientViewResult, ClientViewExchangeError> {
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::ClientView(request.clone()))
            .map_err(|_| ClientViewExchangeError::Encoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| ClientViewExchangeError::Timeout)?;
        let connection = self
            .connection
            .as_mut()
            .ok_or(ClientViewExchangeError::Unavailable)?;
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let generation = connection.reply_generation();
        let error = match attempt {
            ExchangeAttempt::Result(WorkbenchResponse::ClientView(result))
                if generation
                    .is_some_and(|generation| correlated(&request, &result, generation)) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_)
            | ExchangeAttempt::ReadFailed(
                WorkbenchWireError::InvalidHeader | WorkbenchWireError::Contract(_),
            ) => ClientViewExchangeError::Protocol,
            ExchangeAttempt::ReadDeadline(_) => ClientViewExchangeError::Timeout,
            ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(ref error))
            | ExchangeAttempt::WriteFailed(ref error)
                if error.kind() == std::io::ErrorKind::TimedOut =>
            {
                ClientViewExchangeError::Timeout
            }
            ExchangeAttempt::NotSubmitted(_)
            | ExchangeAttempt::WriteFailed(_)
            | ExchangeAttempt::ReadFailed(WorkbenchWireError::Io(_)) => {
                ClientViewExchangeError::Transport
            }
        };
        self.connection = None;
        Err(error)
    }
}

fn correlated(
    request: &WorkbenchClientViewRequest,
    result: &WorkbenchClientViewResult,
    generation: std::num::NonZeroU64,
) -> bool {
    if result.request_id != request.request_id
        || result.session_id != request.session_id
        || result.protocol_version != request.protocol_version
    {
        return false;
    }
    request.action.admits(&result.outcome)
        && match (&request.action, &result.outcome) {
            (
                ClientViewAction::Synchronize { .. },
                ClientViewOutcome::Synchronized { client, .. },
            ) => client.client_id.as_str() == format!("{:032x}", generation.get()),
            // Connection generation is echoed only by Synchronize; pairing belongs to the protocol.
            _ => true,
        }
}
