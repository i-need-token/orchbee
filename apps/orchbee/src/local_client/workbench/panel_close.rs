use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};
use std::{fmt, io, time::Instant};

#[derive(Debug)]
pub(crate) struct WorkbenchPanelCloseRequest(WorkbenchCommand);
impl TryFrom<WorkbenchCommand> for WorkbenchPanelCloseRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::PanelClose { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchPanelCloseAccepted {
    pub(crate) close_request_id: WorkbenchRequestId,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) panel_id: WorkbenchPanelId,
    pub(crate) accepted_revision: WorkbenchRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelClosePrecommit {
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
pub(crate) enum WorkbenchPanelCloseSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelCloseNotSubmitted),
    DefinitePrecommit(WorkbenchPanelClosePrecommit),
    Accepted(WorkbenchPanelCloseAccepted),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelCloseFailure,
    },
}

impl LocalClient {
    pub(crate) fn submit_workbench_panel_close(
        &mut self,
        request: WorkbenchPanelCloseRequest,
        deadline: Instant,
    ) -> WorkbenchPanelCloseSubmission {
        self.submit_workbench_panel_close_cancellable(request, deadline, None)
    }

    pub(crate) fn submit_workbench_panel_close_cancellable(
        &mut self,
        request: WorkbenchPanelCloseRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchPanelCloseSubmission {
        let command = request.0;
        let response = match self.exchange_panel_close(
            WorkbenchRequest::Command(command.clone()),
            deadline,
            wake,
        ) {
            Ok(response) => response,
            Err(PanelCloseExchangeError::NotSubmitted(reason)) => {
                return WorkbenchPanelCloseSubmission::DefinitelyNotSubmitted(reason);
            }
            Err(PanelCloseExchangeError::Failed(reason)) => {
                return WorkbenchPanelCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                };
            }
        };
        match classify_command_result(&command, response) {
            Ok(result) => result,
            Err(reason) => {
                self.connection.take();
                WorkbenchPanelCloseSubmission::OutcomeUnknown {
                    request_id: command.request_id,
                    reason,
                }
            }
        }
    }

