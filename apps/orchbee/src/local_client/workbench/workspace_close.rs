use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    SiteId, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandUnavailableReason, WorkbenchRejectionReason, WorkbenchRequest,
    WorkbenchRequestId, WorkbenchResponse, WorkbenchRevision, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTerminalContextId, WorkbenchWireError, WorkbenchWorkspaceId, read_workbench_response,
    write_workbench_request,
};
use std::{fmt, io, time::Instant};

#[cfg(test)]
mod idle_tests;
mod read;
mod resume;
#[cfg(any(target_os = "linux", windows))]
pub(crate) use read::WorkbenchWorkspaceCloseReadError;
#[cfg(any(target_os = "linux", windows))]
pub(crate) use resume::WorkbenchWorkspaceCloseResumeSubmission;

#[derive(Debug)]
pub(crate) struct WorkbenchWorkspaceCloseRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for WorkbenchWorkspaceCloseRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::WorkspaceClose { .. } | WorkbenchAction::WorkspaceCloseIfIdle { .. }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchWorkspaceCloseAccepted {
    pub(crate) close_request_id: WorkbenchRequestId,
    pub(crate) workspace_id: WorkbenchWorkspaceId,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) accepted_revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchWorkspaceClosePrecommit {
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
pub(crate) enum WorkbenchWorkspaceCloseSubmission {
    DefinitelyNotSubmitted(WorkbenchWorkspaceCloseNotSubmitted),
    DefinitePrecommit(WorkbenchWorkspaceClosePrecommit),
    Accepted(WorkbenchWorkspaceCloseAccepted),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchWorkspaceCloseFailure,
    },
}

impl LocalClient {
    pub(crate) fn submit_workbench_workspace_close(
        &mut self,
        request: WorkbenchWorkspaceCloseRequest,
        deadline: Instant,
    ) -> WorkbenchWorkspaceCloseSubmission {
        let command = request.0;
        let response = match self
            .exchange_workspace_close(WorkbenchRequest::Command(command.clone()), deadline)
        {
            Ok(response) => response,
            Err(WorkspaceCloseExchangeError::NotSubmitted(reason)) => {
                return WorkbenchWorkspaceCloseSubmission::DefinitelyNotSubmitted(reason);
            }
            Err(WorkspaceCloseExchangeError::Failed(reason)) => {
                return WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                };
            }
        };
        match classify_command_result(&command, response) {
            Ok(result) => result,
            Err(reason) => {
                self.connection.take();
                WorkbenchWorkspaceCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                }
            }
        }
    }

    // Only the three Workspace-Close adapters use this exchange; classification stays with each request kind.
    fn exchange_workspace_close(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
    ) -> Result<WorkbenchResponse, WorkspaceCloseExchangeError> {
        if self.connection.is_none() {
            return Err(WorkspaceCloseExchangeError::NotSubmitted(
                WorkbenchWorkspaceCloseNotSubmitted::ConnectionUnavailable,
            ));
        }
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &request).map_err(|error| {
            WorkspaceCloseExchangeError::NotSubmitted(
                WorkbenchWorkspaceCloseNotSubmitted::RequestEncoding(error),
            )
        })?;
        let deadline = self.submission_deadline_before(deadline).map_err(|error| {
            WorkspaceCloseExchangeError::NotSubmitted(
                WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(error),
            )
        })?;
        let Some(connection) = self.connection.as_mut() else {
            return Err(WorkspaceCloseExchangeError::NotSubmitted(
                WorkbenchWorkspaceCloseNotSubmitted::ConnectionUnavailable,
            ));
        };
        let failure = match exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            None,
            &frame,
            |io| read_workbench_response(io),
        ) {
            ExchangeAttempt::Result(response) => return Ok(response),
            ExchangeAttempt::NotSubmitted(error) => {
                return Err(WorkspaceCloseExchangeError::NotSubmitted(
                    WorkbenchWorkspaceCloseNotSubmitted::RequestNotSubmitted(error),
                ));
            }
            ExchangeAttempt::WriteFailed(error) => WorkbenchWorkspaceCloseFailure::Write(error),
            ExchangeAttempt::ReadFailed(error) => WorkbenchWorkspaceCloseFailure::Response(error),
            ExchangeAttempt::ReadDeadline(error) => {
                WorkbenchWorkspaceCloseFailure::ResponseDeadline(error)
            }
        };
        self.connection.take();
        Err(WorkspaceCloseExchangeError::Failed(failure))
    }
}

