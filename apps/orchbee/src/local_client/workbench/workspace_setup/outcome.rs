use super::{
    WorkbenchSetupCommit, WorkbenchSetupOutcomeUnknownReason, WorkbenchSetupPrecommit,
    WorkbenchSetupSubmission,
};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchRejectionReason, WorkbenchRevision,
};
pub(super) fn classify_outcome(
    action: &WorkbenchAction,
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchSetupSubmission, WorkbenchSetupOutcomeUnknownReason> {
    let commit = match (action, outcome) {
        (_, WorkbenchCommandOutcome::Rejected { revision, reason })
            if compatible_rejection(action, reason) =>
        {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Rejected { revision, reason },
            ));
        }
        (_, WorkbenchCommandOutcome::Denied {}) => {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Denied,
            ));
        }
        (_, WorkbenchCommandOutcome::Unavailable { reason })
            if compatible_unavailable(action, reason) =>
        {
            return Ok(WorkbenchSetupSubmission::DefinitePrecommit(
                WorkbenchSetupPrecommit::Unavailable { reason },
            ));
        }
        (
            WorkbenchAction::ProjectCreate { .. },
            WorkbenchCommandOutcome::ProjectCreated {
                revision,
                project_id,
            },
        ) => WorkbenchSetupCommit::ProjectCreated {
            revision,
            project_id,
        },
        (
            WorkbenchAction::WorkspaceCreate { .. },
            WorkbenchCommandOutcome::WorkspaceCreated {
                revision,
                workspace_id,
                terminal_context_id,
            },
        ) => WorkbenchSetupCommit::WorkspaceCreated {
            revision,
            workspace_id,
            terminal_context_id,
        },
        (
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. },
            WorkbenchCommandOutcome::WorkspacePrimaryDirectoryAccepted {
                revision,
                directory_id,
                worker_id,
            },
        ) => WorkbenchSetupCommit::WorkspacePrimaryDirectoryAccepted {
            revision,
            directory_id,
            worker_id,
        },
        (
            WorkbenchAction::ProjectRename { project_id, .. },
            outcome @ (WorkbenchCommandOutcome::Renamed { .. }
            | WorkbenchCommandOutcome::Unchanged { .. }),
        ) => {
            let (revision, unchanged) = rename_revision(outcome);
            WorkbenchSetupCommit::ProjectRenamed {
                revision,
                project_id: project_id.clone(),
                unchanged,
            }
        }
        (
            WorkbenchAction::WorkspaceRename {
                project_id,
                workspace_id,
                ..
            },
            outcome @ (WorkbenchCommandOutcome::Renamed { .. }
            | WorkbenchCommandOutcome::Unchanged { .. }),
        ) => {
            let (revision, unchanged) = rename_revision(outcome);
            WorkbenchSetupCommit::WorkspaceRenamed {
                revision,
                project_id: project_id.clone(),
                workspace_id: workspace_id.clone(),
                unchanged,
            }
        }
        _ => return Err(WorkbenchSetupOutcomeUnknownReason::ActionIncompatible),
    };
    let unchanged = matches!(
        commit,
        WorkbenchSetupCommit::ProjectRenamed {
            unchanged: true,
            ..
        } | WorkbenchSetupCommit::WorkspaceRenamed {
            unchanged: true,
            ..
        }
    );
    let expected = if unchanged {
        expected_revision
    } else {
        WorkbenchRevision::new(expected_revision.value().checked_add(1).ok_or(
            WorkbenchSetupOutcomeUnknownReason::ExpectedRevisionOverflow {
                expected: expected_revision,
            },
        )?)
    };
    let received = commit.revision();
    if expected != received {
        return Err(WorkbenchSetupOutcomeUnknownReason::RevisionMismatch { expected, received });
    }
    Ok(WorkbenchSetupSubmission::Committed(commit))
}

fn compatible_rejection(action: &WorkbenchAction, reason: WorkbenchRejectionReason) -> bool {
    match reason {
        WorkbenchRejectionReason::SiteMismatch
        | WorkbenchRejectionReason::RevisionMismatch
        | WorkbenchRejectionReason::ResourceLimit
        | WorkbenchRejectionReason::RevisionExhausted
        | WorkbenchRejectionReason::RequestIdentityConflict => true,
        WorkbenchRejectionReason::IdentityExhausted => matches!(
            action,
            WorkbenchAction::ProjectCreate { .. }
                | WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::ProjectUnavailable => matches!(
            action,
            WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::ProjectRename { .. }
                | WorkbenchAction::WorkspaceRename { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceUnavailable => matches!(
            action,
            WorkbenchAction::WorkspaceRename { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceDirectoryConflict => matches!(
            action,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceGroupCloseRequired
        | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
        | WorkbenchRejectionReason::TerminalContextUnavailable
        | WorkbenchRejectionReason::TabUnavailable
        | WorkbenchRejectionReason::PanelUnavailable
        | WorkbenchRejectionReason::PanelNotLeaf
        | WorkbenchRejectionReason::LastPanelInTab
        | WorkbenchRejectionReason::SameTab => false,
    }
}

fn compatible_unavailable(
    action: &WorkbenchAction,
    reason: WorkbenchCommandUnavailableReason,
) -> bool {
    match reason {
        WorkbenchCommandUnavailableReason::RequestLedgerFull
        | WorkbenchCommandUnavailableReason::ServiceUnavailable => true,
        WorkbenchCommandUnavailableReason::NoCompatibleWorker
        | WorkbenchCommandUnavailableReason::ServerBusy
        | WorkbenchCommandUnavailableReason::DirectoryUnavailable => matches!(
            action,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchCommandUnavailableReason::TerminalBusy
        | WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable => false,
    }
}

fn rename_revision(outcome: WorkbenchCommandOutcome) -> (WorkbenchRevision, bool) {
    match outcome {
        WorkbenchCommandOutcome::Renamed { revision } => (revision, false),
        WorkbenchCommandOutcome::Unchanged { revision } => (revision, true),
        _ => unreachable!("rename result arm only admits Renamed or Unchanged"),
    }
}
