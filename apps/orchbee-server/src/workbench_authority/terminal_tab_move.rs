use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, WorkbenchAuthority, WorkbenchGrant,
    WorkbenchTerminalMembership, result, unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchTabId, WorkbenchTabInsertIndex, WorkbenchTabMoveDirection,
    WorkbenchTerminalContextId, serialize_document,
};

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabMoveBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchTabMoveAdmission<'a>>),
}

pub struct WorkbenchTabMoveAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    terminal_context_id: WorkbenchTerminalContextId,
    membership: Option<WorkbenchTerminalMembership>,
    tab_id: WorkbenchTabId,
    placement: TabPlacement,
}

#[derive(Clone, Copy)]
enum TabPlacement {
    Direction(WorkbenchTabMoveDirection),
    InsertIndex(WorkbenchTabInsertIndex),
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabMovePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchTabMove<'a>>),
}

pub struct PreparedWorkbenchTabMove<'a>(PreparedCommandCommit<'a>);

impl PreparedWorkbenchTabMove<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.0.response
    }

    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.0.commit()
    }
}

impl<'a> WorkbenchTabMoveAdmission<'a> {
    #[must_use]
    pub fn standalone_membership(&self) -> Option<&WorkbenchTerminalMembership> {
        self.membership.as_ref()
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Site validates standalone bootstrap composition before resolving its target or no-op.
    #[must_use]
    pub fn finish(self) -> WorkbenchTabMovePreparation<'a> {
        let plan = match self.plan() {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => CommitPlan {
                projection: None,
                last_identity: None,
                catalog_next: None,
                outcome: self.authority.rejected(reason),
            },
            Err(PlanFailure::Internal) => {
                return WorkbenchTabMovePreparation::Ready(self.authority.retire(&self.command));
            }
        };
        match self.authority.prepare_command_commit(self.command, plan) {
            Ok(prepared) => {
                WorkbenchTabMovePreparation::Prepared(Box::new(PreparedWorkbenchTabMove(prepared)))
            }
            Err(result) => WorkbenchTabMovePreparation::Ready(*result),
        }
    }

    fn plan(&self) -> Result<CommitPlan, PlanFailure> {
        let context = self
            .authority
            .projection
            .snapshot()
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        let source = context
            .tabs
            .iter()
            .position(|tab| tab.tab_id == self.tab_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::TabUnavailable,
            ))?;
        // Exact target lookup establishes a nonempty vector, bounded by the validated projection.
        let destination = match self.placement {
            TabPlacement::Direction(direction) => match direction {
                WorkbenchTabMoveDirection::Previous => source.saturating_sub(1),
                WorkbenchTabMoveDirection::Next => (source + 1).min(context.tabs.len() - 1),
                WorkbenchTabMoveDirection::First => 0,
                WorkbenchTabMoveDirection::Last => context.tabs.len() - 1,
            },
            TabPlacement::InsertIndex(index) => {
                let boundary = index.value();
                if boundary > context.tabs.len() {
                    return Err(PlanFailure::Rejected(
                        WorkbenchRejectionReason::TabMoveIndexOutOfBounds,
                    ));
                }
                if source < boundary {
                    boundary - 1
                } else {
                    boundary
                }
            }
        };
        if source == destination {
            return Ok(self.authority.unchanged());
        }
        let mut candidate = self.authority.projection.snapshot().clone();
        let context = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| context.terminal_context_id == self.terminal_context_id)
            .ok_or(PlanFailure::Internal)?;
        let moved = context
            .tabs
            .get_mut(source.min(destination)..=source.max(destination))
            .ok_or(PlanFailure::Internal)?;
        if source < destination {
            moved.rotate_left(1);
        } else {
            moved.rotate_right(1);
        }
        if !preserves_tab_order_constraints(&context.tabs) {
            return Err(PlanFailure::Rejected(
                WorkbenchRejectionReason::TabMoveOrderConflict,
            ));
        }
        let revision = self.authority.next_revision()?;
        candidate.revision = revision;
        let encoded = serialize_document(&WorkbenchProtocolDocument::Snapshot(candidate)).map_err(
            |error| match error {
                ContractError::EncodedValueTooLarge { .. } => {
                    PlanFailure::Rejected(WorkbenchRejectionReason::ResourceLimit)
                }
                _ => PlanFailure::Internal,
            },
        )?;
        let projection = self
            .authority
            .projection
            .prepare_advance(&encoded)
            .map_err(|_error| PlanFailure::Internal)?;
        Ok(CommitPlan {
            projection: Some(projection),
            last_identity: None,
            catalog_next: None,
            outcome: WorkbenchCommandOutcome::TabMoved {
                revision,
                terminal_context_id: self.terminal_context_id.clone(),
                tab_id: self.tab_id.clone(),
            },
        })
    }
}

