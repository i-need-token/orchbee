use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandUnavailableReason, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWireError, read_workbench_response,
    write_workbench_request,
};
use std::{fmt, io, time::Instant};

mod read;
mod resume;
pub(crate) use read::WorkbenchTabCloseReadError;
pub(crate) use resume::WorkbenchTabCloseResumeSubmission;

#[derive(Debug)]
pub(crate) struct WorkbenchTabCloseRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for WorkbenchTabCloseRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::TabClose { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchTabCloseAccepted {
    pub(crate) close_request_id: WorkbenchRequestId,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) accepted_revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchTabClosePrecommit {
    Denied,
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseSubmission {
    DefinitelyNotSubmitted(WorkbenchTabCloseNotSubmitted),
    DefinitePrecommit(WorkbenchTabClosePrecommit),
    Accepted(WorkbenchTabCloseAccepted),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabCloseFailure,
    },
}

impl LocalClient {
    pub(crate) fn submit_workbench_tab_close(
        &mut self,
        request: WorkbenchTabCloseRequest,
        deadline: Instant,
    ) -> WorkbenchTabCloseSubmission {
        self.submit_workbench_tab_close_with_wake(request, deadline, None)
    }

    /// Submit once with caller cancellation, preserving definite rejection and uncertain handoff.
    pub(crate) fn submit_workbench_tab_close_with_wake(
        &mut self,
        request: WorkbenchTabCloseRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchTabCloseSubmission {
        let command = request.0;
        let response = match self.exchange_tab_close_with_wake(
            WorkbenchRequest::Command(command.clone()),
            deadline,
            wake,
        ) {
            Ok(response) => response,
            Err(CloseExchangeError::NotSubmitted(reason)) => {
                return WorkbenchTabCloseSubmission::DefinitelyNotSubmitted(reason);
            }
            Err(CloseExchangeError::Failed(reason)) => {
                return WorkbenchTabCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                };
            }
        };
        match classify_command_result(&command, response) {
            Ok(result) => result,
            Err(reason) => {
                self.connection.take();
                WorkbenchTabCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                }
            }
        }
    }

    // Only the three Close-family adapters use this exchange; classification stays with each action.
    fn exchange_tab_close(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
    ) -> Result<WorkbenchResponse, CloseExchangeError> {
        self.exchange_tab_close_with_wake(request, deadline, None)
    }

    fn exchange_tab_close_with_wake(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> Result<WorkbenchResponse, CloseExchangeError> {
        if self.connection.is_none() {
            return Err(CloseExchangeError::NotSubmitted(
                WorkbenchTabCloseNotSubmitted::ConnectionUnavailable,
            ));
        }
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &request).map_err(|error| {
            CloseExchangeError::NotSubmitted(WorkbenchTabCloseNotSubmitted::RequestEncoding(error))
        })?;
        let deadline = self.submission_deadline_before(deadline).map_err(|error| {
            CloseExchangeError::NotSubmitted(WorkbenchTabCloseNotSubmitted::RequestNotSubmitted(
                error,
            ))
        })?;
        let Some(connection) = self.connection.as_mut() else {
            return Err(CloseExchangeError::NotSubmitted(
                WorkbenchTabCloseNotSubmitted::ConnectionUnavailable,
            ));
        };
        let failure = match exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        ) {
            ExchangeAttempt::Result(response) => return Ok(response),
            ExchangeAttempt::NotSubmitted(error) => {
                return Err(CloseExchangeError::NotSubmitted(
                    WorkbenchTabCloseNotSubmitted::RequestNotSubmitted(error),
                ));
            }
            ExchangeAttempt::WriteFailed(error) => WorkbenchTabCloseFailure::Write(error),
            ExchangeAttempt::ReadFailed(error) => WorkbenchTabCloseFailure::Response(error),
            ExchangeAttempt::ReadDeadline(error) => {
                WorkbenchTabCloseFailure::ResponseDeadline(error)
            }
        };
        self.connection.take();
        Err(CloseExchangeError::Failed(failure))
    }
}

