use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    LaunchProfileId, TerminalPresentationId, WorkbenchAction, WorkbenchCommand,
    WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId,
    WorkbenchResponse, WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    pub(crate) fn submit_workbench_panel_split(
        &mut self,
        request: WorkbenchPanelSplitRequest,
        deadline: Instant,
    ) -> WorkbenchPanelSplitSubmission {
        self.submit_workbench_panel_split_with_wake(request, deadline, None)
    }

    pub(crate) fn submit_workbench_panel_split_with_wake(
        &mut self,
        request: WorkbenchPanelSplitRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchPanelSplitSubmission {
        if self.connection.is_none() {
            return WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelSplitNotSubmitted::ConnectionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let WorkbenchAction::PanelSplit {
            terminal_context_id,
            tab_id,
            panel_id,
            geometry,
            ..
        } = &command.action
        else {
            unreachable!("opaque panel-split request only admits panel.split")
        };
        let expected_context = terminal_context_id.clone();
        let expected_tab = tab_id.clone();
        let expected_panel = panel_id.clone();
        let expected_geometry = *geometry;
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelSplitNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelSplitNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelSplitNotSubmitted::ConnectionUnavailable,
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
                return WorkbenchPanelSplitSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelSplitNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_panel_split_unknown(
                    request_id,
                    WorkbenchPanelSplitOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_panel_split_unknown(
                    request_id,
                    WorkbenchPanelSplitOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_panel_split_unknown(
                    request_id,
                    WorkbenchPanelSplitOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_panel_split_unknown(
                request_id,
                WorkbenchPanelSplitOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_panel_split_unknown(
                request_id.clone(),
                WorkbenchPanelSplitOutcomeUnknownReason::CorrelationMismatch {
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
            expected_geometry,
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_panel_split_unknown(request_id, reason),
        }
    }

    fn retire_with_panel_split_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelSplitOutcomeUnknownReason,
    ) -> WorkbenchPanelSplitSubmission {
        self.connection.take();
        WorkbenchPanelSplitSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchPanelSplitRequest(pub(crate) WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchPanelSplitRequest {
    type Error = WorkbenchCommand;

    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::PanelSplit { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelSplitPrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
    ProfileUnavailable,
    ProfileAmbiguous {
        candidate_ids: Vec<LaunchProfileId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelSplitCommit {
    Pending {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        new_panel_id: WorkbenchPanelId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        presentation_id: TerminalPresentationId,
    },
    Created {
        revision: WorkbenchRevision,
        terminal_context_id: WorkbenchTerminalContextId,
        tab_id: WorkbenchTabId,
        panel_id: WorkbenchPanelId,
        new_panel_id: WorkbenchPanelId,
        split_id: WorkbenchSplitId,
        first_slot_id: WorkbenchPanelSlotId,
        second_slot_id: WorkbenchPanelSlotId,
        presentation_id: TerminalPresentationId,
        terminal_outcome: WorkbenchTerminalOutcome,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelSplitSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelSplitNotSubmitted),
    DefinitePrecommit(WorkbenchPanelSplitPrecommit),
    Committed(WorkbenchPanelSplitCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelSplitOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelSplitNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchPanelSplitNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP panel.split request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP panel.split request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelSplitNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelSplitOutcomeUnknownReason {
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
    StartedGeometryMismatch {
        expected: WorkbenchTerminalGeometry,
        received: WorkbenchTerminalGeometry,
    },
}

impl fmt::Display for WorkbenchPanelSplitOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible panel.split outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "panel.split expected revision {expected} has no successor"
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
            Self::StartedGeometryMismatch { expected, received } => write!(
                f,
                "expected started geometry {expected:?} but received {received:?}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelSplitOutcomeUnknownReason {
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
            | Self::StartedGeometryMismatch { .. } => None,
        }
    }
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    expected_panel: &WorkbenchPanelId,
    expected_geometry: WorkbenchTerminalGeometry,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchPanelSplitSubmission, WorkbenchPanelSplitOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if panel_split_rejection_is_compatible(reason) =>
        {
            Ok(WorkbenchPanelSplitSubmission::DefinitePrecommit(
                WorkbenchPanelSplitPrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => Ok(WorkbenchPanelSplitSubmission::DefinitePrecommit(
            WorkbenchPanelSplitPrecommit::Denied,
        )),
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::NoCompatibleWorker
                    | WorkbenchCommandUnavailableReason::TerminalBusy
                    | WorkbenchCommandUnavailableReason::ServerBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchPanelSplitSubmission::DefinitePrecommit(
                WorkbenchPanelSplitPrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::LaunchProfileUnavailable {} => {
            Ok(WorkbenchPanelSplitSubmission::DefinitePrecommit(
                WorkbenchPanelSplitPrecommit::ProfileUnavailable,
            ))
        }
        WorkbenchCommandOutcome::LaunchProfileAmbiguous { candidate_ids } => {
            Ok(WorkbenchPanelSplitSubmission::DefinitePrecommit(
                WorkbenchPanelSplitPrecommit::ProfileAmbiguous { candidate_ids },
            ))
        }
        WorkbenchCommandOutcome::PanelSplitPending {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            new_panel_id,
            split_id,
            first_slot_id,
            second_slot_id,
            presentation_id,
        } => {
            validate_commit_fields(
                PanelSplitIdentity {
                    revision: expected_revision,
                    terminal_context_id: expected_context,
                    tab_id: expected_tab,
                    panel_id: expected_panel,
                },
                PanelSplitIdentity {
                    revision,
                    terminal_context_id: &terminal_context_id,
                    tab_id: &tab_id,
                    panel_id: &panel_id,
                },
            )?;
            Ok(WorkbenchPanelSplitSubmission::Committed(
                WorkbenchPanelSplitCommit::Pending {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                },
            ))
        }
        WorkbenchCommandOutcome::PanelSplitCreated {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            new_panel_id,
            split_id,
            first_slot_id,
            second_slot_id,
            presentation_id,
            terminal_outcome,
        } => {
            validate_commit_fields(
                PanelSplitIdentity {
                    revision: expected_revision,
                    terminal_context_id: expected_context,
                    tab_id: expected_tab,
                    panel_id: expected_panel,
                },
                PanelSplitIdentity {
                    revision,
                    terminal_context_id: &terminal_context_id,
                    tab_id: &tab_id,
                    panel_id: &panel_id,
                },
            )?;
            if let WorkbenchTerminalOutcome::Started { geometry } = terminal_outcome
                && geometry != expected_geometry
            {
                return Err(
                    WorkbenchPanelSplitOutcomeUnknownReason::StartedGeometryMismatch {
                        expected: expected_geometry,
                        received: geometry,
                    },
                );
            }
            Ok(WorkbenchPanelSplitSubmission::Committed(
                WorkbenchPanelSplitCommit::Created {
                    revision,
                    terminal_context_id,
                    tab_id,
                    panel_id,
                    new_panel_id,
                    split_id,
                    first_slot_id,
                    second_slot_id,
                    presentation_id,
                    terminal_outcome,
                },
            ))
        }
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchPanelSplitOutcomeUnknownReason::ActionIncompatible),
    }
}

fn panel_split_rejection_is_compatible(reason: WorkbenchRejectionReason) -> bool {
    matches!(
        reason,
        WorkbenchRejectionReason::SiteMismatch
            | WorkbenchRejectionReason::RevisionMismatch
            | WorkbenchRejectionReason::TerminalContextUnavailable
            | WorkbenchRejectionReason::TabUnavailable
            | WorkbenchRejectionReason::PanelUnavailable
            | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
            | WorkbenchRejectionReason::ResourceLimit
            | WorkbenchRejectionReason::IdentityExhausted
            | WorkbenchRejectionReason::RevisionExhausted
            | WorkbenchRejectionReason::RequestIdentityConflict
    )
}

struct PanelSplitIdentity<'a> {
    revision: WorkbenchRevision,
    terminal_context_id: &'a WorkbenchTerminalContextId,
    tab_id: &'a WorkbenchTabId,
    panel_id: &'a WorkbenchPanelId,
}

fn validate_commit_fields(
    expected: PanelSplitIdentity<'_>,
    received: PanelSplitIdentity<'_>,
) -> Result<(), WorkbenchPanelSplitOutcomeUnknownReason> {
    let successor = expected.revision.value().checked_add(1).ok_or(
        WorkbenchPanelSplitOutcomeUnknownReason::ExpectedRevisionOverflow {
            expected: expected.revision,
        },
    )?;
    let expected_revision = WorkbenchRevision::new(successor);
    if received.revision != expected_revision {
        return Err(WorkbenchPanelSplitOutcomeUnknownReason::RevisionMismatch {
            expected: expected_revision,
            received: received.revision,
        });
    }
    if received.terminal_context_id != expected.terminal_context_id {
        return Err(
            WorkbenchPanelSplitOutcomeUnknownReason::TerminalContextMismatch {
                expected: expected.terminal_context_id.clone(),
                received: received.terminal_context_id.clone(),
            },
        );
    }
    if received.tab_id != expected.tab_id {
        return Err(WorkbenchPanelSplitOutcomeUnknownReason::TabMismatch {
            expected: expected.tab_id.clone(),
            received: received.tab_id.clone(),
        });
    }
    if received.panel_id != expected.panel_id {
        return Err(WorkbenchPanelSplitOutcomeUnknownReason::PanelMismatch {
            expected: expected.panel_id.clone(),
            received: received.panel_id.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
