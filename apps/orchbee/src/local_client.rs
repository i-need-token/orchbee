use std::{fmt, io, path::Path, time::Duration};

use orchbee_client_server_protocol::{
    ClientMessage, ProtocolError, ReadMessageError, RequestId, ServerMessage,
    TerminalCreateRequest, TerminalCreateResult, TerminalStartRequest, TerminalStartResult,
    WriteMessageError, read_server_message, write_client_message,
};
use orchbee_local_client_protocol::{ClientSessionConnection, ConnectClientError};

const REQUEST_IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(crate) struct LocalClient {
    connection: Option<ClientSessionConnection>,
}

impl LocalClient {
    pub(crate) fn connect(endpoint: &Path, cohort: &str) -> Result<Self, LocalClientConnectError> {
        Self::connect_with_request_timeout(endpoint, cohort, REQUEST_IO_TIMEOUT)
    }

    fn connect_with_request_timeout(
        endpoint: &Path,
        cohort: &str,
        request_timeout: Duration,
    ) -> Result<Self, LocalClientConnectError> {
        let connection = ClientSessionConnection::connect(endpoint, cohort)
            .map_err(LocalClientConnectError::Admission)?;
        connection
            .set_io_timeout(Some(request_timeout))
            .map_err(LocalClientConnectError::RequestTimeoutConfiguration)?;
        Ok(Self {
            connection: Some(connection),
        })
    }

    #[must_use]
    pub(crate) fn generation(&self) -> Option<std::num::NonZeroU64> {
        self.connection
            .as_ref()
            .map(ClientSessionConnection::generation)
    }

    pub(crate) fn submit_terminal_create(
        &mut self,
        request: TerminalCreateRequest,
    ) -> Result<TerminalCreateResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let result = self.exchange(ClientMessage::TerminalCreate(request), &request_id)?;
        let ServerMessage::TerminalCreateResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::TerminalCreate,
                    received: ResultKind::TerminalStart,
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn submit_terminal_start(
        &mut self,
        request: TerminalStartRequest,
    ) -> Result<TerminalStartResult, SubmissionError> {
        let request_id = request.request_id().clone();
        let result = self.exchange(ClientMessage::TerminalStart(request), &request_id)?;
        let ServerMessage::TerminalStartResult(result) = result else {
            return self.retire_with_unknown(
                request_id,
                OutcomeUnknownReason::UnexpectedResult {
                    expected: ResultKind::TerminalStart,
                    received: ResultKind::TerminalCreate,
                },
            );
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        Ok(result)
    }

    fn exchange(
        &mut self,
        request: ClientMessage,
        request_id: &RequestId,
    ) -> Result<ServerMessage, SubmissionError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(SubmissionError::SessionUnavailable);
        };

        match write_client_message(connection, &request) {
            Ok(()) => {}
            Err(WriteMessageError::Protocol(error)) => {
                return Err(SubmissionError::RequestEncoding(error));
            }
            Err(WriteMessageError::Io(error)) => {
                return self
                    .retire_with_unknown(request_id.clone(), OutcomeUnknownReason::Write(error));
            }
        }

        match read_server_message(connection) {
            Ok(result) => Ok(result),
            Err(ReadMessageError::Io(error)) => {
                self.retire_with_unknown(request_id.clone(), OutcomeUnknownReason::Read(error))
            }
            Err(ReadMessageError::Protocol(error)) => self.retire_with_unknown(
                request_id.clone(),
                OutcomeUnknownReason::InvalidResult(error),
            ),
        }
    }

    fn retire_with_unknown<Value>(
        &mut self,
        request_id: RequestId,
        reason: OutcomeUnknownReason,
    ) -> Result<Value, SubmissionError> {
        self.connection.take();
        Err(SubmissionError::OutcomeUnknown { request_id, reason })
    }
}

#[derive(Debug)]
pub(crate) enum LocalClientConnectError {
    Admission(ConnectClientError),
    RequestTimeoutConfiguration(io::Error),
}

impl fmt::Display for LocalClientConnectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(formatter, "local Client admission failed: {error}"),
            Self::RequestTimeoutConfiguration(error) => {
                write!(
                    formatter,
                    "local Client request timeout setup failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for LocalClientConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::RequestTimeoutConfiguration(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum SubmissionError {
    SessionUnavailable,
    RequestEncoding(ProtocolError),
    OutcomeUnknown {
        request_id: RequestId,
        reason: OutcomeUnknownReason,
    },
}

impl fmt::Display for SubmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => formatter.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBCS request encoding failed before submission: {error}"
                )
            }
            Self::OutcomeUnknown { request_id, reason } => {
                write!(
                    formatter,
                    "OBCS request {request_id} outcome is unknown: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for SubmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::OutcomeUnknown { reason, .. } => reason.source(),
            Self::SessionUnavailable => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum OutcomeUnknownReason {
    Write(io::Error),
    Read(io::Error),
    InvalidResult(ProtocolError),
    UnexpectedResult {
        expected: ResultKind,
        received: ResultKind,
    },
    CorrelationMismatch {
        expected: RequestId,
        received: RequestId,
    },
}

impl fmt::Display for OutcomeUnknownReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Read(error) => write!(formatter, "result read failed: {error}"),
            Self::InvalidResult(error) => write!(formatter, "invalid Server result: {error}"),
            Self::UnexpectedResult { expected, received } => {
                write!(formatter, "expected {expected} but received {received}")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for OutcomeUnknownReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::Read(error) => Some(error),
            Self::InvalidResult(error) => Some(error),
            Self::UnexpectedResult { .. } | Self::CorrelationMismatch { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultKind {
    TerminalCreate,
    TerminalStart,
}

impl fmt::Display for ResultKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TerminalCreate => "terminal.create.result",
            Self::TerminalStart => "terminal.start.result",
        })
    }
}

#[cfg(test)]
mod tests;
