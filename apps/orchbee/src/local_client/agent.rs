//! The Agent observation read exchange on one admitted Client Session: a
//! single OBAP request/result pair under the caller's deadline, with exact
//! requestId+operation correlation and no replay after ambiguous outcomes.

use std::{fmt, io, os::fd::BorrowedFd, time::Instant};

use orchbee_agent_observation_protocol::{
    AgentReadRequest, AgentReadResult, AgentRequest, AgentResult, AgentWireError, ObapRequestId,
    write_agent_request,
};

use super::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    /// Submit one Agent read request and return the correlated typed result.
    /// Any post-submission ambiguity retires the session — the caller must
    /// not replay a request whose outcome is unknown. `wake` is the signal
    /// owner's borrowed descriptor joined into every exchange wait set: the
    /// transport only polls it and surfaces provisional wake markers inside
    /// the returned `io::Error`s; the caller performs the sole arbitration
    /// read.
    pub(crate) fn submit_agent_read_before_cancellable(
        &mut self,
        request: AgentReadRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<AgentReadResult, AgentReadError> {
        if self.connection.is_none() {
            return Err(AgentReadError::SessionUnavailable);
        }

        let request_id = request.request_id().clone();
        let operation = AgentReadOperation::of(&request);
        let mut frame = Vec::new();
        write_agent_request(&mut frame, &AgentRequest::Read(request))
            .map_err(AgentReadError::RequestEncoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(AgentReadError::RequestNotSubmitted)?;
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(AgentReadError::SessionUnavailable);
            };
            exchange_once(
                connection,
                &self.clock,
                deadline,
                self.io_chunk_limits,
                wake,
                &frame,
                |io| orchbee_agent_observation_protocol::read_agent_result(io),
            )
        };
        let result = match attempt {
            ExchangeAttempt::Result(result) => result,
            ExchangeAttempt::NotSubmitted(error) => {
                return Err(AgentReadError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_agent_failure(request_id, AgentReadFailure::Write(error));
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self
                    .retire_with_agent_failure(request_id, AgentReadFailure::Response(error));
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_agent_failure(
                    request_id,
                    AgentReadFailure::ResponseDeadline(error),
                );
            }
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_agent_failure(
                request_id.clone(),
                AgentReadFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        let AgentResult::Read(result) = result else {
            return self
                .retire_with_agent_failure(request_id, AgentReadFailure::UnexpectedResponse);
        };
        if AgentReadOperation::of_result(&result) != operation {
            return self
                .retire_with_agent_failure(request_id, AgentReadFailure::UnexpectedResponse);
        }
        Ok(result)
    }

    fn retire_with_agent_failure<Value>(
        &mut self,
        request_id: ObapRequestId,
        reason: AgentReadFailure,
    ) -> Result<Value, AgentReadError> {
        self.connection.take();
        Err(AgentReadError::Failed { request_id, reason })
    }
}

/// The OBAP operation a request or result document carries — plus `Wait`,
/// the CLI-side composite that drives `Get` polls on one held session.
/// `of`/`of_result` only ever produce the three wire operations; `Wait`
/// exists so failure details name the command the poll belonged to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AgentReadOperation {
    List,
    Get,
    Explain,
    Wait,
}

impl AgentReadOperation {
    pub(crate) fn of(request: &AgentReadRequest) -> Self {
        match request {
            AgentReadRequest::List { .. } => Self::List,
            AgentReadRequest::Get { .. } => Self::Get,
            AgentReadRequest::Explain { .. } => Self::Explain,
        }
    }

    fn of_result(result: &AgentReadResult) -> Self {
        match result {
            AgentReadResult::List { .. } => Self::List,
            AgentReadResult::Get { .. } => Self::Get,
            AgentReadResult::Explain { .. } => Self::Explain,
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Get => "get",
            Self::Explain => "explain",
            Self::Wait => "wait",
        }
    }
}

#[derive(Debug)]
pub(crate) enum AgentReadError {
    SessionUnavailable,
    RequestEncoding(AgentWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: ObapRequestId,
        reason: AgentReadFailure,
    },
}

impl fmt::Display for AgentReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => formatter.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(
                    formatter,
                    "OBAP request encoding failed before submission: {error}"
                )
            }
            Self::RequestNotSubmitted(error) => write!(
                formatter,
                "OBAP request failed before any transport handoff: {error}"
            ),
            Self::Failed { request_id, reason } => {
                write!(formatter, "OBAP request {request_id} failed: {reason}")
            }
        }
    }
}

impl std::error::Error for AgentReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::Failed { reason, .. } => reason.source(),
            Self::SessionUnavailable => None,
        }
    }
}

/// Why a submitted OBAP read could not produce a trustworthy typed result;
/// every variant means the exchange outcome is unknown and the session is
/// retired.
#[derive(Debug)]
pub(crate) enum AgentReadFailure {
    Write(io::Error),
    Response(AgentWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: ObapRequestId,
        received: ObapRequestId,
    },
}

impl fmt::Display for AgentReadFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => write!(formatter, "invalid read result: {error}"),
            Self::ResponseDeadline(error) => {
                write!(formatter, "read response failed: {error}")
            }
            Self::UnexpectedResponse => {
                formatter.write_str("response kind does not match the request operation")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                formatter,
                "expected OBAP request identity {expected} but received {received}"
            ),
        }
    }
}

impl std::error::Error for AgentReadFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}
