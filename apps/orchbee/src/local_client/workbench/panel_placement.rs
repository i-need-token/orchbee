use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_panel_placement(
        &mut self,
        request: WorkbenchPanelPlacementRequest,
        deadline: Instant,
    ) -> WorkbenchPanelPlacementSubmission {
        self.submit_workbench_panel_placement_with_wake(request, deadline, None)
    }

    pub(crate) fn submit_workbench_panel_placement_with_wake(
        &mut self,
        request: WorkbenchPanelPlacementRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchPanelPlacementSubmission {
        if self.connection.is_none() {
            return WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelPlacementNotSubmitted::ConnectionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let kind = if matches!(command.action, WorkbenchAction::PanelSwap { .. }) {
            PlacementKind::Swap
        } else {
            PlacementKind::Move
        };
        let (WorkbenchAction::PanelMove {
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
            ..
        }
        | WorkbenchAction::PanelSwap {
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
        }) = &command.action
        else {
            unreachable!("opaque placement request admits only panel.move or panel.swap")
        };
        let expected_context = terminal_context_id.clone();
        let expected_tab = tab_id.clone();
        let expected_panel = panel_id.clone();
        let expected_target = target_panel_id.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelPlacementNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelPlacementNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelPlacementNotSubmitted::ConnectionUnavailable,
            );
        };
        let attempt = exchange_once(
            connection,
            &self.clock,
            deadline,
            self.io_chunk_limits,
            wake,
            &frame,
            |io| read_workbench_response(io),
        );
        let response = match attempt {
            ExchangeAttempt::Result(response) => response,
            ExchangeAttempt::NotSubmitted(error) => {
                return WorkbenchPanelPlacementSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelPlacementNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_panel_placement_unknown(
                    request_id,
                    WorkbenchPanelPlacementOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_panel_placement_unknown(
                    request_id,
                    WorkbenchPanelPlacementOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_panel_placement_unknown(
                    request_id,
                    WorkbenchPanelPlacementOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_panel_placement_unknown(
                request_id,
                WorkbenchPanelPlacementOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_panel_placement_unknown(
                request_id.clone(),
                WorkbenchPanelPlacementOutcomeUnknownReason::CorrelationMismatch {
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
            kind,
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_panel_placement_unknown(request_id, reason),
        }
    }

    fn retire_with_panel_placement_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelPlacementOutcomeUnknownReason,
    ) -> WorkbenchPanelPlacementSubmission {
        self.connection.take();
        WorkbenchPanelPlacementSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchPanelPlacementRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchPanelPlacementRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(
            command.action,
            WorkbenchAction::PanelMove { .. } | WorkbenchAction::PanelSwap { .. }
        ) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelPlacementPrecommit {
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
pub(crate) struct WorkbenchPanelPlacementCommit {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) panel_id: WorkbenchPanelId,
    pub(crate) target_panel_id: WorkbenchPanelId,
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelPlacementSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelPlacementNotSubmitted),
    DefinitePrecommit(WorkbenchPanelPlacementPrecommit),
    Unchanged {
        revision: WorkbenchRevision,
    },
    Committed(WorkbenchPanelPlacementCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelPlacementOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelPlacementNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchPanelPlacementNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP Panel placement request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP Panel placement request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelPlacementNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelPlacementOutcomeUnknownReason {
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

impl fmt::Display for WorkbenchPanelPlacementOutcomeUnknownReason {
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

impl std::error::Error for WorkbenchPanelPlacementOutcomeUnknownReason {
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum PlacementKind {
    Move,
    Swap,
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    expected_panel: &WorkbenchPanelId,
    expected_target: &WorkbenchPanelId,
    kind: PlacementKind,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchPanelPlacementSubmission, WorkbenchPanelPlacementOutcomeUnknownReason> {
    let compatible_commit = match kind {
        PlacementKind::Move => matches!(&outcome, WorkbenchCommandOutcome::PanelMoved { .. }),
        PlacementKind::Swap => matches!(&outcome, WorkbenchCommandOutcome::PanelSwapped { .. }),
    };
    match outcome {
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
                    | WorkbenchRejectionReason::IdentityExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) && (kind == PlacementKind::Move
                || reason != WorkbenchRejectionReason::IdentityExhausted) =>
        {
            Ok(WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                WorkbenchPanelPlacementPrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => {
            Ok(WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                WorkbenchPanelPlacementPrecommit::Denied,
            ))
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::TerminalBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchPanelPlacementSubmission::DefinitePrecommit(
                WorkbenchPanelPlacementPrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::Unchanged { revision }
            if kind == PlacementKind::Move || expected_panel == expected_target =>
        {
            validate_revision(expected_revision, revision)?;
            Ok(WorkbenchPanelPlacementSubmission::Unchanged { revision })
        }
        WorkbenchCommandOutcome::PanelMoved {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
        }
        | WorkbenchCommandOutcome::PanelSwapped {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_panel_id,
        } if compatible_commit => {
            let successor = expected_revision.value().checked_add(1).ok_or(
                WorkbenchPanelPlacementOutcomeUnknownReason::ExpectedRevisionOverflow {
                    expected: expected_revision,
                },
            )?;
            validate_revision(WorkbenchRevision::new(successor), revision)?;
            if &terminal_context_id != expected_context {
                return Err(
                    WorkbenchPanelPlacementOutcomeUnknownReason::TerminalContextMismatch {
                        expected: expected_context.clone(),
                        received: terminal_context_id,
                    },
                );
            }
            if &tab_id != expected_tab {
                return Err(WorkbenchPanelPlacementOutcomeUnknownReason::TabMismatch {
                    expected: expected_tab.clone(),
                    received: tab_id,
                });
            }
            if &panel_id != expected_panel {
                return Err(WorkbenchPanelPlacementOutcomeUnknownReason::PanelMismatch {
                    expected: expected_panel.clone(),
                    received: panel_id,
                });
            }
            if &target_panel_id != expected_target {
                return Err(
                    WorkbenchPanelPlacementOutcomeUnknownReason::TargetPanelMismatch {
                        expected: expected_target.clone(),
                        received: target_panel_id,
                    },
                );
            }
            Ok(WorkbenchPanelPlacementSubmission::Committed(
                WorkbenchPanelPlacementCommit {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    target_panel_id,
                },
            ))
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchPanelPlacementOutcomeUnknownReason::ActionIncompatible),
    }
}

fn validate_revision(
    expected: WorkbenchRevision,
    received: WorkbenchRevision,
) -> Result<(), WorkbenchPanelPlacementOutcomeUnknownReason> {
    if expected == received {
        Ok(())
    } else {
        Err(WorkbenchPanelPlacementOutcomeUnknownReason::RevisionMismatch { expected, received })
    }
}
