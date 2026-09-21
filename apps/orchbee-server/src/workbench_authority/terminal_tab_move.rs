use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, WorkbenchAuthority, WorkbenchGrant,
    WorkbenchTerminalMembership, result, unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchTabId, WorkbenchTabMoveDirection,
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
    direction: WorkbenchTabMoveDirection,
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
        let destination = match self.direction {
            WorkbenchTabMoveDirection::Previous => source.saturating_sub(1),
            WorkbenchTabMoveDirection::Next => (source + 1).min(context.tabs.len() - 1),
            WorkbenchTabMoveDirection::First => 0,
            WorkbenchTabMoveDirection::Last => context.tabs.len() - 1,
        };
        if source == destination {
            return Ok(self.authority.unchanged());
        }
        let mut candidate = self.authority.projection.snapshot().clone();
        let revision = self.authority.next_revision()?;
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

impl WorkbenchAuthority {
    pub fn begin_terminal_tab_move(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchTabMoveBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::TabMove { .. })
            && grant
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
        let WorkbenchAction::TabMove {
            terminal_context_id,
            tab_id,
            direction,
        } = command.action.clone()
        else {
            return WorkbenchTabMoveBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
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
            direction,
        }))
    }
}

#[cfg(test)]
mod tests;
