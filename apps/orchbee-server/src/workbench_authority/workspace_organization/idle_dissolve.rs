//! Conditional organization-only deletion at the canonical authority gate.
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchRejectionReason,
};

use super::super::{
    PlanFailure, PreparedCommandCommit, PureWorkbenchAction, WorkbenchAuthority, WorkbenchGrant,
    WorkbenchTabCloseMemberRuntime, WorkbenchTabCloseRuntimeUnavailableReason,
    WorkbenchTerminalTab, result, unavailable,
};

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchProjectDissolveBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchProjectDissolveAdmission<'a>>),
}

/// Holds the exact complete member set and unpublished organization plan. Its
/// exclusive authority borrow prevents membership or pending-create drift while
/// the Site resolves each binding through its terminal coordinator.
pub struct WorkbenchProjectDissolveAdmission<'a> {
    prepared: PreparedCommandCommit<'a>,
    members: Vec<WorkbenchTerminalTab>,
}

impl WorkbenchProjectDissolveAdmission<'_> {
    #[must_use]
    pub fn members(&self) -> &[WorkbenchTerminalTab] {
        &self.members
    }

    #[must_use]
    pub fn runtime_unavailable(
        self,
        reason: WorkbenchTabCloseRuntimeUnavailableReason,
    ) -> WorkbenchCommandResult {
        self.prepared
            .authority
            .retain_outcome(self.prepared.retained.command, unavailable(reason.into()))
    }

    /// An absent proof, omitted child or substituted binding is an invariant
    /// failure. A real runtime is not evidence of current process exit.
    #[must_use]
    pub fn finish_resolved_members(
        self,
        runtimes: Vec<WorkbenchTabCloseMemberRuntime>,
    ) -> WorkbenchCommandResult {
        if runtimes.len() != self.members.len()
            || self
                .members
                .iter()
                .zip(&runtimes)
                .any(|(member, runtime)| !runtime.matches_member(member))
        {
            return self
                .prepared
                .authority
                .retire(&self.prepared.retained.command);
        }
        if runtimes
            .iter()
            .any(|runtime| !matches!(runtime, WorkbenchTabCloseMemberRuntime::Uncreated { .. }))
        {
            return self.prepared.authority.retain_rejection(
                self.prepared.retained.command,
                WorkbenchRejectionReason::ConfirmationRequired,
            );
        }
        self.prepared.commit()
    }
}

impl WorkbenchAuthority {
    pub fn begin_project_dissolve_if_idle(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchProjectDissolveBegin<'_> {
        let authorized = matches!(
            &command.action,
            WorkbenchAction::ProjectDissolveIfIdle { .. }
        ) && grant
            .is_some_and(|grant| grant.site_id == *self.site_id() && grant.mutate_navigation);
        if !authorized {
            return WorkbenchProjectDissolveBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(result) = self.command_preflight(&command) {
            return WorkbenchProjectDissolveBegin::Ready(result);
        }
        let WorkbenchAction::ProjectDissolveIfIdle { project_id } = &command.action else {
            return WorkbenchProjectDissolveBegin::Ready(self.retire(&command));
        };
        let plan = match self.plan(
            &command,
            PureWorkbenchAction::ProjectDissolve {
                project_id: project_id.clone(),
            },
        ) {
            Ok(plan) => plan,
            Err(PlanFailure::Rejected(reason)) => {
                return WorkbenchProjectDissolveBegin::Ready(
                    self.retain_rejection(command, reason),
                );
            }
            Err(PlanFailure::Internal) => {
                return WorkbenchProjectDissolveBegin::Ready(self.retire(&command));
            }
        };
        let contexts: Vec<_> = self
            .snapshot()
            .navigation
            .workspaces_in_project(Some(project_id))
            .map(|workspace| workspace.terminal_context_id.clone())
            .collect();
        let mut members = Vec::new();
        for context in contexts {
            // A prepared create can owe this Context a member without advancing
            // the projection revision. Revision CAS alone cannot prove idle.
            if self.pending_terminal_covers_context(&context) {
                return WorkbenchProjectDissolveBegin::Ready(self.retain_outcome(
                    command,
                    unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
                ));
            }
            let Ok(membership) = self.presentation_membership(&context) else {
                return WorkbenchProjectDissolveBegin::Ready(self.retire(&command));
            };
            members.extend_from_slice(membership.members());
        }
        match self.prepare_command_commit(command, plan) {
            Ok(prepared) => WorkbenchProjectDissolveBegin::Admitted(Box::new(
                WorkbenchProjectDissolveAdmission { prepared, members },
            )),
            Err(result) => WorkbenchProjectDissolveBegin::Ready(*result),
        }
    }
}
