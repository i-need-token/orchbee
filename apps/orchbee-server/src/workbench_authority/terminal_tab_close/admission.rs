use std::time::Instant;

use orchbee_capability_node_protocol::LifecycleTerminalPhase;
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandOutcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason, WorkbenchRejectionReason,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabId,
};

use super::super::{
    CommitPlan, MAXIMUM_LEDGER_BYTES, MAXIMUM_LEDGER_ENTRIES, PlanFailure, RetainedRequest,
    WorkbenchAuthority, WorkbenchGrant, WorkbenchTerminalMembership, result, unavailable,
};
use super::{
    CloseContinuation, CloseRecord, WorkbenchTabCloseRuntimeTarget, WorkbenchTabCloseTarget,
};

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabCloseBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchTabCloseAdmission<'a>>),
}

pub struct WorkbenchTabCloseAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    membership: WorkbenchTerminalMembership,
    tab_id: WorkbenchTabId,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabCloseResolution<'a> {
    Ready(WorkbenchCommandResult),
    Primary(Box<WorkbenchTabClosePrimaryAdmission<'a>>),
}

pub struct WorkbenchTabClosePrimaryAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    membership: WorkbenchTerminalMembership,
    members: Vec<super::super::WorkbenchTerminalTab>,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchTabClosePreparation<'a> {
    Ready(WorkbenchCommandResult),
    Prepared(Box<PreparedWorkbenchTabClose<'a>>),
}

pub struct PreparedWorkbenchTabClose<'a> {
    authority: &'a mut WorkbenchAuthority,
    record: Box<CloseRecord>,
    next_ledger_bytes: usize,
    response: WorkbenchCommandResult,
    request_id: orchbee_workbench_protocol::WorkbenchRequestId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchTabCloseRuntimeUnavailableReason {
    TerminalBusy,
    TerminalRuntimeUnavailable,
    ServerBusy,
}

impl From<WorkbenchTabCloseRuntimeUnavailableReason> for WorkbenchCommandUnavailableReason {
    fn from(value: WorkbenchTabCloseRuntimeUnavailableReason) -> Self {
        match value {
            WorkbenchTabCloseRuntimeUnavailableReason::TerminalBusy => Self::TerminalBusy,
            WorkbenchTabCloseRuntimeUnavailableReason::TerminalRuntimeUnavailable => {
                Self::TerminalRuntimeUnavailable
            }
            WorkbenchTabCloseRuntimeUnavailableReason::ServerBusy => Self::ServerBusy,
        }
    }
}

impl<'a> WorkbenchTabCloseAdmission<'a> {
    #[must_use]
    pub fn terminal_membership(&self) -> &WorkbenchTerminalMembership {
        &self.membership
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// Site first validates standalone reference-derived composition when applicable.
    #[must_use]
    pub fn resolve_after_composition(self) -> WorkbenchTabCloseResolution<'a> {
        let members: Vec<_> = self
            .membership
            .members_for_tab(&self.tab_id)
            .cloned()
            .collect();
        if members.is_empty() {
            return WorkbenchTabCloseResolution::Ready(
                self.authority
                    .retain_rejection(self.command, WorkbenchRejectionReason::TabUnavailable),
            );
        }
        let existing = self
            .authority
            .ledger
            .values()
            .find_map(|entry| match entry {
                RetainedRequest::TabClose(record)
                    if record.target.terminal_context_id
                        == *self.membership.terminal_context_id()
                        && record.target.member.tab_id() == &self.tab_id =>
                {
                    Some(record)
                }
                _ => None,
            });
        if let Some(record) = existing {
            if record.is_closed() || !record.matches_members(&members) {
                return WorkbenchTabCloseResolution::Ready(self.authority.retire(&self.command));
            }
            // Joining retained facts is effect-free even when the runtime is now unavailable.
            let plan = CommitPlan {
                projection: None,
                last_identity: None,
                outcome: record.result.outcome.clone(),
            };
            return WorkbenchTabCloseResolution::Ready(
                match self.authority.prepare_command_commit(self.command, plan) {
                    Ok(prepared) => prepared.commit(),
                    Err(response) => *response,
                },
            );
        }
        WorkbenchTabCloseResolution::Primary(Box::new(WorkbenchTabClosePrimaryAdmission {
            authority: self.authority,
            command: self.command,
            membership: self.membership,
            members,
        }))
    }
}

impl<'a> WorkbenchTabClosePrimaryAdmission<'a> {
    #[must_use]
    pub fn member(&self) -> &super::super::WorkbenchTerminalTab {
        self.members.first().expect("Close membership is nonempty")
    }

    #[must_use]
    pub fn members(&self) -> &[super::super::WorkbenchTerminalTab] {
        &self.members
    }

    #[must_use]
    pub fn runtime_unavailable(
        self,
        reason: WorkbenchTabCloseRuntimeUnavailableReason,
    ) -> WorkbenchCommandResult {
        result(&self.command, unavailable(reason.into()))
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// The Site supplies an exact retained association and a fresh monotonic acceptance sample.
    #[must_use]
    pub fn finish(
        self,
        runtime: WorkbenchTabCloseRuntimeTarget,
        observed_phase: Option<LifecycleTerminalPhase>,
        now: Instant,
    ) -> WorkbenchTabClosePreparation<'a> {
        self.finish_members(vec![(runtime, observed_phase)], now)
    }

