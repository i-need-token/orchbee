//! Resolves a Workspace display-order action against one admitted navigation snapshot while
//! preserving its complete membership. It returns a pure candidate without reserving the
//! ledger, advancing revisions, or publishing state.

use super::{PlanFailure, PureWorkbenchAction};
use orchbee_workbench_protocol::{Navigation, WorkbenchRejectionReason, WorkbenchWorkspaceList};
use std::collections::HashSet;

/// Resolve against one admitted snapshot. Nothing is published until the parent
/// authority has reserved the result ledger and prepared the complete projection.
pub(super) fn reorder(
    navigation: &Navigation,
    action: &PureWorkbenchAction,
) -> Result<Option<WorkbenchWorkspaceList>, PlanFailure> {
    let original: Vec<_> = navigation
        .ordered_workspaces()
        .map_err(|_| PlanFailure::Internal)?
        .into_iter()
        .map(|(_, workspace)| workspace.workspace_id.clone())
        .collect();
    let mut candidate = original.clone();
    match action {
        PureWorkbenchAction::WorkspaceMove {
            workspace_id,
            insert_index,
        } => {
            let source =
                original
                    .iter()
                    .position(|id| id == workspace_id)
                    .ok_or(PlanFailure::Rejected(
                        WorkbenchRejectionReason::WorkspaceUnavailable,
                    ))?;
            let boundary = insert_index.value();
            if boundary > original.len() {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::WorkspaceMoveIndexOutOfBounds,
                ));
            }
            let destination = boundary - usize::from(source < boundary);
            let moved = candidate.remove(source);
            candidate.insert(destination, moved);
        }
        PureWorkbenchAction::WorkspaceMoveBlock {
            workspace_ids,
            before_workspace_id,
        } => {
            let members: HashSet<_> = original.iter().collect();
            let sources: HashSet<_> = workspace_ids.ids().iter().collect();
            if sources.iter().any(|id| !members.contains(id))
                || before_workspace_id
                    .as_ref()
                    .is_some_and(|id| !members.contains(id))
            {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::WorkspaceUnavailable,
                ));
            }
            if before_workspace_id
                .as_ref()
                .is_some_and(|id| sources.contains(id))
            {
                return Err(PlanFailure::Rejected(
                    WorkbenchRejectionReason::WorkspaceMoveTargetInBlock,
                ));
            }
            candidate.retain(|id| !sources.contains(id));
            let index = match before_workspace_id {
                Some(before) => candidate
                    .iter()
                    .position(|id| id == before)
                    .ok_or(PlanFailure::Internal)?,
                None => candidate.len(),
            };
            candidate.splice(index..index, workspace_ids.ids().iter().cloned());
        }
        _ => return Err(PlanFailure::Internal),
    }
    if candidate == original {
        return Ok(None);
    }
    WorkbenchWorkspaceList::new(candidate)
        .map(Some)
        .map_err(|_| PlanFailure::Internal)
}
