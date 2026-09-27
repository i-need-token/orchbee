//! One exact request, with transport handoff retained for report uncertainty.
use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_local_ipc::LocalIo;
use orchbee_workbench_protocol::{
    WorkbenchRequest, WorkbenchResponse, WorkspaceMetadataRequest, WorkspaceMetadataResult,
    read_workbench_response, write_workbench_request,
};
use std::time::Instant;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MetadataExchangeReason {
    SessionUnavailable,
    Encoding,
    Deadline,
    NotSubmitted,
    Write,
    Read,
    ResponseDeadline,
    Correlation,
    Protocol,
}
impl MetadataExchangeReason {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SessionUnavailable => "session-unavailable",
            Self::Encoding => "request-encoding",
            Self::Deadline => "request-deadline",
            Self::NotSubmitted => "request-not-submitted",
            Self::Write => "request-write",
            Self::Read => "response-read",
            Self::ResponseDeadline => "response-deadline",
            Self::Correlation => "response-correlation",
            Self::Protocol => "response-protocol",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MetadataExchangeError {
    pub(crate) submitted: bool,
    pub(crate) reason: MetadataExchangeReason,
}
impl LocalClient {
    pub(crate) fn workspace_metadata(
        &mut self,
        request: WorkspaceMetadataRequest,
        deadline: Instant,
        wake: Option<LocalIo<'_>>,
    ) -> Result<WorkspaceMetadataResult, MetadataExchangeError> {
        let before = |reason| MetadataExchangeError {
            submitted: false,
            reason,
        };
        let mut frame = Vec::new();
        write_workbench_request(
            &mut frame,
            &WorkbenchRequest::WorkspaceMetadata(request.clone()),
        )
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
            ExchangeAttempt::Result(WorkbenchResponse::WorkspaceMetadata(result))
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