/// Order-only moves cannot implicitly unpin or regroup a Tab. Input metadata
/// already passed canonical validation; only its permutation changes here.
fn preserves_tab_order_constraints(tabs: &[orchbee_workbench_protocol::Tab]) -> bool {
    let mut unpinned = false;
    let mut previous_group = None;
    let mut completed_groups = std::collections::HashSet::new();
    for tab in tabs {
        if tab.pinned && unpinned {
            return false;
        }
        unpinned |= !tab.pinned;
        let group = tab.group_id.as_ref();
        if group != previous_group {
            if let Some(previous) = previous_group {
                completed_groups.insert(previous);
            }
            if group.is_some_and(|group| completed_groups.contains(group)) {
                return false;
            }
            previous_group = group;
        }
    }
    true
}

impl WorkbenchAuthority {
    pub fn begin_terminal_tab_move(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchTabMoveBegin<'_> {
        let authorized = matches!(
            &command.action,
            WorkbenchAction::TabMove { .. } | WorkbenchAction::TabMoveIndex { .. }
        ) && grant
            .is_some_and(|grant| grant.site_id == *self.site_id() && grant.mutate_navigation);
        if !authorized {
            return WorkbenchTabMoveBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchTabMoveBegin::Ready(result);
        }
        let (terminal_context_id, tab_id, placement) = match command.action.clone() {
            WorkbenchAction::TabMove {
                terminal_context_id,
                tab_id,
                direction,
            } => (
                terminal_context_id,
                tab_id,
                TabPlacement::Direction(direction),
            ),
            WorkbenchAction::TabMoveIndex {
                terminal_context_id,
                tab_id,
                insert_index,
            } => (
                terminal_context_id,
                tab_id,
                TabPlacement::InsertIndex(insert_index),
            ),
            _ => {
                return WorkbenchTabMoveBegin::Ready(result(
                    &command,
                    unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
                ));
            }
        };
        let snapshot = self.projection.snapshot();
        let rejection = if command.site_id != snapshot.site_id {
            Some(WorkbenchRejectionReason::SiteMismatch)
        } else if command.expected_revision != snapshot.revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else if terminal_context_id != snapshot.navigation.standalone_terminal_context_id
            && !snapshot.navigation.projects.iter().any(|project| {
                project
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.terminal_context_id == terminal_context_id)
            })
        {
            Some(WorkbenchRejectionReason::TerminalContextUnavailable)
        } else {
            None
        };
        if let Some(reason) = rejection {
            return WorkbenchTabMoveBegin::Ready(self.retain_rejection(command, reason));
        }
        // An open Context Close fences every membership change in its
        // Terminal Context: a reorder would invalidate the captured ordered
        // re-proof its publication performs.
        if self.open_close_fences_context(&terminal_context_id) {
            return WorkbenchTabMoveBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        let membership = if terminal_context_id
            == snapshot.navigation.standalone_terminal_context_id
        {
            match self.tree_standalone_membership() {
                Ok(membership) => Some(membership),
                Err(_invariant) => return WorkbenchTabMoveBegin::Ready(self.retire(&command)),
            }
        } else {
            if super::terminal_membership::tree_terminal_membership(snapshot, &terminal_context_id)
                .is_err()
            {
                return WorkbenchTabMoveBegin::Ready(self.retire(&command));
            }
            None
        };
        WorkbenchTabMoveBegin::Admitted(Box::new(WorkbenchTabMoveAdmission {
            authority: self,
            command,
            terminal_context_id,
            membership,
            tab_id,
            placement,
        }))
    }
}

#[cfg(test)]
mod tests;
