//! Admits Tab, Panel, and Workspace close scope against one canonical revision, reserving
//! ledger capacity and captured runtime members before commitment. It does not inspect
//! processes or send termination and disposal requests.

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
    CloseContinuation, CloseRecord, CloseScope, WorkbenchTabCloseMemberRuntime,
    WorkbenchTabCloseRuntimeTarget, WorkbenchTabCloseTarget,
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
    panel_id: Option<orchbee_workbench_protocol::WorkbenchPanelId>,
}

#[allow(clippy::large_enum_variant)]
pub enum WorkbenchWorkspaceCloseBegin<'a> {
    Ready(WorkbenchCommandResult),
    Admitted(Box<WorkbenchWorkspaceCloseAdmission<'a>>),
}

pub struct WorkbenchWorkspaceCloseAdmission<'a> {
    authority: &'a mut WorkbenchAuthority,
    command: WorkbenchCommand,
    membership: WorkbenchTerminalMembership,
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
    scope: CloseScope,
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

    #[must_use]
    pub fn resolve_after_composition(self) -> WorkbenchTabCloseResolution<'a> {
        let tab = self
            .authority
            .snapshot()
            .terminal_contexts
            .iter()
            .find(|context| &context.terminal_context_id == self.membership.terminal_context_id())
            .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == self.tab_id));
        let Some(tab) = tab else {
            return WorkbenchTabCloseResolution::Ready(self.authority.retain_rejection(
                self.command,
                if self.panel_id.is_some() {
                    WorkbenchRejectionReason::PanelUnavailable
                } else {
                    WorkbenchRejectionReason::TabUnavailable
                },
            ));
        };
        if self
            .panel_id
            .as_ref()
            .is_some_and(|id| super::publication::leaf_presentation(&tab.root, id).is_none())
        {
            return WorkbenchTabCloseResolution::Ready(
                self.authority
                    .retain_rejection(self.command, WorkbenchRejectionReason::PanelUnavailable),
            );
        }
        let single_panel = matches!(
            tab.root,
            orchbee_workbench_protocol::PanelNode::Panel { .. }
        );
        let tab_members: Vec<_> = self
            .membership
            .members_for_tab(&self.tab_id)
            .cloned()
            .collect();
        let (members, scope) = match &self.panel_id {
            None => (tab_members.clone(), CloseScope::Tab),
            Some(panel_id) => {
                let members: Vec<_> = tab_members
                    .iter()
                    .filter(|member| member.panel_id() == panel_id)
                    .cloned()
                    .collect();
                // Children are independent targets, not additional layout Panels.
                let scope = if single_panel {
                    CloseScope::Tab
                } else {
                    CloseScope::Panel
                };
                (members, scope)
            }
        };
        let covering = self
            .authority
            .ledger
            .values()
            .find_map(|entry| match entry {
                RetainedRequest::TabClose(record)
                    if record.overlaps_tab_scope(
                        self.membership.terminal_context_id(),
                        &self.tab_id,
                        self.panel_id.as_ref(),
                    ) && (!record.is_closed()
                        || members.iter().any(|member| record.covers_member(member))) =>
                {
                    Some(record)
                }
                _ => None,
            });
        if let Some(record) = covering {
            if record.is_closed() {
                // A completed Close cannot still cover a member that remains projected.
                return WorkbenchTabCloseResolution::Ready(self.authority.retire(&self.command));
            }
            if same_leaf_close(&record.command.action, &self.command.action)
                && record.panel_id() == self.panel_id.as_ref()
                && record.matches_members(&members)
            {
                // Joining retained facts is effect-free even when the runtime is now unavailable.
                let plan = CommitPlan {
                    projection: None,
                    last_identity: None,
                    catalog_next: None,
                    outcome: record.result.outcome.clone(),
                };
                return WorkbenchTabCloseResolution::Ready(
                    match self.authority.prepare_command_commit(self.command, plan) {
                        Ok(prepared) => prepared.commit(),
                        Err(response) => *response,
                    },
                );
            }
            // A member is already inside an open Close whose action or membership differs.
            return WorkbenchTabCloseResolution::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        WorkbenchTabCloseResolution::Primary(Box::new(WorkbenchTabClosePrimaryAdmission {
            authority: self.authority,
            command: self.command,
            membership: self.membership,
            members,
            scope,
        }))
    }
}