fn classify_command_result(
    command: &WorkbenchCommand,
    response: WorkbenchResponse,
) -> Result<WorkbenchTabCloseSubmission, WorkbenchTabCloseFailure> {
    let WorkbenchResponse::Command(result) = response else {
        return Err(WorkbenchTabCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&command.request_id, &result.request_id)?;
    let WorkbenchAction::TabClose {
        terminal_context_id: expected_context,
        tab_id: expected_tab,
    } = &command.action
    else {
        unreachable!("opaque request admits only tab.close")
    };
    match result.outcome {
        WorkbenchCommandOutcome::TabCloseAccepted {
            close_request_id,
            terminal_context_id,
            tab_id,
            accepted_revision,
        } => {
            validate_target(
                expected_context,
                expected_tab,
                &terminal_context_id,
                &tab_id,
            )?;
            if accepted_revision.value() == u64::MAX
                || accepted_revision > command.expected_revision
                || (close_request_id == command.request_id
                    && accepted_revision != command.expected_revision)
            {
                return Err(WorkbenchTabCloseFailure::AcceptedRevisionMismatch {
                    admitted: command.expected_revision,
                    received: accepted_revision,
                });
            }
            Ok(WorkbenchTabCloseSubmission::Accepted(
                WorkbenchTabCloseAccepted {
                    close_request_id,
                    terminal_context_id,
                    tab_id,
                    accepted_revision,
                },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchTabCloseSubmission::DefinitePrecommit(
            WorkbenchTabClosePrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SiteMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::TerminalContextUnavailable
                    | WorkbenchRejectionReason::TabUnavailable
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            Ok(WorkbenchTabCloseSubmission::DefinitePrecommit(
                WorkbenchTabClosePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::TerminalBusy
                    | WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable
                    | WorkbenchCommandUnavailableReason::ServerBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchTabCloseSubmission::DefinitePrecommit(
                WorkbenchTabClosePrecommit::Unavailable { reason },
            ))
        }
        _ => Err(WorkbenchTabCloseFailure::ActionIncompatible),
    }
}

fn validate_correlation(
    expected: &WorkbenchRequestId,
    received: &WorkbenchRequestId,
) -> Result<(), WorkbenchTabCloseFailure> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchTabCloseFailure::CorrelationMismatch {
            expected: expected.clone(),
            received: received.clone(),
        })
    }
}

fn validate_target(
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    received_context: &WorkbenchTerminalContextId,
    received_tab: &WorkbenchTabId,
) -> Result<(), WorkbenchTabCloseFailure> {
    if expected_context != received_context {
        return Err(WorkbenchTabCloseFailure::TerminalContextMismatch {
            expected: expected_context.clone(),
            received: received_context.clone(),
        });
    }
    if expected_tab != received_tab {
        return Err(WorkbenchTabCloseFailure::TabMismatch {
            expected: expected_tab.clone(),
            received: received_tab.clone(),
        });
    }
    Ok(())
}

enum CloseExchangeError {
    NotSubmitted(WorkbenchTabCloseNotSubmitted),
    Failed(WorkbenchTabCloseFailure),
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}
impl fmt::Display for WorkbenchTabCloseNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP Close request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP Close request failed before any transport handoff: {error}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchTabCloseNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabCloseFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    ActionIncompatible,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
    SiteMismatch {
        expected: SiteId,
        received: SiteId,
    },
    TerminalContextMismatch {
        expected: WorkbenchTerminalContextId,
        received: WorkbenchTerminalContextId,
    },
    TabMismatch {
        expected: WorkbenchTabId,
        received: WorkbenchTabId,
    },
    OperationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
    AcceptedRevisionMismatch {
        admitted: WorkbenchRevision,
        received: WorkbenchRevision,
    },
    AttemptGenerationMismatch {
        expected: WorkbenchTabCloseAttemptGeneration,
        received: WorkbenchTabCloseAttemptGeneration,
    },
}
impl fmt::Display for WorkbenchTabCloseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid Close response: {error}"),
            Self::ResponseDeadline(error) => write!(f, "Close response failed: {error}"),
            Self::UnexpectedResponse => f.write_str("received the wrong Close response envelope"),
            Self::ActionIncompatible => {
                f.write_str("received an action-incompatible tab.close outcome")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request {expected} but received {received}"
            ),
            Self::SiteMismatch { expected, received } => {
                write!(f, "expected Site {expected} but received {received}")
            }
            Self::TerminalContextMismatch { expected, received } => write!(
                f,
                "expected terminal context {expected} but received {received}"
            ),
            Self::TabMismatch { expected, received } => {
                write!(f, "expected Tab {expected} but received {received}")
            }
            Self::OperationMismatch { expected, received } => write!(
                f,
                "expected canonical Close request {expected} but received {received}"
            ),
            Self::AcceptedRevisionMismatch { admitted, received } => write!(
                f,
                "Close accepted revision {received} is incompatible with admitted base {admitted}"
            ),
            Self::AttemptGenerationMismatch { expected, received } => write!(
                f,
                "Resume generation {received} is incompatible with submitted generation {expected}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchTabCloseFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::ActionIncompatible
            | Self::CorrelationMismatch { .. }
            | Self::SiteMismatch { .. }
            | Self::TerminalContextMismatch { .. }
            | Self::TabMismatch { .. }
            | Self::OperationMismatch { .. }
            | Self::AcceptedRevisionMismatch { .. }
            | Self::AttemptGenerationMismatch { .. } => None,
        }
    }
}
