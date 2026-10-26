//! The Agent observation exchange on one admitted Client Connection: a
//! single OBAP request/result pair under the caller's deadline, with exact
//! requestId+operation correlation and no replay after ambiguous outcomes.

use orchbee_local_ipc::LocalIo as BorrowedFd;
use std::{fmt, io, time::Instant};

use orchbee_agent_observation_protocol::{
    AgentNamesRequest, AgentNamesResult, AgentReadRequest, AgentReadResult, AgentRequest,
    AgentResult, AgentWireError, ObapRequestId, write_agent_request,
};

use orchbee_agent_observation_protocol::{AgentLaunchRequest, AgentLaunchResult};

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
    ) -> Result<AgentReadResult, AgentExchangeError> {
        let request_id = request.request_id().clone();
        match self.submit_agent_before_cancellable(AgentRequest::Read(request), deadline, wake)? {
            AgentResult::Read(result) => Ok(result),
            AgentResult::Names(_) | AgentResult::Launch(_) | AgentResult::Changes(_) => {
                self.retire_with_agent_failure(request_id, AgentExchangeFailure::UnexpectedResponse)
            }
        }
    }

    /// Names mutations share the same one-shot exchange. A lost reply is
    /// ambiguous and retires the session; this method never replays it.
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    pub(crate) fn submit_agent_names_before_cancellable(
        &mut self,
        request: AgentNamesRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<AgentNamesResult, AgentExchangeError> {
        let request_id = request.request_id().clone();
        match self.submit_agent_before_cancellable(AgentRequest::Names(request), deadline, wake)? {
            AgentResult::Names(result) => Ok(result),
            AgentResult::Read(_) | AgentResult::Launch(_) | AgentResult::Changes(_) => {
                self.retire_with_agent_failure(request_id, AgentExchangeFailure::UnexpectedResponse)
            }
        }
    }

    /// One correlated launch exchange; mutations are never replayed after
    /// an ambiguous transport outcome.
    #[cfg(windows)]
    pub(crate) fn submit_agent_launch_before_cancellable(
        &mut self,
        request: AgentLaunchRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<AgentLaunchResult, AgentExchangeError> {
        let request_id = request.request_id().clone();
        match self.submit_agent_before_cancellable(AgentRequest::Launch(request), deadline, wake)? {
            AgentResult::Launch(result) => Ok(result),
            AgentResult::Read(_) | AgentResult::Names(_) | AgentResult::Changes(_) => {
                self.retire_with_agent_failure(request_id, AgentExchangeFailure::UnexpectedResponse)
            }
        }
    }

    pub(crate) fn submit_agent_changes_before_cancellable(
        &mut self,
        request: orchbee_agent_observation_protocol::AgentChangesRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<orchbee_agent_observation_protocol::AgentChangesResult, AgentExchangeError> {
        use orchbee_agent_observation_protocol::{
            AgentChangesOutcome, AgentChangesRequest, AgentChangesResult,
        };
        let AgentChangesRequest::Read {
            request_id,
            limit,
            cursor,
        } = &request;
        let (request_id, limit, checkpoint) = (request_id.clone(), limit.value(), cursor.is_none());
        match self.submit_agent_before_cancellable(
            AgentRequest::Changes(request),
            deadline,
            wake,
        )? {
            AgentResult::Changes(result) => {
                let AgentChangesResult::Read {
                    result: envelope, ..
                } = &result;
                let valid = match &envelope.outcome {
                    AgentChangesOutcome::Changes { changes, .. } => {
                        !checkpoint && changes.len() as u64 <= limit
                    }
                    AgentChangesOutcome::ResetRequired { .. } => !checkpoint,
                    _ => true,
                };
                if valid {
                    Ok(result)
                } else {
                    self.retire_with_agent_failure(
                        request_id,
                        AgentExchangeFailure::UnexpectedResponse,
                    )
                }
            }
            _ => {
                self.retire_with_agent_failure(request_id, AgentExchangeFailure::UnexpectedResponse)
            }
        }
    }

    fn submit_agent_before_cancellable(
        &mut self,
        request: AgentRequest,
        deadline: Instant,
        wake: Option<BorrowedFd<'_>>,
    ) -> Result<AgentResult, AgentExchangeError> {
        if self.connection.is_none() {
            return Err(AgentExchangeError::ConnectionUnavailable);
        }

        let request_id = request.request_id().clone();
        let operation = operation_of_request(&request);
        let mut frame = Vec::new();
        write_agent_request(&mut frame, &request).map_err(AgentExchangeError::RequestEncoding)?;
        let deadline = self
            .submission_deadline_before(deadline)
            .map_err(AgentExchangeError::RequestNotSubmitted)?;
        let attempt = {
            let Some(connection) = self.connection.as_mut() else {
                return Err(AgentExchangeError::ConnectionUnavailable);
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
                return Err(AgentExchangeError::RequestNotSubmitted(error));
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self
                    .retire_with_agent_failure(request_id, AgentExchangeFailure::Write(error));
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self
                    .retire_with_agent_failure(request_id, AgentExchangeFailure::Response(error));
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_agent_failure(
                    request_id,
                    AgentExchangeFailure::ResponseDeadline(error),
                );
            }
        };
        if result.request_id() != &request_id {
            let received = result.request_id().clone();
            return self.retire_with_agent_failure(
                request_id.clone(),
                AgentExchangeFailure::CorrelationMismatch {
                    expected: request_id,
                    received,
                },
            );
        }
        if operation_of_result(&result) != operation {
            return self
                .retire_with_agent_failure(request_id, AgentExchangeFailure::UnexpectedResponse);
        }
        Ok(result)
    }

    fn retire_with_agent_failure<Value>(
        &mut self,
        request_id: ObapRequestId,
        reason: AgentExchangeFailure,
    ) -> Result<Value, AgentExchangeError> {
        self.connection.take();
        Err(AgentExchangeError::Failed { request_id, reason })
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
    Changes,
    Management,
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
            Self::Changes => "changes",
            Self::Management => "management",
        }
    }
}

#[derive(Debug)]
pub(crate) enum AgentExchangeError {
    ConnectionUnavailable,
    RequestEncoding(AgentWireError),
    RequestNotSubmitted(io::Error),
    Failed {
        request_id: ObapRequestId,
        reason: AgentExchangeFailure,
    },
}

impl fmt::Display for AgentExchangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => {
                formatter.write_str("local Client Connection is unavailable")
            }
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

impl std::error::Error for AgentExchangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
            Self::Failed { reason, .. } => reason.source(),
            Self::ConnectionUnavailable => None,
        }
    }
}