impl<'a> WorkbenchWorkspaceCloseAdmission<'a> {
    /// Explicit ordinary Close may resume a parked conditional operation with
    /// confirmation authority, preserving its canonical identity and receipts.
    #[must_use]
    pub fn resolve_after_confirmation(self, now: Instant) -> WorkbenchTabCloseResolution<'a> {
        self.resolve_workspace_scope(Some(now))
    }
    #[must_use]
    pub fn terminal_membership(&self) -> &WorkbenchTerminalMembership {
        &self.membership
    }

    #[must_use]
    pub fn retire_composition_failure(self) -> WorkbenchCommandResult {
        self.authority.retire(&self.command)
    }

    /// The captured scope is the whole Context membership — possibly empty.
    /// Judgment order: identical scope joins the retained operation; any other
    /// retained Close covering a captured member refuses busy; a pending
    /// create/split owing this Context a member refuses busy.
    #[must_use]
    pub fn resolve_after_composition(self) -> WorkbenchTabCloseResolution<'a> {
        self.resolve_workspace_scope(None)
    }

    fn resolve_workspace_scope(
        self,
        confirmed_at: Option<Instant>,
    ) -> WorkbenchTabCloseResolution<'a> {
        let workspace_id = match &self.command.action {
            WorkbenchAction::WorkspaceClose { workspace_id }
            | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => workspace_id.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => return WorkbenchTabCloseResolution::Ready(self.authority.retire(&self.command)),
        };
        let members: Vec<_> = self.membership.members().to_vec();
        let context_id = self.membership.terminal_context_id().clone();
        // Scope join precedes member conflict: an open Context Close on this
        // exact triple replays its original acknowledgement with no further
        // close credit, even while members sit inside it.
        let same_scope = self
            .authority
            .ledger
            .values()
            .find_map(|entry| match entry {
                RetainedRequest::TabClose(record)
                    if record.matches_context_scope(&workspace_id) =>
                {
                    Some(record)
                }
                _ => None,
            });
        if let Some(record) = same_scope {
            if record.is_closed() {
                // A completed Context Close cannot coexist with the projected scope.
                return WorkbenchTabCloseResolution::Ready(self.authority.retire(&self.command));
            }
            let close_id = record.command.request_id.clone();
            if !self.authority.worktree_close_join_allowed(&close_id) {
                return WorkbenchTabCloseResolution::Ready(result(
                    &self.command,
                    unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
                ));
            }
            let same_action = std::mem::discriminant(&record.command.action)
                == std::mem::discriminant(&self.command.action);
            let upgrade = confirmed_at
                .filter(|_| {
                    matches!(self.command.action, WorkbenchAction::WorkspaceClose { .. })
                        && matches!(
                            record.command.action,
                            WorkbenchAction::WorkspaceCloseIfIdle { .. }
                        )
                        && record.is_resumable()
                })
                .and_then(|now| {
                    Some((
                        record.generation.checked_next()?,
                        record.prepare_resumes(now).ok()?,
                    ))
                });
            if same_action || upgrade.is_some() {
                // Plain joins are effect-free. Explicit confirmation resumes
                // only this retained operation after the alias commit succeeds.
                let plan = CommitPlan {
                    projection: None,
                    last_identity: None,
                    catalog_next: None,
                    outcome: record.result.outcome.clone(),
                };
                return WorkbenchTabCloseResolution::Ready(
                    match self.authority.prepare_command_commit(self.command, plan) {
                        Ok(prepared) => {
                            let result = prepared.commit();
                            if let Some((generation, resumes)) = upgrade {
                                // The same Authority gate spans lookup and alias
                                // insertion; committing an alias never removes its primary.
                                let Some(RetainedRequest::TabClose(record)) =
                                    self.authority.ledger.get_mut(&close_id)
                                else {
                                    unreachable!("confirmed alias retains its primary Close");
                                };
                                record.commit_resumes(resumes);
                                record.generation = generation;
                                for continuation in &mut record.continuations {
                                    continuation.confirmed();
                                }
                            }
                            // Only a confirmed explicit Close ends a failed removal's
                            // hold; its publication budget restarts from that sample.
                            if let Some(now) = confirmed_at {
                                self.authority.release_failed_worktree_close(&close_id, now);
                            }
                            result
                        }
                        Err(response) => *response,
                    },
                );
            }
            // A Context Close under a different action owns this scope.
            return WorkbenchTabCloseResolution::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        let covering = self
            .authority
            .ledger
            .values()
            .find_map(|entry| match entry {
                RetainedRequest::TabClose(record)
                    if members.iter().any(|member| record.covers_member(member))
                        || (!record.is_finished() && record.matches_context(&context_id)) =>
                {
                    Some(record)
                }
                _ => None,
            });
        if let Some(record) = covering {
            if record.is_closed() {
                // A completed Close cannot still cover a projected member.
                return WorkbenchTabCloseResolution::Ready(self.authority.retire(&self.command));
            }
            // A member sits inside an open incompatible Close.
            return WorkbenchTabCloseResolution::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        if self.authority.pending_terminal_covers_context(&context_id) {
            return WorkbenchTabCloseResolution::Ready(result(
                &self.command,
                unavailable(WorkbenchCommandUnavailableReason::TerminalBusy),
            ));
        }
        WorkbenchTabCloseResolution::Primary(Box::new(WorkbenchTabClosePrimaryAdmission {
            authority: self.authority,
            command: self.command,
            membership: self.membership,
            members,
            scope: CloseScope::Context { workspace_id },
        }))
    }
}

