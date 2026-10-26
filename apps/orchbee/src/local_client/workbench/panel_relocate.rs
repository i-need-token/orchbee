use std::{fmt, io, time::Instant};

use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchPanelId, WorkbenchPanelRelocateDestination, WorkbenchPanelSlotId,
    WorkbenchRejectionReason, WorkbenchRequest, WorkbenchRequestId, WorkbenchResponse,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchWireError, read_workbench_response, write_workbench_request,
};

use crate::local_client::{ExchangeAttempt, LocalClient, exchange_once};

impl LocalClient {
    #[cfg(test)]
    pub(crate) fn submit_workbench_panel_relocate(
        &mut self,
        request: WorkbenchPanelRelocateRequest,
        deadline: Instant,
    ) -> WorkbenchPanelRelocateSubmission {
        self.submit_workbench_panel_relocate_with_wake(request, deadline, None)
    }

    pub(crate) fn submit_workbench_panel_relocate_with_wake(
        &mut self,
        request: WorkbenchPanelRelocateRequest,
        deadline: Instant,
        wake: Option<orchbee_local_ipc::LocalIo<'_>>,
    ) -> WorkbenchPanelRelocateSubmission {
        if self.connection.is_none() {
            return WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelRelocateNotSubmitted::ConnectionUnavailable,
            );
        }
        let command = request.0;
        let request_id = command.request_id.clone();
        let expected_revision = command.expected_revision;
        let WorkbenchAction::PanelRelocate {
            terminal_context_id,
            tab_id,
            panel_id,
            destination,
        } = &command.action
        else {
            unreachable!("opaque panel-relocate request only admits panel.relocate")
        };
        let expected_context = terminal_context_id.clone();
        let expected_tab = tab_id.clone();
        let expected_panel = panel_id.clone();
        let expected_destination = destination.clone();
        let mut frame = Vec::new();
        if let Err(error) = write_workbench_request(&mut frame, &WorkbenchRequest::Command(command))
        {
            return WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelRelocateNotSubmitted::RequestEncoding(error),
            );
        }
        let deadline = match self.submission_deadline_before(deadline) {
            Ok(deadline) => deadline,
            Err(error) => {
                return WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelRelocateNotSubmitted::RequestNotSubmitted(error),
                );
            }
        };
        let Some(connection) = self.connection.as_mut() else {
            return WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(
                WorkbenchPanelRelocateNotSubmitted::ConnectionUnavailable,
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
                return WorkbenchPanelRelocateSubmission::DefinitelyNotSubmitted(
                    WorkbenchPanelRelocateNotSubmitted::RequestNotSubmitted(error),
                );
            }
            ExchangeAttempt::WriteFailed(error) => {
                return self.retire_with_panel_relocate_unknown(
                    request_id,
                    WorkbenchPanelRelocateOutcomeUnknownReason::Write(error),
                );
            }
            ExchangeAttempt::ReadFailed(error) => {
                return self.retire_with_panel_relocate_unknown(
                    request_id,
                    WorkbenchPanelRelocateOutcomeUnknownReason::Response(error),
                );
            }
            ExchangeAttempt::ReadDeadline(error) => {
                return self.retire_with_panel_relocate_unknown(
                    request_id,
                    WorkbenchPanelRelocateOutcomeUnknownReason::ResponseDeadline(error),
                );
            }
        };
        let WorkbenchResponse::Command(result) = response else {
            return self.retire_with_panel_relocate_unknown(
                request_id,
                WorkbenchPanelRelocateOutcomeUnknownReason::UnexpectedResponse,
            );
        };
        if result.request_id != request_id {
            return self.retire_with_panel_relocate_unknown(
                request_id.clone(),
                WorkbenchPanelRelocateOutcomeUnknownReason::CorrelationMismatch {
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
            &expected_destination,
            result.outcome,
        ) {
            Ok(submission) => submission,
            Err(reason) => self.retire_with_panel_relocate_unknown(request_id, reason),
        }
    }

    fn retire_with_panel_relocate_unknown(
        &mut self,
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelRelocateOutcomeUnknownReason,
    ) -> WorkbenchPanelRelocateSubmission {
        self.connection.take();
        WorkbenchPanelRelocateSubmission::OutcomeUnknown { request_id, reason }
    }
}

#[derive(Debug)]
pub(crate) struct WorkbenchPanelRelocateRequest(WorkbenchCommand);

