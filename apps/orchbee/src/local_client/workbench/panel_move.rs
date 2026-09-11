use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_panel_move(
        &mut self,
        request: WorkbenchPanelMoveRequest,
        deadline: Instant,
    ) -> WorkbenchPanelMoveSubmission {
        if self.connection.is_none() {
            return WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelMoveNotSubmitted::SessionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let WorkbenchAction::PanelMove {
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
            ..
        } = &command.action
        else {
            unreachable!("opaque panel-move request only admits panel.move")
        };
        let expected_context = terminal_context_id.clone();
        let expected_tab = tab_id.clone();
        let expected_panel = panel_id.clone();
        let expected_target = target_panel_id.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelMoveNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelMoveNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelMoveNotSubmitted::SessionUnavailable,
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
                return WorkbenchPanelMoveSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelMoveNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_panel_move_unknown(
                    request_id,
                    WorkbenchPanelMoveOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_panel_move_unknown(
                    request_id,
                    WorkbenchPanelMoveOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_panel_move_unknown(
                    request_id,
                    WorkbenchPanelMoveOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_panel_move_unknown(
                request_id,
                WorkbenchPanelMoveOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_panel_move_unknown(
                request_id.clone(),
                WorkbenchPanelMoveOutcomeUnknownReason::CorrelationMismatch {
                    expected: request_id,
                    received: result.request_id,
                },
            );
        }
        match classify_outcome(
            expected_revision,
            &expected_context,
            &expected_tab,
            &expected_panel,
            &expected_target,
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_panel_move_unknown(request_id, reason),
        }
    }

    fn retire_with_panel_move_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelMoveOutcomeUnknownReason,
    ) -> WorkbenchPanelMoveSubmission {
        self.connection.take();
        WorkbenchPanelMoveSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchPanelMoveRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchPanelMoveRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::PanelMove { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelMovePrecommit {
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
pub(crate) struct WorkbenchPanelMoveCommit {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) panel_id: WorkbenchPanelId,
    pub(crate) target_panel_id: WorkbenchPanelId,
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelMoveSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelMoveNotSubmitted),
    DefinitePrecommit(WorkbenchPanelMovePrecommit),
    Unchanged {
        revision: WorkbenchRevision,
    },
    Committed(WorkbenchPanelMoveCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelMoveOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelMoveNotSubmitted {
    SessionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchPanelMoveNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionUnavailable => f.write_str("local Client Session is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP panel.move request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP panel.move request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelMoveNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SessionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelMoveOutcomeUnknownReason {
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
    PanelMismatch {
        expected: WorkbenchPanelId,
        received: WorkbenchPanelId,
    },
    TargetPanelMismatch {
        expected: WorkbenchPanelId,
        received: WorkbenchPanelId,
    },
}

impl fmt::Display for WorkbenchPanelMoveOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible panel.move outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "panel.move expected revision {expected} has no successor"
                )
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
            Self::PanelMismatch { expected, received } => {
                write!(f, "expected Panel {expected} but received {received}")
            }
            Self::TargetPanelMismatch { expected, received } => {
                write!(
                    f,
                    "expected target Panel {expected} but received {received}"
                )
            }
        }
    }
}

impl std::error::Error for WorkbenchPanelMoveOutcomeUnknownReason {
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
            | Self::TabMismatch { .. }
            | Self::PanelMismatch { .. }
            | Self::TargetPanelMismatch { .. } => None,
        }
    }
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    expected_panel: &WorkbenchPanelId,
    expected_target: &WorkbenchPanelId,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchPanelMoveSubmission, WorkbenchPanelMoveOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SiteMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::TerminalContextUnavailable
                    | WorkbenchRejectionReason::TabUnavailable
                    | WorkbenchRejectionReason::PanelUnavailable
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::IdentityExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            Ok(WorkbenchPanelMoveSubmission::DefinitePrecommit(
                WorkbenchPanelMovePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchPanelMoveSubmission::DefinitePrecommit(
            WorkbenchPanelMovePrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchPanelMoveSubmission::DefinitePrecommit(
                WorkbenchPanelMovePrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::Unchanged { revision } => {
            validate_revision(expected_revision, revision)?;
            Ok(WorkbenchPanelMoveSubmission::Unchanged { revision })
        }
        WorkbenchCommandOutcome::PanelMoved {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
        } => {
            let successor = expected_revision.value().checked_add(1).ok_or(
                WorkbenchPanelMoveOutcomeUnknownReason::ExpectedRevisionOverflow {
                    expected: expected_revision,
                },
            )?;
            validate_revision(WorkbenchRevision::new(successor), revision)?;
            if &terminal_context_id != expected_context {
                return Err(
                    WorkbenchPanelMoveOutcomeUnknownReason::TerminalContextMismatch {
                        expected: expected_context.clone(),
                        received: terminal_context_id,
                    },
                );
            }
            if &tab_id != expected_tab {
                return Err(WorkbenchPanelMoveOutcomeUnknownReason::TabMismatch {
                    expected: expected_tab.clone(),
                    received: tab_id,
                });
            }
            if &panel_id != expected_panel {
                return Err(WorkbenchPanelMoveOutcomeUnknownReason::PanelMismatch {
                    expected: expected_panel.clone(),
                    received: panel_id,
                });
            }
            if &target_panel_id != expected_target {
                return Err(
                    WorkbenchPanelMoveOutcomeUnknownReason::TargetPanelMismatch {
                        expected: expected_target.clone(),
                        received: target_panel_id,
                    },
                );
            }
            Ok(WorkbenchPanelMoveSubmission::Committed(
                WorkbenchPanelMoveCommit {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                },
            ))
        }
        _ => Err(WorkbenchPanelMoveOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_revision(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), WorkbenchPanelMoveOutcomeUnknownReason> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchPanelMoveOutcomeUnknownReason::RevisionMismatch { expected, received })
    }
}