fn classify_command_result(
    command: &WorkbenchCommand,
    response: WorkbenchResponse,
) -> Result<WorkbenchWorkspaceCloseSubmission, WorkbenchWorkspaceCloseFailure> {
    let WorkbenchResponse::Command(result) = response else {
        return Err(WorkbenchWorkspaceCloseFailure::UnexpectedResponse);
    };
    validate_correlation(&command.request_id, &result.request_id)?;
    let expected_workspace = match &command.action {
        WorkbenchAction::WorkspaceClose { workspace_id }
        | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => workspace_id,
        _ => unreachable!("opaque request admits only Workspace Close actions"),
    };
    match result.outcome {
        WorkbenchCommandOutcome::WorkspaceCloseAccepted {
            close_request_id,
            workspace_id,
            terminal_context_id,
            accepted_revision,
        } => {
            validate_scope(expected_workspace, &workspace_id)?;
            if accepted_revision.value() == u64::MAX
                || accepted_revision > command.expected_revision
                || (close_request_id == command.request_id
                    && accepted_revision != command.expected_revision)
            {
                return Err(WorkbenchWorkspaceCloseFailure::AcceptedRevisionMismatch {
                    admitted: command.expected_revision,
                    received: accepted_revision,
                });
            }
            Ok(WorkbenchWorkspaceCloseSubmission::Accepted(
                WorkbenchWorkspaceCloseAccepted {
                    close_request_id,
                    workspace_id,
                    terminal_context_id,
                    accepted_revision,
                },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => {
            Ok(WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(
                WorkbenchWorkspaceClosePrecommit::Denied,
            ))
        }
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if (reason == WorkbenchRejectionReason::ConfirmationRequired
                && matches!(command.action, WorkbenchAction::WorkspaceCloseIfIdle { .. })
                && revision == command.expected_revision)
                || matches!(
                    reason,
                    WorkbenchRejectionReason::SiteMismatch
                        | WorkbenchRejectionReason::RevisionMismatch
                        | WorkbenchRejectionReason::ProjectUnavailable
                        | WorkbenchRejectionReason::WorkspaceUnavailable
                        | WorkbenchRejectionReason::WorkspaceGroupCloseRequired
                        | WorkbenchRejectionReason::ResourceLimit
                        | WorkbenchRejectionReason::RevisionExhausted
                        | WorkbenchRejectionReason::RequestIdentityConflict
                ) =>
        {
            Ok(WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(
                WorkbenchWorkspaceClosePrecommit::Rejected { revision, reason },
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
            Ok(WorkbenchWorkspaceCloseSubmission::DefinitePrecommit(
                WorkbenchWorkspaceClosePrecommit::Unavailable { reason },
            ))
        }
        _ => Err(WorkbenchWorkspaceCloseFailure::ActionIncompatible),
    }
}

fn validate_correlation(
    expected: &WorkbenchRequestId,
    received: &WorkbenchRequestId,
) -> Result<(), WorkbenchWorkspaceCloseFailure> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchWorkspaceCloseFailure::CorrelationMismatch {
            expected: expected.clone(),
            received: received.clone(),
        })
    }
}

fn validate_scope(
    expected_workspace: &WorkbenchWorkspaceId,
    received_workspace: &WorkbenchWorkspaceId,
) -> Result<(), WorkbenchWorkspaceCloseFailure> {
    if expected_workspace != received_workspace {
        return Err(WorkbenchWorkspaceCloseFailure::WorkspaceMismatch {
            expected: expected_workspace.clone(),
            received: received_workspace.clone(),
        });
    }
    Ok(())
}

enum WorkspaceCloseExchangeError {
    NotSubmitted(WorkbenchWorkspaceCloseNotSubmitted),
    Failed(WorkbenchWorkspaceCloseFailure),
}

#[derive(Debug)]
pub(crate) enum WorkbenchWorkspaceCloseNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}
impl fmt::Display for WorkbenchWorkspaceCloseNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP Workspace Close request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP Workspace Close request failed before any transport handoff: {error}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchWorkspaceCloseNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchWorkspaceCloseFailure {
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
    WorkspaceMismatch {
        expected: WorkbenchWorkspaceId,
        received: WorkbenchWorkspaceId,
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
impl fmt::Display for WorkbenchWorkspaceCloseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid Workspace Close response: {error}"),
            Self::ResponseDeadline(error) => {
                write!(f, "Workspace Close response failed: {error}")
            }
            Self::UnexpectedResponse => {
                f.write_str("received the wrong Workspace Close response envelope")
            }
            Self::ActionIncompatible => {
                f.write_str("received an action-incompatible workspace.close outcome")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request {expected} but received {received}"
            ),
            Self::SiteMismatch { expected, received } => {
                write!(f, "expected Site {expected} but received {received}")
            }
            Self::WorkspaceMismatch { expected, received } => {
                write!(f, "expected Workspace {expected} but received {received}")
            }
            Self::OperationMismatch { expected, received } => write!(
                f,
                "expected canonical Workspace Close request {expected} but received {received}"
            ),
            Self::AcceptedRevisionMismatch { admitted, received } => write!(
                f,
                "Workspace Close accepted revision {received} is incompatible with admitted base {admitted}"
            ),
            Self::AttemptGenerationMismatch { expected, received } => write!(
                f,
                "Workspace Close Resume generation {received} is incompatible with submitted generation {expected}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchWorkspaceCloseFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::ActionIncompatible
            | Self::CorrelationMismatch { .. }
            | Self::SiteMismatch { .. }
            | Self::WorkspaceMismatch { .. }
            | Self::OperationMismatch { .. }
            | Self::AcceptedRevisionMismatch { .. }
            | Self::AttemptGenerationMismatch { .. } => None,
        }
    }
}
