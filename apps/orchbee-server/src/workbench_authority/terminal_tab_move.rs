use super::{
    CommitPlan, PlanFailure, PreparedCommandCommit, WorkbenchAuthority, WorkbenchGrant,
    WorkbenchStandaloneMembership, result, unavailable,
};
use orchbee_workbench_protocol::{
    ContractError, WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome,
    WorkbenchCommandResult, WorkbenchCommandUnavailableReason, WorkbenchProtocolDocument,
    WorkbenchRejectionReason, WorkbenchTabId, WorkbenchTabMoveDirection, serialize_document,
};

pub enum WorkbenchTabMoveBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchTabMoveAdmission<'a>>),
}

pub struct WorkbenchTabMoveAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    membership: WorkbenchStandaloneMembership,
    tab_id: WorkbenchTabId,
    direction: WorkbenchTabMoveDirection,
}

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
    pub fn standalone_membership(&self) -> &WorkbenchStandaloneMembership {
        &self.membership
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Site validates its retained bootstrap composition before resolving the target or no-op.
    #[must_use]
    pub fn finish(self) -> WorkbenchTabMovePreparation<'a> {
        let plan = match self.plan() {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => CommitPlan {
                projection: None,
                last_identity: None,
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
        let members = self.membership.members();
        let source = members
            .iter()
            .position(|member| member.tab_id() == &self.tab_id)
            .ok_or(PlanFailure::Rejected(
                WorkbenchRejectionReason::TabUnavailable,
            ))?;
        // Exact target lookup establishes a nonempty vector, bounded by the validated projection.
        let destination = match self.direction {
            WorkbenchTabMoveDirection::Previous => source.saturating_sub(1),
            WorkbenchTabMoveDirection::Next => (source + 1).min(members.len() - 1),
            WorkbenchTabMoveDirection::First => 0,
            WorkbenchTabMoveDirection::Last => members.len() - 1,
        };
        if source == destination {
            return Ok(self.authority.unchanged());
        }
        let mut candidate = self.authority.projection.snapshot().clone();
        let revision = self.authority.next_revision()?;
        let context = candidate
            .terminal_contexts
            .iter_mut()
            .find(|context| &context.terminal_context_id == self.membership.terminal_context_id())
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
            outcome: WorkbenchCommandOutcome::TabMoved {
                revision,
                terminal_context_id: self.membership.terminal_context_id().clone(),
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
        } else if terminal_context_id != snapshot.navigation.standalone_terminal_context_id {
            Some(WorkbenchRejectionReason::TerminalContextUnavailable)
        } else {
            None
        };
        if let Some(reason) = rejection {
            return WorkbenchTabMoveBegin::Ready(self.retain_rejection(command, reason));
        }
        let membership = match self.standalone_membership() {
            Ok(membership) => membership,
            Err(_invariant) => return WorkbenchTabMoveBegin::Ready(self.retire(&command)),
        };
        WorkbenchTabMoveBegin::Admitted(Box::new(WorkbenchTabMoveAdmission {
            authority: self,
            command,
            membership,
            tab_id,
            direction,
        }))
    }
}

#[cfg(test)]
mod tests;
