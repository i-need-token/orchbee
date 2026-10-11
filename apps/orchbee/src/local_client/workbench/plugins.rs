//! One exact request, with transport handoff retained for report uncertainty.
use super::metadata_exchange::{MetadataExchangeError, MetadataExchangeReason};
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    PluginRequest, PluginResult, WorkbenchRequest, WorkbenchResponse, read_workbench_response,
    write_workbench_request,
};
use std::time::Instant;
impl LocalClient {
    pub(crate) fn plugin(
        &mut self,
        request: PluginRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<PluginResult, MetadataExchangeError> {
        let before = |reason| MetadataExchangeError {
            submitted: false,
            reason,
        };
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &WorkbenchRequest::Plugin(request.clone()))
            .map_err(|_| before(MetadataExchangeReason::Encoding))?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(|_| before(MetadataExchangeReason::Deadline))?;
        let connection = self
            .connection
            .as_mut()
            .ok_or_else(|| before(MetadataExchangeReason::SessionUnavailable))?;
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
            ExchangeAttempt::Result(WorkbenchResponse::Plugin(result))
                if result.answers(&request) =>
            {
                return Ok(result);
            }
            ExchangeAttempt::Result(_) => MetadataExchangeReason::Correlation,
            ExchangeAttempt::NotSubmitted(_) => {
                return Err(before(MetadataExchangeReason::NotSubmitted));
            }
            ExchangeAttempt::WriteFailed(_) => MetadataExchangeReason::Write,
            ExchangeAttempt::ReadFailed(
                orchbee_workbench_protocol::WorkbenchWireError::Contract(_)
                | orchbee_workbench_protocol::WorkbenchWireError::InvalidHeader,
            ) => MetadataExchangeReason::Protocol,
            ExchangeAttempt::ReadFailed(orchbee_workbench_protocol::WorkbenchWireError::Io(
                error,
            )) if error.kind() == std::io::ErrorKind::TimedOut => {
                MetadataExchangeReason::ResponseDeadline
            }
            ExchangeAttempt::ReadFailed(_) => MetadataExchangeReason::Read,
            ExchangeAttempt::ReadDeadline(_) => MetadataExchangeReason::ResponseDeadline,
        };
        self.connection = None;
        Err(MetadataExchangeError {
            submitted: true,
            reason,
        })
    }
}