impl TryFrom<WorkbenchCommand> for WorkbenchPanelRelocateRequest {
    type Error = WorkbenchCommand;
    fn try_from(command: WorkbenchCommand) -> Result<Self, Self::Error> {
        if matches!(command.action, WorkbenchAction::PanelRelocate { .. }) {
            Ok(Self(command))
        } else {
            Err(command)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WorkbenchPanelRelocatePrecommit {
    Rejected {
        revision: WorkbenchRevision,
        reason: WorkbenchRejectionReason,
    },
    Denied,
    Unavailable {
        reason: WorkbenchCommandUnavailableReason,
    },
}

/// A committed relocation: `target_tab_id` names the destination Tab — the
/// fresh identity under `new-tab`/`new-workspace`, the resolved Tab under `tab`.
/// `split` carries the minted graft Split identities only for an
/// existing-Tab insert.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkbenchPanelRelocateCommit {
    pub(crate) revision: WorkbenchRevision,
    pub(crate) terminal_context_id: WorkbenchTerminalContextId,
    pub(crate) tab_id: WorkbenchTabId,
    pub(crate) panel_id: WorkbenchPanelId,
    pub(crate) target_tab_id: WorkbenchTabId,
    pub(crate) split: Option<RelocatedSplitIds>,
    pub(crate) transfer: Option<orchbee_workbench_protocol::WorkbenchPanelTransfer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RelocatedSplitIds {
    pub(crate) split_id: WorkbenchSplitId,
    pub(crate) first_slot_id: WorkbenchPanelSlotId,
    pub(crate) second_slot_id: WorkbenchPanelSlotId,
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelRelocateSubmission {
    DefinitelyNotSubmitted(WorkbenchPanelRelocateNotSubmitted),
    DefinitePrecommit(WorkbenchPanelRelocatePrecommit),
    Committed(WorkbenchPanelRelocateCommit),
    OutcomeUnknown {
        request_id: WorkbenchRequestId,
        reason: WorkbenchPanelRelocateOutcomeUnknownReason,
    },
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelRelocateNotSubmitted {
    ConnectionUnavailable,
    RequestEncoding(WorkbenchWireError),
    RequestNotSubmitted(io::Error),
}

impl fmt::Display for WorkbenchPanelRelocateNotSubmitted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionUnavailable => f.write_str("local Client Connection is unavailable"),
            Self::RequestEncoding(error) => {
                write!(f, "OBWP panel.relocate request encoding failed: {error}")
            }
            Self::RequestNotSubmitted(error) => write!(
                f,
                "OBWP panel.relocate request failed before any transport handoff: {error}"
            ),
        }
    }
}

impl std::error::Error for WorkbenchPanelRelocateNotSubmitted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ConnectionUnavailable => None,
            Self::RequestEncoding(error) => Some(error),
            Self::RequestNotSubmitted(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub(crate) enum WorkbenchPanelRelocateOutcomeUnknownReason {
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
    TargetTabMismatch {
        expected: WorkbenchTabId,
        received: WorkbenchTabId,
    },
}

impl fmt::Display for WorkbenchPanelRelocateOutcomeUnknownReason {
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
                f.write_str("received an action-incompatible panel.relocate outcome")
            }
            Self::ExpectedRevisionOverflow { expected } => {
                write!(
                    f,
                    "panel.relocate expected revision {expected} has no successor"
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
            Self::TargetTabMismatch { expected, received } => {
                write!(f, "expected target Tab {expected} but received {received}")
            }
        }
    }
}

impl std::error::Error for WorkbenchPanelRelocateOutcomeUnknownReason {
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
            | Self::TargetTabMismatch { .. } => None,
        }
    }
}

