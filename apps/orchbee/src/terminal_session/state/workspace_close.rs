//! Confirm and retain one Workspace Close or explicit Project close-all. Membership owns removal.
use super::{
    NativeContextTabs, NativeSessionState, OperationDeadlines, PaneKey, PendingRefreshFlight,
    SessionNotice, SessionPhase,
};
use crate::{
    local_client::workbench::WorkbenchWorkspaceCloseAccepted,
    terminal_session::workbench_projection::NativeContextLocation,
    tui_presentation::{GeometrySettlement, PresentationPhase},
};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchAction, WorkbenchCommand, WorkbenchProjectId, WorkbenchRequestId,
    WorkbenchTabClosePublicationReason, WorkbenchTabCloseStatus, WorkbenchTerminalContextId,
    WorkbenchWorkspaceCloseReadOutcome, WorkbenchWorkspaceGroupCloseRequest,
    WorkbenchWorkspaceGroupCloseResult, WorkbenchWorkspaceId, WorkspaceGroupCloseAction,
    WorkspaceGroupCloseOutcome, WorkspaceGroupCloseStatus,
};
use std::time::{Duration, Instant};

/// Identifies exactly one fully drawn destructive confirmation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceCloseToken(u64);
#[cfg(test)]
impl WorkspaceCloseToken {
    pub(crate) const fn for_test(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum WorkspaceCloseTarget {
    Workspace {
        workspace_id: WorkbenchWorkspaceId,
        terminal_context_id: WorkbenchTerminalContextId,
    },
    Project {
        project_id: WorkbenchProjectId,
        member_count: usize,
    },
}
impl WorkspaceCloseTarget {
    fn matches_snapshot(&self, baseline: &NativeContextTabs) -> bool {
        let snapshot = baseline.canonical_snapshot();
        match self {
            Self::Workspace {
                workspace_id,
                terminal_context_id,
            } => {
                snapshot
                    .navigation
                    .workspace(workspace_id)
                    .is_some_and(|workspace| &workspace.terminal_context_id == terminal_context_id)
                    && snapshot
                        .terminal_contexts
                        .iter()
                        .any(|context| &context.terminal_context_id == terminal_context_id)
            }
            Self::Project {
                project_id,
                member_count,
            } => {
                snapshot.navigation.project(project_id).is_some()
                    && snapshot
                        .navigation
                        .workspaces_in_project(Some(project_id))
                        .count()
                        == *member_count
            }
        }
    }
    fn project_member_count(&self) -> Option<usize> {
        match self {
            Self::Project { member_count, .. } => Some(*member_count),
            Self::Workspace { .. } => None,
        }
    }
    fn workspace_id(&self) -> Option<&WorkbenchWorkspaceId> {
        match self {
            Self::Workspace { workspace_id, .. } => Some(workspace_id),
            Self::Project { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct WorkspaceCloseOperation {
    pub(in crate::terminal_session) target: WorkspaceCloseTarget,
    pub(in crate::terminal_session) request_id: WorkbenchRequestId,
    pub(in crate::terminal_session) generation: u64,
    pub(in crate::terminal_session) deadlines: OperationDeadlines,
    pub(in crate::terminal_session) baseline: NativeContextTabs,
    desired: Option<PaneKey>,
    label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) enum WorkspaceCloseResult {
    NotSubmitted,
    Refused,
    Unknown,
    WorkspaceAccepted(WorkbenchWorkspaceCloseAccepted),
    WorkspaceObserved {
        accepted: WorkbenchWorkspaceCloseAccepted,
        outcome: Box<WorkbenchWorkspaceCloseReadOutcome>,
    },
    ProjectObserved(Box<WorkbenchWorkspaceGroupCloseResult>),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::terminal_session) struct WorkspaceCloseEvidence {
    pub(in crate::terminal_session) operation: WorkspaceCloseOperation,
    pub(in crate::terminal_session) result: WorkspaceCloseResult,
}
impl WorkspaceCloseResult {
    pub(in crate::terminal_session) fn is_closed(&self) -> bool {
        matches!(self,
        Self::WorkspaceObserved { outcome, .. }
            if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                status: WorkbenchTabCloseStatus::Closed { .. }, ..
            }))
            || matches!(self, Self::ProjectObserved(reply) if matches!(reply.outcome,
                WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Closed {}, .. }))
    }
    pub(in crate::terminal_session) fn is_pending(&self) -> bool {
        matches!(self, Self::WorkspaceAccepted(_))
            || matches!(self,
            Self::WorkspaceObserved { outcome, .. }
                if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                    status: WorkbenchTabCloseStatus::Closing { .. }
                        | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                            reason: WorkbenchTabClosePublicationReason::Preparing
                                | WorkbenchTabClosePublicationReason::TargetCleanupPending,
                            resumable: false,
                        }, ..
                }))
            || matches!(self, Self::ProjectObserved(reply) if matches!(reply.outcome,
                WorkspaceGroupCloseOutcome::Available { status: WorkspaceGroupCloseStatus::Pending {}, .. }))
    }
    fn has_admission(&self) -> bool {
        matches!(
            self,
            Self::WorkspaceAccepted(_) | Self::WorkspaceObserved { .. } | Self::ProjectObserved(_)
        )
    }
    fn unresolved(&self) -> bool {
        !matches!(self, Self::NotSubmitted | Self::Refused) && !self.is_closed()
    }
    fn status(&self) -> WorkspaceCloseStatus {
        if self.is_closed() {
            return WorkspaceCloseStatus::Closed;
        }
        if self.is_pending() {
            return WorkspaceCloseStatus::Closing;
        }
        match self {
            Self::NotSubmitted | Self::Refused => WorkspaceCloseStatus::Refused,
            Self::Unknown => WorkspaceCloseStatus::Unknown,
            Self::WorkspaceObserved { outcome, .. }
                if matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                    status: WorkbenchTabCloseStatus::OutcomeUnknown { .. }, ..
                }) => WorkspaceCloseStatus::Unknown,
            Self::ProjectObserved(reply)
                if matches!(reply.outcome, WorkspaceGroupCloseOutcome::Available {
                    status: WorkspaceGroupCloseStatus::Failed {
                        reason: orchbee_workbench_protocol::WorkspaceGroupCloseFailure::OutcomeUnknown, ..
                    }, ..
                }) => WorkspaceCloseStatus::Unknown,
            _ => WorkspaceCloseStatus::Failed,
        }
    }
}
impl WorkspaceCloseOperation {
    pub(in crate::terminal_session) fn workspace_command(&self) -> Option<WorkbenchCommand> {
        let WorkspaceCloseTarget::Workspace { workspace_id, .. } = &self.target else {
            return None;
        };
        Some(WorkbenchCommand {
            protocol_version: ProtocolVersion,
            request_id: self.request_id.clone(),
            site_id: self.baseline.site_id().clone(),
            expected_revision: self.baseline.revision(),
            action: WorkbenchAction::WorkspaceClose {
                workspace_id: workspace_id.clone(),
            },
        })
    }
    pub(in crate::terminal_session) fn project_request(
        &self,
    ) -> Option<WorkbenchWorkspaceGroupCloseRequest> {
        let WorkspaceCloseTarget::Project { project_id, .. } = &self.target else {
            return None;
        };
        Some(WorkbenchWorkspaceGroupCloseRequest {
            protocol_version: ProtocolVersion,
            request_id: self.request_id.clone(),
            site_id: self.baseline.site_id().clone(),
            action: WorkspaceGroupCloseAction::Start {
                operation_id: self.request_id.clone(),
                project_id: project_id.clone(),
            },
        })
    }
    fn admits_accepted(&self, accepted: &WorkbenchWorkspaceCloseAccepted) -> bool {
        matches!(&self.target, WorkspaceCloseTarget::Workspace { workspace_id, terminal_context_id }
            if accepted.workspace_id == *workspace_id
                && accepted.terminal_context_id == *terminal_context_id
                && accepted.accepted_revision.value() != u64::MAX
                && accepted.accepted_revision <= self.baseline.revision()
                && (accepted.close_request_id != self.request_id
                    || accepted.accepted_revision == self.baseline.revision()))
    }
    pub(in crate::terminal_session) fn admits(&self, result: &WorkspaceCloseResult) -> bool {
        if !self.target.matches_snapshot(&self.baseline) {
            return false;
        }
        match result {
            WorkspaceCloseResult::NotSubmitted
            | WorkspaceCloseResult::Refused
            | WorkspaceCloseResult::Unknown => true,
            WorkspaceCloseResult::WorkspaceAccepted(accepted) => self.admits_accepted(accepted),
            WorkspaceCloseResult::WorkspaceObserved { accepted, outcome } => {
                self.admits_accepted(accepted)
                    && matches!(outcome.as_ref(), WorkbenchWorkspaceCloseReadOutcome::Available {
                        close_request_id, workspace_id, terminal_context_id, status, ..
                    } if close_request_id == &accepted.close_request_id
                        && workspace_id == &accepted.workspace_id
                        && terminal_context_id == &accepted.terminal_context_id
                        && (!matches!(status, WorkbenchTabCloseStatus::Closed { revision }
                            if revision <= &accepted.accepted_revision)))
            }
            WorkspaceCloseResult::ProjectObserved(reply) => {
                matches!(&self.target, WorkspaceCloseTarget::Project { project_id, .. }
                    if reply.operation_id == self.request_id
                        && reply.validate().is_ok()
                        && matches!(&reply.outcome, WorkspaceGroupCloseOutcome::Available {
                            project_id: actual, ..
                        } if actual == project_id))
            }
        }
    }
}
struct Confirmation {
    token: WorkspaceCloseToken,
    target: WorkspaceCloseTarget,
    baseline: NativeContextTabs,
    desired: Option<PaneKey>,
    label: String,
    presentation: PresentationPhase,
    shown: bool,
}
#[derive(Default)]
pub(super) struct WorkspaceCloseState {
    next_token: u64,
    confirmation: Option<Confirmation>,
    pub(super) pending: Option<WorkspaceCloseOperation>,
    evidence: Option<WorkspaceCloseEvidence>,
    show_pending: bool,
}
/// Read-only scope and draw identity of one pending confirmation.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseConfirmationView<'a> {
    inner: &'a Confirmation,
}
impl<'a> WorkspaceCloseConfirmationView<'a> {
    pub(crate) const fn token(self) -> WorkspaceCloseToken {
        self.inner.token
    }
    pub(crate) fn label(self) -> &'a str {
        &self.inner.label
    }
    /// A Project confirmation names its captured member count, including zero.
    pub(crate) fn project_member_count(self) -> Option<usize> {
        self.inner.target.project_member_count()
    }
    pub(crate) const fn interactive(self) -> bool {
        self.inner.shown && self.inner.presentation.interactive()
    }
    pub(crate) const fn presentation(self) -> PresentationPhase {
        self.inner.presentation
    }
}
/// Lifecycle truth; an absent observation never implies successful removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceCloseStatus {
    Closing,
    Closed,
    Failed,
    Unknown,
    Refused,
}
/// Scope and recovery identity retained independently of the current selection.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceCloseStatusView<'a> {
    operation: &'a WorkspaceCloseOperation,
    operation_id: &'a WorkbenchRequestId,
    status: WorkspaceCloseStatus,
}
impl<'a> WorkspaceCloseStatusView<'a> {
    pub(crate) fn label(self) -> &'a str {
        &self.operation.label
    }
    pub(crate) const fn status(self) -> WorkspaceCloseStatus {
        self.status
    }
    pub(crate) fn operation_id(self) -> Option<&'a WorkbenchRequestId> {
        (self.status != WorkspaceCloseStatus::Refused).then_some(self.operation_id)
    }
    /// Ordinary recovery is scoped by stable Workspace identity, independent of its Project.
    pub(crate) fn workspace_id(self) -> Option<&'a WorkbenchWorkspaceId> {
        self.operation.target.workspace_id()
    }
}
impl NativeSessionState {
    pub(super) fn workspace_close_invariant_holds(&self) -> bool {
        self.workspace_close
            .pending
            .as_ref()
            .is_none_or(|operation| {
                operation.baseline == self.cache
                    && operation.target.matches_snapshot(&operation.baseline)
                    && (self.pending_refresh_generation() == Some(operation.generation)
                        || matches!(self.phase, SessionPhase::Retired(_)))
            })
            && self
                .workspace_close
                .evidence
                .as_ref()
                .is_none_or(|evidence| evidence.operation.admits(&evidence.result))
    }
    pub(in crate::terminal_session) fn workspace_close_blocks_structure(&self) -> bool {
        self.workspace_close_blocks_membership()
            || self
                .workspace_close
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence.result.unresolved())
    }
    /// Finished evidence still fences effects, but canonical read-only membership repair may run.
    pub(in crate::terminal_session) fn workspace_close_blocks_membership(&self) -> bool {
        self.workspace_close.confirmation.is_some() || self.workspace_close.pending.is_some()
    }
    fn workspace_close_quiet(&self) -> bool {
        self.pending_refresh.is_none()
            && self.pending_mutation.is_none()
            && self.pending_membership.is_none()
            && !self.setup_blocks_structure()
            && !self.rename_draft_blocks_structure()
            && self.move_recovery.is_none()
            && self.rename_recovery.is_none()
            && self.rename_uncertain.is_none()
            && self.panel_recovery.is_none()
            && self.close.permits_retired_membership()
            && self.panel_close.confirmation().is_none()
            && matches!(
                self.phase,
                SessionPhase::Empty
                    | SessionPhase::Ready(_)
                    | SessionPhase::SelectedWithoutAttachment(_)
                    | SessionPhase::RetryableAttachUnavailable(_)
            )
    }
    /// Shared availability for either Close scope; opening additionally resolves its exact target.
    pub(in crate::terminal_session) fn close_scope_available(&self) -> bool {
        self.workspace_close_quiet()
            && !self.workspace_close_blocks_structure()
            && self.invariant_holds()
    }
    pub(in crate::terminal_session) fn open_workspace_close(
        &mut self,
    ) -> Option<WorkspaceCloseToken> {
        let NativeContextLocation::Workspace { workspace_id, .. } = self.cache.location() else {
            self.set_notice(SessionNotice::WorkspaceCloseUnavailable);
            return None;
        };
        self.open_workspace_close_at(&workspace_id.clone())
    }
    /// Freezes an addressed Workspace while retaining the current Context and attachment.
    pub(in crate::terminal_session) fn open_workspace_close_at(
        &mut self,
        workspace_id: &WorkbenchWorkspaceId,
    ) -> Option<WorkspaceCloseToken> {
        let Some(workspace) = self
            .cache
            .canonical_snapshot()
            .navigation
            .workspace(workspace_id)
        else {
            self.set_notice(SessionNotice::WorkspaceCloseUnavailable);
            return None;
        };
        self.open_workspace_scope_close_confirmation(
            WorkspaceCloseTarget::Workspace {
                workspace_id: workspace_id.clone(),
                terminal_context_id: workspace.terminal_context_id.clone(),
            },
            workspace.label.as_str().to_owned(),
        )
    }
    /// Opens explicit Project close-all without selecting or borrowing an anchor Workspace.
    pub(in crate::terminal_session) fn open_project_close_all(
        &mut self,
        project_id: &WorkbenchProjectId,
    ) -> Option<WorkspaceCloseToken> {
        let navigation = &self.cache.canonical_snapshot().navigation;
        let Some(project) = navigation
            .projects
            .iter()
            .find(|project| &project.project_id == project_id)
        else {
            self.set_notice(SessionNotice::WorkspaceCloseUnavailable);
            return None;
        };
        let target = WorkspaceCloseTarget::Project {
            project_id: project_id.clone(),
            member_count: navigation.workspaces_in_project(Some(project_id)).count(),
        };
        self.open_workspace_scope_close_confirmation(target, project.label.as_str().to_owned())
    }
    fn open_workspace_scope_close_confirmation(
        &mut self,
        target: WorkspaceCloseTarget,
        label: String,
    ) -> Option<WorkspaceCloseToken> {
        if !self.close_scope_available() {
            self.set_notice(SessionNotice::LocalBusy);
            return None;
        }
        let next = self.workspace_close.next_token.checked_add(1)?;
        self.workspace_close.next_token = next;
        let token = WorkspaceCloseToken(next);
        self.workspace_close.confirmation = Some(Confirmation {
            token,
            target,
            baseline: self.cache.clone(),
            desired: self.desired.clone(),
            label,
            presentation: PresentationPhase::Pending,
            shown: false,
        });
        self.notice = None;
        Some(token)
    }
    pub(in crate::terminal_session) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'_>> {
        self.workspace_close
            .confirmation
            .as_ref()
            .map(|inner| WorkspaceCloseConfirmationView { inner })
    }
    pub(in crate::terminal_session) fn cancel_workspace_close(&mut self) {
        self.workspace_close.confirmation = None;
    }
    pub(in crate::terminal_session) fn acknowledge_workspace_close_draw(
        &mut self,
        token: Option<WorkspaceCloseToken>,
    ) {
        if let Some(prompt) = &mut self.workspace_close.confirmation {
            prompt.shown = token == Some(prompt.token);
            if prompt.shown {
                prompt.presentation.presented();
            }
        }
    }
    /// Refuses a new prompt whose first draw is geometry-blocked, or suspends
    /// an admitted prompt without discarding its frozen scope.
    pub(in crate::terminal_session) fn settle_workspace_close_not_presented(
        &mut self,
    ) -> Option<GeometrySettlement> {
        let prompt = self.workspace_close.confirmation.as_mut()?;
        prompt.shown = false;
        let settlement = prompt.presentation.geometry_blocked();
        if settlement == GeometrySettlement::Refused {
            self.cancel_workspace_close();
        }
        Some(settlement)
    }
    pub(in crate::terminal_session) fn settle_stale_workspace_close(&mut self) -> bool {
        if self
            .workspace_close
            .confirmation
            .as_ref()
            .is_some_and(|prompt| {
                prompt.baseline != self.cache
                    || !prompt.target.matches_snapshot(&prompt.baseline)
                    || prompt.desired != self.desired
                    || !self.workspace_close_quiet()
            })
        {
            self.cancel_workspace_close();
            self.set_notice(SessionNotice::ConfirmationCancelled);
            return true;
        }
        false
    }
    pub(in crate::terminal_session) fn plan_workspace_close(
        &self,
        token: WorkspaceCloseToken,
        request_id: WorkbenchRequestId,
        generation: u64,
        deadlines: OperationDeadlines,
    ) -> Option<WorkspaceCloseOperation> {
        let prompt = self.workspace_close.confirmation.as_ref()?;
        if !prompt.shown
            || prompt.token != token
            || prompt.baseline != self.cache
            || !prompt.target.matches_snapshot(&prompt.baseline)
            || prompt.desired != self.desired
            || !self.workspace_close_quiet()
            || generation == 0
        {
            return None;
        }
        Some(WorkspaceCloseOperation {
            target: prompt.target.clone(),
            request_id,
            generation,
            deadlines,
            baseline: prompt.baseline.clone(),
            desired: prompt.desired.clone(),
            label: prompt.label.clone(),
        })
    }
    pub(in crate::terminal_session) fn begin_workspace_close(
        &mut self,
        operation: WorkspaceCloseOperation,
    ) -> bool {
        let Some(token) = self
            .workspace_close
            .confirmation
            .as_ref()
            .map(|prompt| prompt.token)
        else {
            return false;
        };
        if self
            .plan_workspace_close(
                token,
                operation.request_id.clone(),
                operation.generation,
                operation.deadlines,
            )
            .as_ref()
            != Some(&operation)
        {
            return false;
        }
        self.cancel_workspace_close();
        self.pending_refresh = Some(PendingRefreshFlight::recovery(operation.generation));
        self.workspace_close.pending = Some(operation);
        self.workspace_close.evidence = None;
        self.workspace_close.show_pending = false;
        self.adopt_publication_debt();
        true
    }
    pub(in crate::terminal_session) fn tick_workspace_close_pending(
        &mut self,
        sampled: Instant,
    ) -> bool {
        if !self.workspace_close.show_pending
            && self
                .workspace_close
                .pending
                .as_ref()
                .is_some_and(|operation| {
                    sampled
                        >= operation.deadlines.total() - super::OPERATION_TOTAL
                            + Duration::from_millis(200)
                })
        {
            self.workspace_close.show_pending = true;
            return true;
        }
        false
    }
    pub(in crate::terminal_session) fn retain_workspace_close_evidence(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !evidence.operation.admits(&evidence.result) {
            return false;
        }
        // Late completion may neither replace a later operation nor move its selection.
        if evidence.result.is_closed()
            && self
                .workspace_close
                .pending
                .as_ref()
                .is_some_and(|pending| pending.generation > evidence.operation.generation)
        {
            return true;
        }
        if let Some(prior) = &self.workspace_close.evidence {
            if prior.operation != evidence.operation {
                return evidence.result.is_closed()
                    && prior.operation.generation > evidence.operation.generation;
            }
            if prior.result.has_admission() && !evidence.result.has_admission() {
                return true;
            }
            if !retains_close_result(&prior.result, &evidence.result) {
                return false;
            }
        } else if self.workspace_close.pending.as_ref() != Some(&evidence.operation) {
            return false;
        }
        self.workspace_close.evidence = Some(evidence.clone());
        true
    }
    pub(in crate::terminal_session) fn finish_workspace_close(
        &mut self,
        evidence: &WorkspaceCloseEvidence,
    ) -> bool {
        if !self.retain_workspace_close_evidence(evidence) {
            return false;
        }
        if self.workspace_close.pending.as_ref() == Some(&evidence.operation) {
            self.workspace_close.pending = None;
        }
        true
    }
    pub(in crate::terminal_session) fn workspace_close_deadlines(
        &self,
    ) -> Option<OperationDeadlines> {
        self.workspace_close
            .evidence
            .as_ref()
            .filter(|evidence| {
                self.pending_refresh_generation() == Some(evidence.operation.generation)
            })
            .map(|evidence| evidence.operation.deadlines)
    }
    pub(in crate::terminal_session) fn workspace_close_status(
        &self,
    ) -> Option<WorkspaceCloseStatusView<'_>> {
        if let Some(evidence) = &self.workspace_close.evidence {
            if evidence.result.is_closed()
                && self.cache.location() != evidence.operation.baseline.location()
                && !matches!(self.cache.location(), NativeContextLocation::Standalone(_))
            {
                return None;
            }
            let operation_id = match &evidence.result {
                WorkspaceCloseResult::WorkspaceAccepted(accepted)
                | WorkspaceCloseResult::WorkspaceObserved { accepted, .. } => {
                    &accepted.close_request_id
                }
                WorkspaceCloseResult::ProjectObserved(reply) => &reply.operation_id,
                _ => &evidence.operation.request_id,
            };
            return Some(WorkspaceCloseStatusView {
                operation: &evidence.operation,
                operation_id,
                status: evidence.result.status(),
            });
        }
        let pending = self
            .workspace_close
            .pending
            .as_ref()
            .filter(|_| self.workspace_close.show_pending)?;
        Some(WorkspaceCloseStatusView {
            operation: pending,
            operation_id: &pending.request_id,
            status: WorkspaceCloseStatus::Closing,
        })
    }
}