impl<'a> WorkbenchTabClosePrimaryAdmission<'a> {
    /// A conditional preflight could not establish complete live idle evidence.
    /// This retained rejection allocates no Close operation or lifecycle fence.
    #[must_use]
    pub fn confirmation_required(self) -> WorkbenchCommandResult {
        self.authority
            .retain_rejection(self.command, WorkbenchRejectionReason::ConfirmationRequired)
    }
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

    /// The Session supplies an exact retained association and a fresh monotonic acceptance sample.
    #[must_use]
    pub fn finish(
        self,
        runtime: WorkbenchTabCloseRuntimeTarget,
        observed_phase: Option<LifecycleTerminalPhase>,
        now: Instant,
    ) -> WorkbenchTabClosePreparation<'a> {
        self.finish_members(vec![(runtime, observed_phase)], now)
    }

    /// The Session supplies one exact runtime observation for every frozen leaf,
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
        self.finish_resolved_members(
            runtimes
                .into_iter()
                .map(
                    |(target, observed_phase)| WorkbenchTabCloseMemberRuntime::Runtime {
                        target,
                        observed_phase,
                    },
                )
                .collect(),
            now,
        )
    }

    /// Accept one Session-proven runtime or never-created binding per frozen leaf.
    #[must_use]
    pub fn finish_resolved_members(
        self,
        runtimes: Vec<WorkbenchTabCloseMemberRuntime>,
        now: Instant,
    ) -> WorkbenchTabClosePreparation<'a> {
        if runtimes.len() != self.members.len()
            || self
                .members
                .iter()
                .zip(&runtimes)
                .any(|(member, runtime)| !runtime.matches_member(member))
        {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        }
        // Every conditional scope admits only the Server's own evidence for each runtime.
        let conditional_unproven = matches!(
            self.command.action,
            WorkbenchAction::WorkspaceCloseIfIdle { .. }
                | WorkbenchAction::TabCloseIfIdle { .. }
                | WorkbenchAction::PanelCloseIfIdle { .. }
        ) && runtimes.iter().any(|runtime| {
            !matches!(
                runtime,
                WorkbenchTabCloseMemberRuntime::Uncreated { .. }
                    | WorkbenchTabCloseMemberRuntime::QuietRuntime { .. }
            )
        });
        if conditional_unproven {
            return WorkbenchTabClosePreparation::Ready(
                self.authority
                    .retain_rejection(self.command, WorkbenchRejectionReason::ConfirmationRequired),
            );
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
        let outcome = match &self.command.action {
            WorkbenchAction::TabClose { tab_id, .. }
            | WorkbenchAction::TabCloseIfIdle { tab_id, .. } => {
                WorkbenchCommandOutcome::TabCloseAccepted {
                    close_request_id: self.command.request_id.clone(),
                    terminal_context_id: self.membership.terminal_context_id().clone(),
                    tab_id: tab_id.clone(),
                    accepted_revision: self.command.expected_revision,
                }
            }
            WorkbenchAction::PanelClose {
                tab_id, panel_id, ..
            }
            | WorkbenchAction::PanelCloseIfIdle {
                tab_id, panel_id, ..
            } => WorkbenchCommandOutcome::PanelCloseAccepted {
                close_request_id: self.command.request_id.clone(),
                terminal_context_id: self.membership.terminal_context_id().clone(),
                tab_id: tab_id.clone(),
                panel_id: panel_id.clone(),
                accepted_revision: self.command.expected_revision,
            },
            WorkbenchAction::WorkspaceClose { workspace_id }
            | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => {
                WorkbenchCommandOutcome::WorkspaceCloseAccepted {
                    close_request_id: self.command.request_id.clone(),
                    workspace_id: workspace_id.clone(),
                    terminal_context_id: self.membership.terminal_context_id().clone(),
                    accepted_revision: self.command.expected_revision,
                }
            }
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command)),
        };
        let response = result(&self.command, outcome);
        let member_count = self.members.len();
        let request_id = self.command.request_id.clone();
        let mut targets = Vec::with_capacity(member_count);
        let mut continuations = Vec::with_capacity(member_count);
        for (member, evidence) in self.members.into_iter().zip(runtimes) {
            let (runtime, continuation) = match evidence {
                WorkbenchTabCloseMemberRuntime::Runtime {
                    target,
                    observed_phase,
                } => (
                    Some(target),
                    CloseContinuation::prepare(now, observed_phase),
                ),
                // A terminal awaiting its start may start before the fence commits, so the
                // retained operation observes it again rather than disposing of it unobserved.
                WorkbenchTabCloseMemberRuntime::QuietRuntime {
                    target,
                    observed_phase,
                } => (
                    Some(target),
                    CloseContinuation::prepare(
                        now,
                        Some(observed_phase).filter(|phase| {
                            !matches!(phase, LifecycleTerminalPhase::AwaitingStart)
                        }),
                    ),
                ),
                WorkbenchTabCloseMemberRuntime::Uncreated { .. } => {
                    (None, CloseContinuation::prepare_uncreated(now))
                }
            };
            let Some(continuation) = continuation else {
                return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
            };
            targets.push(WorkbenchTabCloseTarget {
                terminal_context_id: self.membership.terminal_context_id().clone(),
                member,
                runtime,
            });
            continuations.push(continuation);
        }
        // Resolve, build, then measure: the bounded record exists before the
        // ledger cap check so the reservation charges actual owned payloads,
        // never a sampled constant. Any failure below drops the temporaries
        // with no retained effect.
        let Some(receipt_slot_bytes) = super::reservation::receipt_slot_bytes(&self.scope) else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&self.command));
        };
        let mut record = Box::new(CloseRecord {
            command: self.command,
            result: response.clone(),
            scope: self.scope,
            terminal_context_id: self.membership.terminal_context_id().clone(),
            targets,
            generation,
            resume_receipt: None,
            receipt_slot_bytes,
            reserved_bytes: 0,
            continuations,
            scheduler_cursor: 0,
            publication_attempted: false,
            parked_publication: None,
            closed_revision: None,
            workspace_retained: false,
        });
        let Some(reserved_bytes) = super::reservation::entry_reservation(
            &record.command,
            &response,
            member_count,
            &record,
            &request_id,
        ) else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&record.command));
        };
        let Some(next_ledger_bytes) = self.authority.ledger_bytes.checked_add(reserved_bytes)
        else {
            return WorkbenchTabClosePreparation::Ready(self.authority.retire(&record.command));
        };
        if next_ledger_bytes > MAXIMUM_LEDGER_BYTES {
            return WorkbenchTabClosePreparation::Ready(result(
                &record.command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        record.reserved_bytes = reserved_bytes;
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
    pub fn target(&self) -> Option<&WorkbenchTabCloseTarget> {
        self.record.targets().next()
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
        self.begin_terminal_tab_close_with_set(grant, command, None)
    }

    /// Internal Tab set capability: the claimed set's next unsubmitted member, unchanged since the
    /// set froze it, is required.
    pub fn begin_tab_set_member_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        operation_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        index: usize,
    ) -> WorkbenchTabCloseBegin<'_> {
        self.begin_terminal_tab_close_with_set(grant, command, Some((operation_id, index)))
    }

    fn begin_terminal_tab_close_with_set(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        set: Option<(&orchbee_workbench_protocol::WorkbenchRequestId, usize)>,
    ) -> WorkbenchTabCloseBegin<'_> {
        let authorized = matches!(
            &command.action,
            WorkbenchAction::TabClose { .. }
                | WorkbenchAction::TabCloseIfIdle { .. }
                | WorkbenchAction::PanelClose { .. }
                | WorkbenchAction::PanelCloseIfIdle { .. }
        ) && grant
            .is_some_and(|grant| grant.session_id == *self.session_id() && grant.close_terminal);
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
        let (terminal_context_id, tab_id, panel_id) = match command.action.clone() {
            WorkbenchAction::TabClose {
                terminal_context_id,
                tab_id,
            }
            | WorkbenchAction::TabCloseIfIdle {
                terminal_context_id,
                tab_id,
            } => (terminal_context_id, tab_id, None),
            WorkbenchAction::PanelClose {
                terminal_context_id,
                tab_id,
                panel_id,
            }
            | WorkbenchAction::PanelCloseIfIdle {
                terminal_context_id,
                tab_id,
                panel_id,
            } => (terminal_context_id, tab_id, Some(panel_id)),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => return WorkbenchTabCloseBegin::Ready(self.retire(&command)),
        };
        let snapshot = self.projection.snapshot();
        let rejection = if command.session_id != snapshot.session_id {
            Some(WorkbenchRejectionReason::SessionMismatch)
        } else if command.expected_revision != snapshot.revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else if !snapshot
            .navigation
            .workspaces
            .iter()
            .any(|workspace| workspace.terminal_context_id == terminal_context_id)
        {
            Some(WorkbenchRejectionReason::TerminalContextUnavailable)
        } else {
            None
        };
        if let Some(reason) = rejection {
            return WorkbenchTabCloseBegin::Ready(self.retain_rejection(command, reason));
        }
        if let Some((operation_id, index)) = set
            && (panel_id.is_some()
                || !self.tab_set_member_matches(operation_id, index, &terminal_context_id, &tab_id))
        {
            return WorkbenchTabCloseBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        let membership = match self.presentation_membership(&terminal_context_id) {
            Ok(membership) => membership,
            Err(_invariant) => return WorkbenchTabCloseBegin::Ready(self.retire(&command)),
        };
        WorkbenchTabCloseBegin::Admitted(Box::new(WorkbenchTabCloseAdmission {
            authority: self,
            command,
            membership,
            tab_id,
            panel_id,
        }))
    }

    /// `workspace.close` admission: the same Close lifecycle machinery scoped
    /// to a Workspace's whole Terminal Context — empty membership included.
    /// Session supplies runtime resolution; this stage resolves the named
    /// `(project, workspace)` scope against the authoritative snapshot.
    pub fn begin_workspace_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
    ) -> WorkbenchWorkspaceCloseBegin<'_> {
        self.begin_workspace_close_with_group(grant, command, None)
    }

    /// Internal group capability: exact retained member, attempt and admission order are required.
    pub fn begin_workspace_group_member_close(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        operation_id: &orchbee_workbench_protocol::WorkbenchRequestId,
        attempt: u32,
        index: usize,
    ) -> WorkbenchWorkspaceCloseBegin<'_> {
        self.begin_workspace_close_with_group(grant, command, Some((operation_id, attempt, index)))
    }

    fn begin_workspace_close_with_group(
        &mut self,
        grant: Option<&WorkbenchGrant>,
        command: WorkbenchCommand,
        group: Option<(&orchbee_workbench_protocol::WorkbenchRequestId, u32, usize)>,
    ) -> WorkbenchWorkspaceCloseBegin<'_> {
        let authorized = matches!(
            &command.action,
            WorkbenchAction::WorkspaceClose { .. } | WorkbenchAction::WorkspaceCloseIfIdle { .. }
        ) && grant
            .is_some_and(|grant| grant.session_id == *self.session_id() && grant.close_terminal);
        if !authorized {
            return WorkbenchWorkspaceCloseBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        if let Some(retained) = self.ledger.get(&command.request_id) {
            return WorkbenchWorkspaceCloseBegin::Ready(if retained.command() == &command {
                retained.replay_result().clone()
            } else {
                result(
                    &command,
                    self.rejected(WorkbenchRejectionReason::RequestIdentityConflict),
                )
            });
        }
        if self.retired {
            return WorkbenchWorkspaceCloseBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::ServiceUnavailable),
            ));
        }
        if self.ledger.len() >= MAXIMUM_LEDGER_ENTRIES {
            return WorkbenchWorkspaceCloseBegin::Ready(result(
                &command,
                unavailable(WorkbenchCommandUnavailableReason::RequestLedgerFull),
            ));
        }
        let workspace_id = match &command.action {
            WorkbenchAction::WorkspaceClose { workspace_id }
            | WorkbenchAction::WorkspaceCloseIfIdle { workspace_id } => workspace_id.clone(),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => return WorkbenchWorkspaceCloseBegin::Ready(self.retire(&command)),
        };
        let snapshot = self.projection.snapshot();
        let rejection = if command.session_id != snapshot.session_id {
            Some(WorkbenchRejectionReason::SessionMismatch)
        } else if command.expected_revision != snapshot.revision {
            Some(WorkbenchRejectionReason::RevisionMismatch)
        } else {
            snapshot
                .navigation
                .workspace(&workspace_id)
                .is_none()
                .then_some(WorkbenchRejectionReason::WorkspaceUnavailable)
        };
        if let Some(reason) = rejection {
            return WorkbenchWorkspaceCloseBegin::Ready(self.retain_rejection(command, reason));
        }
        if let Some((operation_id, attempt, index)) = group
            && !self.workspace_group_member_matches(operation_id, attempt, index, &workspace_id)
        {
            return WorkbenchWorkspaceCloseBegin::Ready(result(
                &command,
                WorkbenchCommandOutcome::Denied {},
            ));
        }
        let workspace = snapshot.navigation.workspace(&workspace_id);
        let Some(workspace) = workspace else {
            return WorkbenchWorkspaceCloseBegin::Ready(self.retire(&command));
        };
        let terminal_context_id = workspace.terminal_context_id.clone();
        let membership = match self.presentation_membership(&terminal_context_id) {
            Ok(membership) => membership,
            Err(_invariant) => return WorkbenchWorkspaceCloseBegin::Ready(self.retire(&command)),
        };
        WorkbenchWorkspaceCloseBegin::Admitted(Box::new(WorkbenchWorkspaceCloseAdmission {
            authority: self,
            command,
            membership,
        }))
    }
}

/// Whether a request joins an open Close of the same Tab or Panel. A conditional request joins
/// an ordinary one, and the reverse, because the Close already runs and asking would end nothing.
fn same_leaf_close(retained: &WorkbenchAction, requested: &WorkbenchAction) -> bool {
    let tab = |action: &WorkbenchAction| {
        matches!(
            action,
            WorkbenchAction::TabClose { .. } | WorkbenchAction::TabCloseIfIdle { .. }
        )
    };
    let panel = |action: &WorkbenchAction| {
        matches!(
            action,
            WorkbenchAction::PanelClose { .. } | WorkbenchAction::PanelCloseIfIdle { .. }
        )
    };
    (tab(retained) && tab(requested)) || (panel(retained) && panel(requested))
}
