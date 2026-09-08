use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWireError,
    read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_tab_move(
        &mut self,
        request: WorkbenchTabMoveRequest,
        deadline: Instant,
    ) -> WorkbenchTabMoveSubmission {
        if self.connection.is_none() {
            return WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchTabMoveNotSubmitted::SessionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let WorkbenchAction::TabMove {
            terminal_context_id,
            tab_id,
            ..
        } = &command.action
        else {
            unreachable!("opaque tab-move request only admits tab.move")
        };
        let expected_context = terminal_context_id.clone();
        let expected_tab = tab_id.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchTabMoveNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                    WorkbenchTabMoveNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchTabMoveNotSubmitted::SessionUnavailable,
            );
        };
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            &frame,
            |io| read_workbench_response(io),
        );
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkbenchTabMoveSubmission::DefinitelyNotSubmitted(
                    WorkbenchTabMoveNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_tab_move_unknown(
                    request_id,
                    WorkbenchTabMoveOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_tab_move_unknown(
                    request_id,
                    WorkbenchTabMoveOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_tab_move_unknown(
                    request_id,
                    WorkbenchTabMoveOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_tab_move_unknown(
                request_id,
                WorkbenchTabMoveOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_tab_move_unknown(
                request_id.clone(),
                WorkbenchTabMoveOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(
            expected_revision,
            &expected_context,
            &expected_tab,
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_tab_move_unknown(request_id, reason),
        }
    }

    fn retire_with_tab_move_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabMoveOutcomeUnknownReason,
    ) -> WorkbenchTabMoveSubmission {
        self.connection.take();
        WorkbenchTabMoveSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchTabMoveRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchTabMoveRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::TabMove { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchTabMovePrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchTabMoveCommit {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabMoveSubmission {
    DefinitelyNotSubmitted(WorkbenchTabMoveNotSubmitted),
    DefinitePrecommit(WorkbenchTabMovePrecommit),
    Unchanged {
        revision: WorkbenchRevision,
    },
    Committed(WorkbenchTabMoveCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchTabMoveOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabMoveNotSubmitted {
    SessionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchTabMoveNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => f.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP tab.move request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP tab.move request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchTabMoveNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchTabMoveOutcomeUnknownReason {
    Write(io::Error),
    Response(WorkbenchWireError),
    ResponseDeadline(io::Error),
    UnexpectedResponse,
    CorrelationMismatch {
        expected: WorkbenchRequestId,
        received: WorkbenchRequestId,
    },
    ActionIncompatible,
    ExpectedRevisionOverflow {
        expected: WorkbenchRevision,
    },
    RevisionMismatch {
        expected: WorkbenchRevision,
        received: WorkbenchRevision,
    },
    TerminalContextMismatch {
        expected: WorkbenchTerminalContextId,
        received: WorkbenchTerminalContextId,
    },
    TabMismatch {
        expected: WorkbenchTabId,
        received: WorkbenchTabId,
    },
}

impl fmt::Display for WorkbenchTabMoveOutcomeUnknownReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Write(error) => write!(f, "request write failed: {error}"),
            Self::Response(error) => write!(f, "invalid command response: {error}"),
            Self::ResponseDeadline(error) => write!(f, "command response failed: {error}"),
            Self::UnexpectedResponse => f.write_str("expected workbench.command.result response"),
            Self::CorrelationMismatch { expected, received } => write!(
                f,
                "expected Workbench request identity {expected} but received {received}"
            ),
            Self::ActionIncompatible => {
                f.write_str("received an action-incompatible tab.move outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(f, "tab.move expected revision {expected} has no successor")
            }
            Self::RevisionMismatch { expected, received } => write!(
                f,
                "expected Workbench revision {expected} but received {received}"
            ),
            Self::TerminalContextMismatch { expected, received } => write!(
                f,
                "expected terminal context {expected} but received {received}"
            ),
            Self::TabMismatch { expected, received } => {
                write!(f, "expected Tab {expected} but received {received}")
            }
        }
    }
}

impl std::error::Error for WorkbenchTabMoveOutcomeUnknownReason {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Write(error) | Self::ResponseDeadline(error) => Some(error),
            Self::Response(error) => Some(error),
            Self::UnexpectedResponse
            | Self::CorrelationMismatch { .. }
            | Self::ActionIncompatible
            | Self::ExpectedRevisionOverflow { .. }
            | Self::RevisionMismatch { .. }
            | Self::TerminalContextMismatch { .. }
            | Self::TabMismatch { .. } => None,
        }
    }
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchTabMoveSubmission, WorkbenchTabMoveOutcomeUnknownReason> {
    match outcome {
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
            Ok(WorkbenchTabMoveSubmission::DefinitePrecommit(
                WorkbenchTabMovePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchTabMoveSubmission::DefinitePrecommit(
            WorkbenchTabMovePrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchTabMoveSubmission::DefinitePrecommit(
                WorkbenchTabMovePrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::Unchanged { revision } => {
            validate_revision(expected_revision, revision)?;
            Ok(WorkbenchTabMoveSubmission::Unchanged { revision })
        }
        WorkbenchCommandOutcome::TabMoved {
            revision,
            terminal_context_id,
            tab_id,
        } => {
            let successor = expected_revision.value().checked_add(1).ok_or(
                WorkbenchTabMoveOutcomeUnknownReason::ExpectedRevisionOverflow {
                    expected: expected_revision,
                },
            )?;
            validate_revision(WorkbenchRevision::new(successor), revision)?;
            if &terminal_context_id != expected_context {
                return Err(
                    WorkbenchTabMoveOutcomeUnknownReason::TerminalContextMismatch {
                        expected: expected_context.clone(),
                        received: terminal_context_id,
                    },
                );
            }
            if &tab_id != expected_tab {
                return Err(WorkbenchTabMoveOutcomeUnknownReason::TabMismatch {
                    expected: expected_tab.clone(),
                    received: tab_id,
                });
            }
            Ok(WorkbenchTabMoveSubmission::Committed(
                WorkbenchTabMoveCommit {
                    revision,
                    terminal_context_id,
                    tab_id,
                },
            ))
        }
        _ => Err(WorkbenchTabMoveOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_revision(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), WorkbenchTabMoveOutcomeUnknownReason> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchTabMoveOutcomeUnknownReason::RevisionMismatch { expected, received })
    }
}