fn classify_outcome(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    expected_panel: &WorkbenchPanelId,
    expected_destination: &WorkbenchPanelRelocateDestination,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchPanelRelocateSubmission, WorkbenchPanelRelocateOutcomeUnknownReason> {
    match outcome {
        WorkbenchCommandOutcome::Rejected { revision, reason }
            if matches!(
                reason,
                WorkbenchRejectionReason::SiteMismatch
                    | WorkbenchRejectionReason::RevisionMismatch
                    | WorkbenchRejectionReason::TerminalContextUnavailable
                    | WorkbenchRejectionReason::ProjectUnavailable
                    | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
                    | WorkbenchRejectionReason::TabUnavailable
                    | WorkbenchRejectionReason::PanelUnavailable
                    | WorkbenchRejectionReason::SameTab
                    | WorkbenchRejectionReason::ResourceLimit
                    | WorkbenchRejectionReason::RevisionExhausted
                    | WorkbenchRejectionReason::IdentityExhausted
                    | WorkbenchRejectionReason::RequestIdentityConflict
            ) =>
        {
            Ok(WorkbenchPanelRelocateSubmission::DefinitePrecommit(
                WorkbenchPanelRelocatePrecommit::Rejected { revision, reason },
            ))
        }
        WorkbenchCommandOutcome::Denied {} => {
            Ok(WorkbenchPanelRelocateSubmission::DefinitePrecommit(
                WorkbenchPanelRelocatePrecommit::Denied,
            ))
        }
        WorkbenchCommandOutcome::Unavailable { reason }
            if matches!(
                reason,
                WorkbenchCommandUnavailableReason::RequestLedgerFull
                    | WorkbenchCommandUnavailableReason::TerminalBusy
                    | WorkbenchCommandUnavailableReason::DirectoryUnavailable
                    | WorkbenchCommandUnavailableReason::NoCompatibleWorker
                    | WorkbenchCommandUnavailableReason::ServerBusy
                    | WorkbenchCommandUnavailableReason::ServiceUnavailable
            ) =>
        {
            Ok(WorkbenchPanelRelocateSubmission::DefinitePrecommit(
                WorkbenchPanelRelocatePrecommit::Unavailable { reason },
            ))
        }
        WorkbenchCommandOutcome::PanelRelocated {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_tab_id,
            split_id,
            first_slot_id,
            second_slot_id,
            transfer,
        } => {
            let split = RelocatedSplitIds {
                split_id,
                first_slot_id,
                second_slot_id,
            };
            classify_committed(
                expected_revision,
                expected_context,
                expected_tab,
                expected_panel,
                expected_destination,
                revision,
                terminal_context_id,
                tab_id,
                panel_id,
                target_tab_id,
                Some(split),
                transfer,
            )
        }
        WorkbenchCommandOutcome::PanelRelocatedToNewTab {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            new_tab_id,
            transfer,
        } => classify_committed(
            expected_revision,
            expected_context,
            expected_tab,
            expected_panel,
            expected_destination,
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            new_tab_id,
            None,
            transfer,
        ),
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => Err(WorkbenchPanelRelocateOutcomeUnknownReason::ActionIncompatible),
    }
}

#[allow(clippy::too_many_arguments)]
fn classify_committed(
    expected_revision: WorkbenchRevision,
    expected_context: &WorkbenchTerminalContextId,
    expected_tab: &WorkbenchTabId,
    expected_panel: &WorkbenchPanelId,
    expected_destination: &WorkbenchPanelRelocateDestination,
    revision: WorkbenchRevision,
    terminal_context_id: WorkbenchTerminalContextId,
    tab_id: WorkbenchTabId,
    panel_id: WorkbenchPanelId,
    target_tab_id: WorkbenchTabId,
    split: Option<RelocatedSplitIds>,
    transfer: Option<orchbee_workbench_protocol::WorkbenchPanelTransfer>,
) -> Result<WorkbenchPanelRelocateSubmission, WorkbenchPanelRelocateOutcomeUnknownReason> {
    let successor = expected_revision.value().checked_add(1).ok_or(
        WorkbenchPanelRelocateOutcomeUnknownReason::ExpectedRevisionOverflow {
            expected: expected_revision,
        },
    )?;
    let expected = WorkbenchRevision::new(successor);
    if expected != revision {
        return Err(
            WorkbenchPanelRelocateOutcomeUnknownReason::RevisionMismatch {
                expected,
                received: revision,
            },
        );
    }
    if &terminal_context_id != expected_context {
        return Err(
            WorkbenchPanelRelocateOutcomeUnknownReason::TerminalContextMismatch {
                expected: expected_context.clone(),
                received: terminal_context_id,
            },
        );
    }
    if &tab_id != expected_tab {
        return Err(WorkbenchPanelRelocateOutcomeUnknownReason::TabMismatch {
            expected: expected_tab.clone(),
            received: tab_id,
        });
    }
    if &panel_id != expected_panel {
        return Err(WorkbenchPanelRelocateOutcomeUnknownReason::PanelMismatch {
            expected: expected_panel.clone(),
            received: panel_id,
        });
    }
    let (target_context, target_tab, created_workspace) = match expected_destination {
        WorkbenchPanelRelocateDestination::Tab {
            terminal_context_id,
            tab_id,
            ..
        } => (
            Some(terminal_context_id.as_ref().unwrap_or(expected_context)),
            Some(tab_id),
            false,
        ),
        WorkbenchPanelRelocateDestination::NewTab {
            terminal_context_id,
            ..
        } => (
            Some(terminal_context_id.as_ref().unwrap_or(expected_context)),
            None,
            false,
        ),
        WorkbenchPanelRelocateDestination::NewWorkspace { .. } => (None, None, true),
    };
    let received_context = transfer.as_ref().map_or(&terminal_context_id, |transfer| {
        &transfer.target_terminal_context_id
    });
    let created_directory = matches!(
        expected_destination,
        WorkbenchPanelRelocateDestination::NewWorkspace {
            primary_directory_path: Some(_),
            ..
        }
    );
    if target_context.is_some_and(|expected| expected != received_context)
        || (created_workspace && received_context == expected_context)
        || transfer
            .as_ref()
            .and_then(|transfer| transfer.created_directory_id.as_ref())
            .is_some()
            != created_directory
        || transfer
            .as_ref()
            .and_then(|transfer| transfer.created_workspace_id.as_ref())
            .is_some()
            != created_workspace
        || transfer.as_ref().is_some_and(|transfer| {
            transfer.closed_source_workspace_id.is_some()
                && (!transfer.closed_source_tab || received_context == expected_context)
        })
        || split.is_some() != target_tab.is_some()
        || target_tab_id == tab_id
    {
        return Err(WorkbenchPanelRelocateOutcomeUnknownReason::ActionIncompatible);
    }
    if let Some(expected) = target_tab
        && expected != &target_tab_id
    {
        return Err(
            WorkbenchPanelRelocateOutcomeUnknownReason::TargetTabMismatch {
                expected: expected.clone(),
                received: target_tab_id,
            },
        );
    }
    Ok(WorkbenchPanelRelocateSubmission::Committed(
        WorkbenchPanelRelocateCommit {
            revision,
            terminal_context_id,
            tab_id,
            panel_id,
            target_tab_id,
            split,
            transfer,
        },
    ))
}
