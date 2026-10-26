//! Validates setup action-result pairing, committed identities and revisions, and
//! action-specific rejection or unavailable classes. It classifies an observed response
//! without executing a command or applying UI state.

use super::{
    WorkbenchSetupCommit, WorkbenchSetupOutcomeUnknownReason, WorkbenchSetupPrecommit,
    WorkbenchSetupSubmission,
};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommandOutcome, WorkbenchCommandUnavailableReason,
    WorkbenchRejectionReason, WorkbenchRevision,
};
#[cfg(test)]
mod idle_tests;
pub(super) fn classify_outcome(
    action: &WorkbenchAction,
    expected_revision: WorkbenchRevision,
    outcome: WorkbenchCommandOutcome,
) -> Result<WorkbenchSetupSubmission, WorkbenchSetupOutcomeUnknownReason> {
    let commit = match (action, outcome) {
        (
            WorkbenchAction::NavigationConfigure { .. },
            WorkbenchCommandOutcome::NavigationConfigured { revision },
        ) => WorkbenchSetupCommit::NavigationConfigured {
            revision,
            unchanged: false,
        },
        (
            WorkbenchAction::NavigationConfigure { .. },
            WorkbenchCommandOutcome::Unchanged { revision },
        ) => WorkbenchSetupCommit::NavigationConfigured {
            revision,
            unchanged: true,
        },
        (
            WorkbenchAction::WorkspaceSetProject {
                workspace_id,
                project_id,
            },
            WorkbenchCommandOutcome::WorkspaceProjectChanged {
                revision,
                workspace_id: workspace,
                project_id: project,
            },
        ) if workspace_id == &workspace && project_id == &project => {
            WorkbenchSetupCommit::WorkspaceProjectChanged {
                revision,
                workspace_id: workspace,
                project_id: project,
                created: false,
                unchanged: false,
            }
        }
        (
            WorkbenchAction::WorkspaceSetProject {
                workspace_id,
                project_id,
            },
            WorkbenchCommandOutcome::Unchanged { revision },
        ) => WorkbenchSetupCommit::WorkspaceProjectChanged {
            revision,
            workspace_id: workspace_id.clone(),
            project_id: project_id.clone(),
            created: false,
            unchanged: true,
        },
        (
            WorkbenchAction::WorkspaceCreateProject { workspace_id, .. },
            WorkbenchCommandOutcome::WorkspaceProjectChanged {
                revision,
                workspace_id: workspace,
                project_id: Some(project),
            },
        ) if workspace_id == &workspace => WorkbenchSetupCommit::WorkspaceProjectChanged {
            revision,
            workspace_id: workspace,
            project_id: Some(project),
            created: true,
            unchanged: false,
        },
        (
            WorkbenchAction::ProjectDissolve { project_id }
            | WorkbenchAction::ProjectDissolveIfIdle { project_id },
            WorkbenchCommandOutcome::ProjectDissolved {
                revision,
                project_id: project,
            },
        ) if project_id == &project => WorkbenchSetupCommit::ProjectDissolved {
            revision,
            project_id: project,
        },
        (
            WorkbenchAction::TabsConfigure { operation, .. },
            WorkbenchCommandOutcome::TabsConfigured { revision, group_id },
        ) if operation.creates_group() == group_id.is_some() => {
            WorkbenchSetupCommit::TabsConfigured {
                revision,
                group_id,
                unchanged: false,
            }
        }
        (
            WorkbenchAction::TabsConfigure { operation, .. },
            WorkbenchCommandOutcome::Unchanged { revision },
        ) if !operation.creates_group() => WorkbenchSetupCommit::TabsConfigured {
            revision,
            group_id: None,
            unchanged: true,
        },
        (_, WorkbenchCommandOutcome::Rejected { revision, reason })
            if compatible_rejection(action, reason)
                && (reason != WorkbenchRejectionReason::ConfirmationRequired
                    || revision == expected_revision) =>
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
            WorkbenchAction::WorkspaceRename { workspace_id, .. },
            outcome @ (WorkbenchCommandOutcome::Renamed { .. }
            | WorkbenchCommandOutcome::Unchanged { .. }),
        ) => {
            let (revision, unchanged) = rename_revision(outcome);
            WorkbenchSetupCommit::WorkspaceRenamed {
                revision,
                workspace_id: workspace_id.clone(),
                unchanged,
            }
        }
        (
            WorkbenchAction::WorkspaceMove { .. } | WorkbenchAction::WorkspaceMoveBlock { .. },
            WorkbenchCommandOutcome::WorkspacesReordered {
                revision,
                workspace_order,
            },
        ) if ordering_outcome_matches(action, &workspace_order) => {
            WorkbenchSetupCommit::WorkspaceOrder {
                revision,
                workspace_order: Some(workspace_order),
            }
        }
        (
            WorkbenchAction::WorkspaceMove { .. } | WorkbenchAction::WorkspaceMoveBlock { .. },
            WorkbenchCommandOutcome::Unchanged { revision },
        ) => WorkbenchSetupCommit::WorkspaceOrder {
            revision,
            workspace_order: None,
        },
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => return Err(WorkbenchSetupOutcomeUnknownReason::ActionIncompatible),
    };
    let unchanged = matches!(
        commit,
        WorkbenchSetupCommit::WorkspaceProjectChanged {
            unchanged: true,
            ..
        } | WorkbenchSetupCommit::NavigationConfigured {
            unchanged: true,
            ..
        } | WorkbenchSetupCommit::TabsConfigured {
            unchanged: true,
            ..
        } | WorkbenchSetupCommit::WorkspaceOrder {
            workspace_order: None,
            ..
        } | WorkbenchSetupCommit::ProjectRenamed {
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

fn ordering_outcome_matches(
    action: &WorkbenchAction,
    order: &orchbee_workbench_protocol::WorkbenchWorkspaceList,
) -> bool {
    let ids = order.ids();
    match action {
        WorkbenchAction::WorkspaceMove {
            workspace_id,
            insert_index,
        } => {
            let boundary = insert_index.value();
            boundary <= ids.len()
                && ids
                    .iter()
                    .position(|id| id == workspace_id)
                    .is_some_and(|position| {
                        position == boundary || position.checked_add(1) == Some(boundary)
                    })
        }
        WorkbenchAction::WorkspaceMoveBlock {
            workspace_ids,
            before_workspace_id,
        } => {
            let Some(start) = ids
                .windows(workspace_ids.ids().len())
                .position(|window| window == workspace_ids.ids())
            else {
                return false;
            };
            let end = start + workspace_ids.ids().len();
            match before_workspace_id {
                Some(before) => ids.get(end) == Some(before),
                None => end == ids.len(),
            }
        }
        // This postcondition checks only the two Workspace move operations' insertion boundaries.
        _ => false,
    }
}

fn compatible_rejection(action: &WorkbenchAction, reason: WorkbenchRejectionReason) -> bool {
    match reason {
        WorkbenchRejectionReason::SessionMismatch
        | WorkbenchRejectionReason::RevisionMismatch
        | WorkbenchRejectionReason::ResourceLimit
        | WorkbenchRejectionReason::RevisionExhausted
        | WorkbenchRejectionReason::RequestIdentityConflict => true,
        WorkbenchRejectionReason::ConfirmationRequired => {
            matches!(action, WorkbenchAction::ProjectDissolveIfIdle { .. })
        }
        WorkbenchRejectionReason::IdentityExhausted => matches!(
            action,
            WorkbenchAction::ProjectCreate { .. }
                | WorkbenchAction::WorkspaceCreateProject { .. }
                | WorkbenchAction::TabsConfigure { .. }
                | WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::InvalidNavigationConfiguration => {
            matches!(action, WorkbenchAction::NavigationConfigure { .. })
        }
        WorkbenchRejectionReason::ProjectUnavailable => matches!(
            action,
            WorkbenchAction::WorkspaceCreate { .. }
                | WorkbenchAction::NavigationConfigure { .. }
                | WorkbenchAction::WorkspaceSetProject { .. }
                | WorkbenchAction::ProjectDissolve { .. }
                | WorkbenchAction::ProjectDissolveIfIdle { .. }
                | WorkbenchAction::ProjectRename { .. }
                | WorkbenchAction::WorkspaceRename { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceMoveIndexOutOfBounds => {
            matches!(action, WorkbenchAction::WorkspaceMove { .. })
        }
        WorkbenchRejectionReason::WorkspaceMoveTargetInBlock => {
            matches!(action, WorkbenchAction::WorkspaceMoveBlock { .. })
        }
        WorkbenchRejectionReason::WorkspaceUnavailable => matches!(
            action,
            WorkbenchAction::WorkspaceRename { .. }
                | WorkbenchAction::NavigationConfigure { .. }
                | WorkbenchAction::WorkspaceSetProject { .. }
                | WorkbenchAction::WorkspaceCreateProject { .. }
                | WorkbenchAction::WorkspaceMove { .. }
                | WorkbenchAction::WorkspaceMoveBlock { .. }
                | WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceDirectoryConflict => matches!(
            action,
            WorkbenchAction::WorkspacePrimaryDirectoryAccept { .. }
        ),
        WorkbenchRejectionReason::WorkspaceGroupCloseRequired
        | WorkbenchRejectionReason::WorkspaceDirectoryUnavailable
        | WorkbenchRejectionReason::TabMoveIndexOutOfBounds
        | WorkbenchRejectionReason::TabMoveOrderConflict
        | WorkbenchRejectionReason::PanelUnavailable
        | WorkbenchRejectionReason::SameTab => false,
        WorkbenchRejectionReason::TerminalContextUnavailable
        | WorkbenchRejectionReason::TabUnavailable
        | WorkbenchRejectionReason::TabGroupUnavailable => {
            matches!(action, WorkbenchAction::TabsConfigure { .. })
        }
        WorkbenchRejectionReason::SplitUnavailable => false,
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
        WorkbenchCommandUnavailableReason::TerminalBusy => {
            matches!(
                action,
                WorkbenchAction::TabsConfigure { .. }
                    | WorkbenchAction::NavigationConfigure { .. }
                    | WorkbenchAction::WorkspaceSetProject { .. }
                    | WorkbenchAction::WorkspaceCreateProject { .. }
                    | WorkbenchAction::ProjectDissolve { .. }
                    | WorkbenchAction::ProjectDissolveIfIdle { .. }
            )
        }
        WorkbenchCommandUnavailableReason::TerminalRuntimeUnavailable => {
            matches!(action, WorkbenchAction::ProjectDissolveIfIdle { .. })
        }
    }
}

fn rename_revision(outcome: WorkbenchCommandOutcome) -> (WorkbenchRevision, bool) {
    match outcome {
        WorkbenchCommandOutcome::Renamed { revision } => (revision, false),
        WorkbenchCommandOutcome::Unchanged { revision } => (revision, true),
        // This verifier is scoped to its original mutation; a different command receipt cannot prove its postconditions.
        _ => unreachable!("rename result arm only admits Renamed or Unchanged"),
    }
}