    /// The Site supplies one exact runtime observation for every frozen leaf,
    /// in the deterministic membership order.
    #[must_use]
    pub fn finish_members(
        self,
        runtimes: Vec<(
            WorkbenchTabCloseRuntimeTarget,
            Option<LifecycleTerminalPhase>,
        )>,
        now: Instant,
    ) -> WorkbenchTabClosePreparation<'a> {
        if runtimes.len() != self.members.len()
            || self
                .members
                .iter()
                .zip(&runtimes)
                .any(|(member, (runtime, _))| !runtime.matches_member(member))
        {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        }
        if let Err(failure) = self.authority.next_revision() {
            return WorkbenchTabClosePreparation::Ready(match failure {
                PlanFailure::Rejected(reason) => {
                    self.authority.retain_rejection(self.command, reason)
                }
                PlanFailure::Internal => self.authority.retire(&self.command),
            });
        }
        let Ok(generation) = WorkbenchTabCloseAttemptGeneration::new(1) else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        };
        let response = result(
            &self.command,
            WorkbenchCommandOutcome::TabCloseAccepted {
                close_request_id: self.command.request_id.clone(),
                terminal_context_id: self.membership.terminal_context_id().clone(),
                tab_id: self.member().tab_id().clone(),
                accepted_revision: self.command.expected_revision,
            },
        );
        let Some(reserved_bytes) =
            super::reservation::maximum_primary_charge(&self.command, &response)
        else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        };
        let Some(next_ledger_bytes) = self.authority.ledger_bytes.checked_add(reserved_bytes)
        else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        };
        if next_ledger_bytes > MAXIMUM_LEDGER_BYTES {
            return WorkbenchTabClosePreparation::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let request_id = self.command.request_id.clone();
        let mut targets = Vec::with_capacity(self.members.len());
        let mut continuations = Vec::with_capacity(self.members.len());
        for (member, (runtime, observed_phase)) in self.members.into_iter().zip(runtimes) {
            let Some(continuation) = CloseContinuation::prepare(now, observed_phase) else {
                return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
            };
            targets.push(WorkbenchTabCloseTarget {
                terminal_context_id: self.membership.terminal_context_id().clone(),
                member,
                runtime,
            });
            continuations.push(continuation);
        }
        let mut targets = targets.into_iter();
        let mut continuations = continuations.into_iter();
        let target = targets.next().expect("Close membership is nonempty");
        let continuation = continuations.next().expect("Close membership is nonempty");
        let record = Box::new(CloseRecord {
            command: self.command,
            result: response.clone(),
            target,
            additional_targets: targets.collect(),
            generation,
            resume_receipt: None,
            reserved_bytes,
            continuation,
            additional_continuations: continuations.collect(),
            scheduler_cursor: 0,
            closed_revision: None,
        });
        WorkbenchTabClosePreparation::Prepared(Box::new(PreparedWorkbenchTabClose {
            authority: self.authority,
            record,
            next_ledger_bytes,
            response,
            request_id,
        }))
    }
}

impl PreparedWorkbenchTabClose<'_> {
    #[must_use]
    pub fn result(&self) -> &WorkbenchCommandResult {
        &self.response
    }
    #[must_use]
    pub fn target(&self) -> &WorkbenchTabCloseTarget {
        &self.record.target
    }
    #[must_use]
    pub fn commit(self) -> WorkbenchCommandResult {
        self.authority.ledger_bytes = self.next_ledger_bytes;
        self.authority
            .ledger
            .insert(self.request_id, RetainedRequest::TabClose(self.record));
        self.response
    }
}

impl WorkbenchAuthority {
    pub fn begin_terminal_tab_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchTabCloseBegin<'_> {
        let authorized = matches!(&command.action, WorkbenchAction::TabClose { .. })
            && grant.is_some_and(|grant| grant.site_id == *self.site_id() && grant.close_terminal);
        if !authorized {
            return WorkbenchTabCloseBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(retained) = self.ledger.get(&command.request_id) {
            return WorkbenchTabCloseBegin::Ready(if retained.command() == &command {
                retained.replay_result().clone()
            } else {
                result(
                    &command,
                    self.rejected(WorkbenchRejectionReason::RequestIdentityConflict),
                )
            });
        }
        if self.retired {
            return WorkbenchTabCloseBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        }
        if self.ledger.len() >= MAXIMUM_LEDGER_ENTRIES {
            return WorkbenchTabCloseBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let WorkbenchAction::TabClose {
            terminal_context_id,
            tab_id,
        } = command.action.clone()
        else {
            return WorkbenchTabCloseBegin::Ready(self.retire(&command));
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
            return WorkbenchTabCloseBegin::Ready(self.retain_rejection(command, reason));
        }
        let membership = match self.terminal_tree_membership(&terminal_context_id) {
            Ok(membership) => membership,
            Err(_invariant) => return WorkbenchTabCloseBegin::Ready(self.retire(&command)),
        };
        WorkbenchTabCloseBegin::Admitted(Box::new(WorkbenchTabCloseAdmission {
            authority: self,
            command,
            membership,
            tab_id,
        }))
    }
}