    fn exchange_panel_close(
        &mut self,
        request: WorkbenchRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> Result<WorkbenchResponse, PanelCloseExchangeError> {
        if self.connection.is_none() {
            return Err(PanelCloseExchangeError::NotSubmitted(
                WorkbenchPanelCloseNotSubmitted::ConnectionUnavailable,
            ));
        }
        let mut frame = Vec::new();
        write_workbench_request(&mut frame, &request).map_err(|error| {
            PanelCloseExchangeError::NotSubmitted(WorkbenchPanelCloseNotSubmitted::RequestEncoding(
                error,
            ))
        })?;
        let deadline = self.submission_deadline_before(deadline).map_err(|error| {
            PanelCloseExchangeError::NotSubmitted(
                WorkbenchPanelCloseNotSubmitted::RequestNotSubmitted(error),
            )
        })?;
        let Some(connection) = self.connection.as_mut() else {
            return Err(PanelCloseExchangeError::NotSubmitted(
                WorkbenchPanelCloseNotSubmitted::ConnectionUnavailable,
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
                return Err(PanelCloseExchangeError::NotSubmitted(
                    WorkbenchPanelCloseNotSubmitted::RequestNotSubmitted(error),
                ));
            }
            ExchangeAttempt::WriteFailed(error) => WorkbenchPanelCloseFailure::Write(error),
            ExchangeAttempt::ReadFailed(error) => WorkbenchPanelCloseFailure::Response(error),
            ExchangeAttempt::ReadDeadline(error) => {
                WorkbenchPanelCloseFailure::ResponseDeadline(error)
            }
        };
        self.connection.take();
        Err(PanelCloseExchangeError::Failed(failure))
    }
}

fn classify_command_result(
    command: &WorkbenchCommand,
    response: WorkbenchResponse,
) -> Result<WorkbenchPanelCloseSubmission, WorkbenchPanelCloseFailure> {
    let WorkbenchResponse::Command(result) = response else {
        return Err(WorkbenchPanelCloseFailure::UnexpectedResponse);
    };
    if command.request_id != result.request_id {
        return Err(WorkbenchPanelCloseFailure::CorrelationMismatch {
            expected: command.request_id.clone(),
            received: result.request_id,
        });
    }
    let WorkbenchAction::PanelClose {
        terminal_context_id: expected_context,
        tab_id: expected_tab,
        panel_id: expected_panel,
    } = &command.action
    else {
        unreachable!("opaque request admits only panel.close")
    };
    match result.outcome {
        WorkbenchCommandOutcome::PanelCloseAccepted {
            close_request_id,
            terminal_context_id,
            tab_id,
            panel_id,
            accepted_revision,
        } => {
            if terminal_context_id != *expected_context {
                return Err(WorkbenchPanelCloseFailure::TerminalContextMismatch {
                    expected: expected_context.clone(),
                    received: terminal_context_id,
                });
            }
            if tab_id != *expected_tab {
                return Err(WorkbenchPanelCloseFailure::TabMismatch {
                    expected: expected_tab.clone(),
                    received: tab_id,
                });
            }
            if panel_id != *expected_panel {
                return Err(WorkbenchPanelCloseFailure::PanelMismatch {
                    expected: expected_panel.clone(),
                    received: panel_id,
                });
            }
            if accepted_revision.value() == u64::MAX
                || accepted_revision > command.expected_revision
                || (close_request_id == command.request_id
                    && accepted_revision != command.expected_revision)
            {
                return Err(WorkbenchPanelCloseFailure::AcceptedRevisionMismatch {
                    admitted: command.expected_revision,
                    received: accepted_revision,
                });
            }
            Ok(WorkbenchPanelCloseSubmission::Accepted(
                WorkbenchPanelCloseAccepted {
                    close_request_id,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    accepted_revision,
                },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchPanelCloseSubmission::DefinitePrecommit(
            WorkbenchPanelClosePrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SessionMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::TerminalContextUnavailable
                    | WorkbenchRejectionReason::TabUnavailable
                    | WorkbenchRejectionReason::PanelUnavailable
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            Ok(WorkbenchPanelCloseSubmission::DefinitePrecommit(
                WorkbenchPanelClosePrecommit::Rejected { revision, reason },
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
            Ok(WorkbenchPanelCloseSubmission::DefinitePrecommit(
                WorkbenchPanelClosePrecommit::Unavailable { reason },
            ))
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchPanelCloseFailure::ActionIncompatible),
    }
}

enum PanelCloseExchangeError {
    NotSubmitted(WorkbenchPanelCloseNotSubmitted),
    Failed(WorkbenchPanelCloseFailure),
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelCloseNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}
impl fmt::Display for WorkbenchPanelCloseNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP panel.close request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP panel.close request failed before any transport handoff: {error}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchPanelCloseNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelCloseFailure {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    ActionIncompatible,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
    TerminalContextMismatch {
        expected: WorkbenchTerminalContextId,
        received: WorkbenchTerminalContextId,
    },
    TabMismatch {
        expected: WorkbenchTabId,
        received: WorkbenchTabId,
    },
    PanelMismatch {
        expected: WorkbenchPanelId,
        received: WorkbenchPanelId,
    },
    AcceptedRevisionMismatch {
        admitted: WorkbenchRevision,
        received: WorkbenchRevision,
    },
}
impl fmt::Display for WorkbenchPanelCloseFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid panel.close response: {error}"),
            Self::ResponseDeadline(error) => write!(f, "panel.close response failed: {error}"),
            Self::UnexpectedResponse => f.write_str("expected workbench.command.result response"),
            Self::ActionIncompatible => {
                f.write_str("received an action-incompatible panel.close outcome")
            }
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request {expected} but received {received}"
            ),
            Self::TerminalContextMismatch { expected, received } => write!(
                f,
                "expected terminal context {expected} but received {received}"
            ),
            Self::TabMismatch { expected, received } => {
                write!(f, "expected Tab {expected} but received {received}")
            }
            Self::PanelMismatch { expected, received } => {
                write!(f, "expected Panel {expected} but received {received}")
            }
            Self::AcceptedRevisionMismatch { admitted, received } => write!(
                f,
                "panel.close accepted revision {received} is incompatible with admitted base {admitted}"
            ),
        }
    }
}
impl std::error::Error for WorkbenchPanelCloseFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::ActionIncompatible
            | Self::CorrelationMismatch { .. }
            | Self::TerminalContextMismatch { .. }
            | Self::TabMismatch { .. }
            | Self::PanelMismatch { .. }
            | Self::AcceptedRevisionMismatch { .. } => None,
        }
    }
}