/// Every consumer retains the same monotone evidence, including admission before the first read.
pub(in crate::terminal_session) fn retains_close_result(
    before: &WorkspaceCloseResult,
    after: &WorkspaceCloseResult,
) -> bool {
    match (before, after) {
        (
            WorkspaceCloseResult::WorkspaceAccepted(a),
            WorkspaceCloseResult::WorkspaceAccepted(b),
        ) => a == b,
        (
            WorkspaceCloseResult::WorkspaceAccepted(a),
            WorkspaceCloseResult::WorkspaceObserved { accepted, .. },
        ) => a == accepted,
        (
            WorkspaceCloseResult::WorkspaceObserved {
                accepted: a,
                outcome: before,
            },
            WorkspaceCloseResult::WorkspaceObserved {
                accepted: b,
                outcome: after,
            },
        ) => a == b && retains_workspace_observation(before, after),
        (
            WorkspaceCloseResult::ProjectObserved(before),
            WorkspaceCloseResult::ProjectObserved(after),
        ) => retains_group_observation(before, after),
        (
            WorkspaceCloseResult::NotSubmitted
            | WorkspaceCloseResult::Refused
            | WorkspaceCloseResult::Unknown,
            _,
        ) => true,
        _ => false,
    }
}
fn retains_workspace_observation(
    before: &WorkbenchWorkspaceCloseReadOutcome,
    after: &WorkbenchWorkspaceCloseReadOutcome,
) -> bool {
    let (
        WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: br,
            workspace_id: bw,
            terminal_context_id: bc,
            attempt_generation: ba,
            status: bs,
            members: bm,
            ..
        },
        WorkbenchWorkspaceCloseReadOutcome::Available {
            close_request_id: ar,
            workspace_id: aw,
            terminal_context_id: ac,
            attempt_generation: aa,
            status: ast,
            members: am,
            ..
        },
    ) = (before, after)
    else {
        return false;
    };
    br == ar && bw == aw && bc == ac && aa >= ba
        && (!matches!(bs, WorkbenchTabCloseStatus::Closed { .. }) || before == after)
        && (!matches!(bs, WorkbenchTabCloseStatus::OutcomeUnknown { .. }) || (ba == aa && bs == ast))
        && retains_disposal(bs, ast)
        && bm.len() == am.len()
        && bm.iter().zip(am).all(|(old, new)| {
            old.tab_id == new.tab_id && old.panel_id == new.panel_id && old.presentation_id == new.presentation_id
                && (!matches!(old.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::OutcomeUnknown { .. })
                    || (ba == aa && old.status == new.status))
                && (!matches!(old.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { .. })
                    || matches!(new.status, orchbee_workbench_protocol::WorkbenchWorkspaceCloseMemberStatus::DisposedAwaitingPublication { .. }))
        })
}
fn retains_disposal(before: &WorkbenchTabCloseStatus, after: &WorkbenchTabCloseStatus) -> bool {
    !matches!(
        before,
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
            | WorkbenchTabCloseStatus::Closed { .. }
    ) || matches!(
        after,
        WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
            | WorkbenchTabCloseStatus::Closed { .. }
    )
}
fn retains_group_observation(
    before: &WorkbenchWorkspaceGroupCloseResult,
    after: &WorkbenchWorkspaceGroupCloseResult,
) -> bool {
    let (
        WorkspaceGroupCloseOutcome::Available {
            project_id: bp,
            attempt: ba,
            status: bs,
            members: bm,
        },
        WorkspaceGroupCloseOutcome::Available {
            project_id: ap,
            attempt: aa,
            status: ast,
            members: am,
        },
    ) = (&before.outcome, &after.outcome)
    else {
        return false;
    };
    before.operation_id == after.operation_id
        && bp == ap
        && aa >= ba
        && bm.len() == am.len()
        && (!matches!(bs, WorkspaceGroupCloseStatus::Closed {}) || before.outcome == after.outcome)
        && (aa > ba || !matches!(bs, WorkspaceGroupCloseStatus::Failed { .. }) || bs == ast)
        && (!matches!(
            bs,
            WorkspaceGroupCloseStatus::Failed {
                resumable: false,
                ..
            }
        ) || aa == ba)
        && bm.iter().zip(am).all(|(old, new)| {
            old.workspace_id == new.workspace_id
                && old.label == new.label
                && old.close.as_ref().is_none_or(|child| {
                    new.close.as_ref().is_some_and(|next| {
                        child.request_id == next.request_id
                            && (!child.is_closed() || child == next)
                            && child.observation.as_ref().is_none_or(|a| {
                                next.observation.as_ref().is_some_and(|b| {
                                    b.attempt_generation >= a.attempt_generation
                                        && (!matches!(
                                            a.status,
                                            WorkbenchTabCloseStatus::OutcomeUnknown { .. }
                                        ) || a == b)
                                        && retains_disposal(&a.status, &b.status)
                                })
                            })
                    })
                })
        })
}
impl<'a> super::NativeSessionView<'a> {
    pub(in crate::terminal_session) fn workspace_close_confirmation(
        &self,
    ) -> Option<WorkspaceCloseConfirmationView<'a>> {
        self.state.workspace_close_confirmation()
    }
    pub(in crate::terminal_session) fn workspace_close_status(
        &self,
    ) -> Option<WorkspaceCloseStatusView<'a>> {
        self.state.workspace_close_status()
    }
}