/// Why a submitted OBAP request could not produce a trustworthy typed result;
/// every variant means the exchange outcome is unknown and the session is
/// retired.
#[derive(Debug)]
pub(crate) enum AgentExchangeFailure {
    Write(io::Error),
    Response(AgentWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: ObapRequestId,
        received: ObapRequestId,
    },
}

impl fmt::Display for AgentExchangeFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "request write failed: {error}"),
            Self::Response(error) => write!(formatter, "invalid Agent result: {error}"),
            Self::ResponseDeadline(error) => {
                write!(formatter, "Agent response failed: {error}")
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

impl std::error::Error for AgentExchangeFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse | Self::CorrelationMismatch { .. } => None,
        }
    }
}

/// Family plus operation, independent of request identity and outcome.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Operation {
    Read(AgentReadOperation),
    Resolve,
    NamesList,
    NameAssign,
    NameRelease,
    LaunchStatus,
    LaunchTerminalStatus,
    LaunchReserve,
    LaunchInspect,
    LaunchComplete,
    LaunchCancel,
    Changes,
}

fn operation_of_request(request: &AgentRequest) -> Operation {
    match request {
        AgentRequest::Changes(_) => Operation::Changes,
        AgentRequest::Launch(AgentLaunchRequest::TerminalStatus { .. }) => {
            Operation::LaunchTerminalStatus
        }
        AgentRequest::Launch(AgentLaunchRequest::Status { .. }) => Operation::LaunchStatus,
        AgentRequest::Launch(AgentLaunchRequest::Reserve { .. }) => Operation::LaunchReserve,
        AgentRequest::Launch(AgentLaunchRequest::Inspect { .. }) => Operation::LaunchInspect,
        AgentRequest::Launch(AgentLaunchRequest::Complete { .. }) => Operation::LaunchComplete,
        AgentRequest::Launch(AgentLaunchRequest::Cancel { .. }) => Operation::LaunchCancel,
        AgentRequest::Read(request) => Operation::Read(AgentReadOperation::of(request)),
        AgentRequest::Names(AgentNamesRequest::Resolve { .. }) => Operation::Resolve,
        AgentRequest::Names(AgentNamesRequest::NamesList { .. }) => Operation::NamesList,
        AgentRequest::Names(AgentNamesRequest::NameAssign { .. }) => Operation::NameAssign,
        AgentRequest::Names(AgentNamesRequest::NameRelease { .. }) => Operation::NameRelease,
    }
}

fn operation_of_result(result: &AgentResult) -> Operation {
    match result {
        AgentResult::Changes(_) => Operation::Changes,
        AgentResult::Launch(AgentLaunchResult::TerminalStatus { .. }) => {
            Operation::LaunchTerminalStatus
        }
        AgentResult::Launch(AgentLaunchResult::Status { .. }) => Operation::LaunchStatus,
        AgentResult::Launch(AgentLaunchResult::Reserve { .. }) => Operation::LaunchReserve,
        AgentResult::Launch(AgentLaunchResult::Inspect { .. }) => Operation::LaunchInspect,
        AgentResult::Launch(AgentLaunchResult::Complete { .. }) => Operation::LaunchComplete,
        AgentResult::Launch(AgentLaunchResult::Cancel { .. }) => Operation::LaunchCancel,
        AgentResult::Read(result) => Operation::Read(AgentReadOperation::of_result(result)),
        AgentResult::Names(AgentNamesResult::Resolve { .. }) => Operation::Resolve,
        AgentResult::Names(AgentNamesResult::NamesList { .. }) => Operation::NamesList,
        AgentResult::Names(AgentNamesResult::NameAssign { .. }) => Operation::NameAssign,
        AgentResult::Names(AgentNamesResult::NameRelease { .. }) => Operation::NameRelease,
    }
}
