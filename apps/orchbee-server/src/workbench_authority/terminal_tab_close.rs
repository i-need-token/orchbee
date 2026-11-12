//! Retains bounded close records, captured members, physical targets, attempt generations, and
//! structural fences until cleanup and publication settle. It does not perform lifecycle I/O
//! or equate a missing member with confirmed disposal.

use std::time::Instant;

use orchbee_capability_node_protocol::{
    HostTerminalIdentity, TerminalFence, TerminalId, TerminalLifecycleRequest, WorkerGeneration,
    WorkerId,
};
use orchbee_workbench_protocol::{
    WorkbenchAction, WorkbenchCommand, WorkbenchCommandResult, WorkbenchPanelId,
    WorkbenchRequestId, WorkbenchRevision, WorkbenchTabCloseAttemptGeneration,
    WorkbenchTabClosePhase, WorkbenchTabClosePublicationReason, WorkbenchTabCloseResume,
    WorkbenchTabCloseResumeResult, WorkbenchTabCloseStatus, WorkbenchTabId,
    WorkbenchTerminalContextId, WorkbenchWorkspaceCloseResume, WorkbenchWorkspaceCloseResumeResult,
    WorkbenchWorkspaceId,
};

use super::{PlanFailure, RetainedRequest, WorkbenchAuthority, WorkbenchTerminalTab};

mod admission;
mod continuation;
mod control;
mod coordination;
mod operation_read;
mod publication;
mod reservation;

pub use admission::{
    PreparedWorkbenchTabClose, WorkbenchTabCloseAdmission, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabClosePrimaryAdmission, WorkbenchTabCloseResolution,
    WorkbenchTabCloseRuntimeUnavailableReason, WorkbenchWorkspaceCloseAdmission,
    WorkbenchWorkspaceCloseBegin,
};
use continuation::CloseContinuation;
pub use continuation::{
    CloseContinuationNext, CloseStepObservation, CloseStepPrequeueFailure, CloseStepToken,
    CloseStepTokenMismatch,
};
use continuation::{CloseContinuationResumeError, PreparedCloseContinuationResume};
pub use control::{
    PreparedWorkbenchTabCloseResume, WorkbenchTabCloseResumeAdmission,
    WorkbenchTabCloseResumeBegin, WorkbenchTabCloseResumePreparation,
    WorkbenchWorkspaceCloseResumeAdmission, WorkbenchWorkspaceCloseResumeBegin,
    WorkbenchWorkspaceCloseResumePreparation,
};
pub use publication::{PreparedWorkbenchTabClosePublication, WorkbenchTabClosePublicationError};

/// Exact retained runtime association supplied by the Session/Terminal owner under its gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseRuntimeTarget {
    worker_id: WorkerId,
    worker_generation: WorkerGeneration,
    terminal_id: TerminalId,
    host_identity: HostTerminalIdentity,
    fence: TerminalFence,
}

impl WorkbenchTabCloseRuntimeTarget {
    #[must_use]
    pub const fn new(
        worker_id: WorkerId,
        worker_generation: WorkerGeneration,
        terminal_id: TerminalId,
        host_identity: HostTerminalIdentity,
        fence: TerminalFence,
    ) -> Self {
        Self {
            worker_id,
            worker_generation,
            terminal_id,
            host_identity,
            fence,
        }
    }

    #[must_use]
    pub const fn worker_id(self) -> WorkerId {
        self.worker_id
    }
    #[must_use]
    pub const fn worker_generation(self) -> WorkerGeneration {
        self.worker_generation
    }
    #[must_use]
    pub const fn terminal_id(self) -> TerminalId {
        self.terminal_id
    }
    #[must_use]
    pub const fn host_identity(self) -> HostTerminalIdentity {
        self.host_identity
    }
    #[must_use]
    pub const fn fence(self) -> TerminalFence {
        self.fence
    }

    fn matches_member(self, member: &WorkbenchTerminalTab) -> bool {
        u128::from_str_radix(member.binding().worker_id.as_str(), 16)
            .is_ok_and(|value| value.to_be_bytes() == self.worker_id.as_bytes())
            && u128::from_str_radix(member.binding().terminal_id.as_str(), 16)
                .is_ok_and(|value| value.to_be_bytes() == self.terminal_id.as_bytes())
    }

    fn matches_request(self, request: &TerminalLifecycleRequest) -> bool {
        request.target == self.host_identity
            && request.authorization.worker_id == self.worker_id
            && request.authorization.worker_generation == self.worker_generation
            && request.authorization.terminal_id == self.terminal_id
            && request.authorization.binding_fence == self.fence
    }
}

/// Session-owned evidence for one exact frozen member. Uncreated means a settled
/// Create failure before Host commit, never merely a missing runtime observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkbenchTabCloseMemberRuntime {
    Runtime {
        target: WorkbenchTabCloseRuntimeTarget,
        observed_phase: Option<orchbee_capability_node_protocol::LifecycleTerminalPhase>,
    },
    /// This exact runtime ended, or the Server's own read of its shell and processes found
    /// nothing beyond an idle shell. A conditional Workspace, Tab or Panel Close then runs the
    /// ordinary lifecycle.
    QuietRuntime {
        target: WorkbenchTabCloseRuntimeTarget,
        observed_phase: orchbee_capability_node_protocol::LifecycleTerminalPhase,
    },
    Uncreated {
        binding: orchbee_workbench_protocol::PresentationBinding,
    },
}

impl WorkbenchTabCloseMemberRuntime {
    pub(super) fn matches_member(&self, member: &WorkbenchTerminalTab) -> bool {
        match self {
            Self::Runtime { target, .. } | Self::QuietRuntime { target, .. } => {
                target.matches_member(member)
            }
            Self::Uncreated { binding } => binding == member.binding(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseTarget {
    terminal_context_id: WorkbenchTerminalContextId,
    member: WorkbenchTerminalTab,
    runtime: Option<WorkbenchTabCloseRuntimeTarget>,
}

impl WorkbenchTabCloseTarget {
    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    #[must_use]
    pub fn member(&self) -> &WorkbenchTerminalTab {
        &self.member
    }
    #[must_use]
    pub const fn runtime(&self) -> Option<WorkbenchTabCloseRuntimeTarget> {
        self.runtime
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseStepTarget {
    target: Option<WorkbenchTabCloseTarget>,
    next: CloseContinuationNext,
}

impl WorkbenchTabCloseStepTarget {
    #[must_use]
    pub fn target(&self) -> Option<&WorkbenchTabCloseTarget> {
        self.target.as_ref()
    }

    #[must_use]
    pub const fn next(&self) -> CloseContinuationNext {
        self.next
    }
}

/// Whether the Close removes one Tab, one leaf Panel, or a whole Workspace
/// Terminal Context. The Context scope carries its Workspace join identity:
/// coverage, join, status, and publication all key on this triple, never on
/// `members.any` — a Workspace with zero leaf members still owns a scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CloseScope {
    Tab,
    Panel,
    Context { workspace_id: WorkbenchWorkspaceId },
}

pub(super) struct CloseRecord {
    pub(super) command: WorkbenchCommand,
    pub(super) result: WorkbenchCommandResult,
    scope: CloseScope,
    /// The scope's Terminal Context — retained separately from `targets` so an
    /// empty Context membership still has an addressable scope.
    terminal_context_id: WorkbenchTerminalContextId,
    targets: Vec<WorkbenchTabCloseTarget>,
    generation: WorkbenchTabCloseAttemptGeneration,
    resume_receipt: Option<AcceptedResumeReceipt>,
    /// One receipt payload slot — the measured owned-buffer budget any
    /// single accepted-resume receipt must fit; the reservation holds two.
    receipt_slot_bytes: usize,
    reserved_bytes: usize,
    continuations: Vec<CloseContinuation>,
    scheduler_cursor: usize,
    /// Zero-member scope publication-only marker: the first scheduled
    /// Publication flips this, transitioning `closing(queued)` into
    /// `disposed-awaiting-publication(preparing)`.
    publication_attempted: bool,
    /// A failed publication on a zero-member scope parks the same
    /// `(reason, resumable)` truth a member continuation would carry.
    parked_publication: Option<(WorkbenchTabClosePublicationReason, bool)>,
    closed_revision: Option<WorkbenchRevision>,
    workspace_retained: bool,
}

pub(super) enum AcceptedResumeReceipt {
    Tab {
        request: WorkbenchTabCloseResume,
        result: WorkbenchTabCloseResumeResult,
    },
    Workspace {
        request: WorkbenchWorkspaceCloseResume,
        result: WorkbenchWorkspaceCloseResumeResult,
    },
}

impl CloseRecord {
    fn tab_id(&self) -> Option<&WorkbenchTabId> {
        match &self.command.action {
            WorkbenchAction::TabClose { tab_id, .. }
            | WorkbenchAction::TabCloseIfIdle { tab_id, .. }
            | WorkbenchAction::PanelClose { tab_id, .. }
            | WorkbenchAction::PanelCloseIfIdle { tab_id, .. } => Some(tab_id),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        }
    }

    fn panel_id(&self) -> Option<&WorkbenchPanelId> {
        match &self.command.action {
            WorkbenchAction::PanelClose { panel_id, .. }
            | WorkbenchAction::PanelCloseIfIdle { panel_id, .. } => Some(panel_id),
            // This adapter owns a specific Workbench action subset; other commands use their own admission and receipt paths.
            _ => None,
        }
    }

    /// Scope addresses remain authoritative even when there are no runtime targets.
    fn overlaps_tab_scope(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
        panel_id: Option<&WorkbenchPanelId>,
    ) -> bool {
        if self.workspace_retained || !self.matches_context(context_id) {
            return false;
        }
        match self.scope {
            CloseScope::Context { .. } => true,
            CloseScope::Tab => self.tab_id() == Some(tab_id),
            CloseScope::Panel => {
                self.tab_id() == Some(tab_id)
                    && panel_id.is_none_or(|panel| self.panel_id() == Some(panel))
            }
        }
    }

    fn is_closed(&self) -> bool {
        self.closed_revision.is_some()
    }

    fn is_finished(&self) -> bool {
        self.is_closed() || self.workspace_retained
    }

    fn target_count(&self) -> usize {
        self.targets.len()
    }

    fn target(&self, index: usize) -> Option<&WorkbenchTabCloseTarget> {
        self.targets.get(index)
    }

    fn continuation(&self, index: usize) -> Option<&CloseContinuation> {
        self.continuations.get(index)
    }

    fn continuation_mut(&mut self, index: usize) -> Option<&mut CloseContinuation> {
        self.continuations.get_mut(index)
    }

    fn target_index_for_binding(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<usize> {
        self.targets
            .iter()
            .position(|target| target.member.binding() == binding)
    }

    fn target_index_for_runtime(&self, runtime: WorkbenchTabCloseRuntimeTarget) -> Option<usize> {
        self.targets
            .iter()
            .position(|target| target.runtime == Some(runtime))
    }

    fn target_index_for_request(&self, request: &TerminalLifecycleRequest) -> Option<usize> {
        self.targets.iter().position(|target| {
            target
                .runtime
                .is_some_and(|runtime| runtime.matches_request(request))
        })
    }

    fn members(&self) -> impl Iterator<Item = &WorkbenchTerminalTab> {
        self.targets.iter().map(|target| &target.member)
    }

    fn targets(&self) -> impl Iterator<Item = &WorkbenchTabCloseTarget> {
        self.targets.iter()
    }

    fn target_for_binding(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<&WorkbenchTabCloseTarget> {
        self.target_index_for_binding(binding)
            .and_then(|index| self.target(index))
    }

    fn matches_context(&self, context_id: &WorkbenchTerminalContextId) -> bool {
        self.terminal_context_id == *context_id
    }

    /// Tab/Panel addresses come from the retained command, including unbound scopes.
    /// Workspace Context scopes are addressed only through their Workspace ID.
    fn matches_context_tab(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        !self.workspace_retained
            && self.matches_context(context_id)
            && match self.scope {
                CloseScope::Tab | CloseScope::Panel => self.tab_id() == Some(tab_id),
                CloseScope::Context { .. } => false,
            }
    }

    /// A close of this whole Tab: a Tab close, or a Panel close admitted as the close of the
    /// Tab's last leaf. A leaf-scoped Panel close shares the Tab address without closing the Tab.
    fn closes_context_tab(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        self.scope == CloseScope::Tab && self.matches_context_tab(context_id, tab_id)
    }

    /// The `(project, workspace)` pair a `workspace.close` names — only
    /// `CloseScope::Context` carries it; the pair identifies the scope's
    /// Terminal Context uniquely, tombstone reads included.
    fn matches_context_scope(&self, workspace_id: &WorkbenchWorkspaceId) -> bool {
        !self.workspace_retained
            && matches!(
                &self.scope,
                CloseScope::Context {
                    workspace_id: workspace,
                } if workspace == workspace_id
            )
    }

    /// An open aggregate Close fences same-Tab structural admission: its publication
    /// re-proves the complete captured ordered leaf membership, which a same-Tab
    /// split/move would invalidate. A leaf-scoped Close re-proves only its own captured
    /// leaf tuple and fences neither operation. An open Context Close re-proves the
    /// whole captured Context membership and therefore fences every Tab in it.
    fn fences_tab_structure(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        if self.is_finished() {
            return false;
        }
        match self.scope {
            CloseScope::Tab => self.matches_context_tab(context_id, tab_id),
            CloseScope::Panel => false,
            CloseScope::Context { .. } => self.matches_context(context_id),
        }
    }

    /// An open Context Close fences any membership-changing admission inside
    /// its Terminal Context — including `tab.create`, which appends a member
    /// no aggregate-Tab fence would see.
    fn fences_context_structure(&self, context_id: &WorkbenchTerminalContextId) -> bool {
        !self.is_finished()
            && matches!(self.scope, CloseScope::Context { .. })
            && self.matches_context(context_id)
    }

    /// An open or settled Close covers this exact leaf member.
    fn covers_member(&self, member: &WorkbenchTerminalTab) -> bool {
        !self.workspace_retained && self.members().any(|existing| existing == member)
    }

    fn matches_members(&self, members: &[WorkbenchTerminalTab]) -> bool {
        self.members().eq(members.iter())
    }

    /// Whether this accepted Close is still on its way to removing its members: closing, or
    /// disposed and waiting only for its own publication. A Close that stopped, whose outcome is
    /// unknown, or that may resume leaves its members in place.
    fn removing(&self) -> bool {
        !self.is_finished()
            && matches!(
                self.status(),
                WorkbenchTabCloseStatus::Closing { .. }
                    | WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        reason: WorkbenchTabClosePublicationReason::TargetCleanupPending
                            | WorkbenchTabClosePublicationReason::Preparing,
                        resumable: false,
                    }
            )
    }

    fn status(&self) -> WorkbenchTabCloseStatus {
        if self.workspace_retained {
            return WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::WorkspaceRetained,
                resumable: false,
            };
        }
        if let Some(revision) = self.closed_revision {
            return WorkbenchTabCloseStatus::Closed { revision };
        }
        if self.targets.is_empty() {
            // A zero-member scope is its own publication-only
            // continuation: queued, then preparing, then parked — the close
            // revision exists only after `commit_closed` lands.
            if let Some((reason, resumable)) = self.parked_publication {
                return WorkbenchTabCloseStatus::DisposedAwaitingPublication { reason, resumable };
            }
            return if self.publication_attempted {
                WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                    reason: WorkbenchTabClosePublicationReason::Preparing,
                    resumable: false,
                }
            } else {
                WorkbenchTabCloseStatus::Closing {
                    phase: WorkbenchTabClosePhase::Queued,
                }
            };
        }
        let statuses: Vec<_> = (0..self.target_count())
            .filter_map(|index| self.continuation(index).map(CloseContinuation::status))
            .collect();
        for status in &statuses {
            if matches!(status, WorkbenchTabCloseStatus::OutcomeUnknown { .. }) {
                return status.clone();
            }
        }
        for status in &statuses {
            if matches!(status, WorkbenchTabCloseStatus::Closing { .. }) {
                return status.clone();
            }
        }
        for status in &statuses {
            if matches!(status, WorkbenchTabCloseStatus::CleanupBlocked { .. }) {
                return status.clone();
            }
        }
        statuses.into_iter().next().unwrap_or(
            WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                reason: WorkbenchTabClosePublicationReason::Preparing,
                resumable: false,
            },
        )
    }

    fn has_outstanding(&self) -> bool {
        (0..self.target_count()).any(|index| {
            self.continuation(index)
                .is_some_and(CloseContinuation::has_outstanding)
        })
    }

    fn has_unknown(&self) -> bool {
        (0..self.target_count()).any(|index| {
            self.continuation(index)
                .is_some_and(CloseContinuation::is_unknown)
        })
    }

    fn is_resumable(&self) -> bool {
        if self.has_unknown() || self.has_outstanding() {
            return false;
        }
        if self.targets.is_empty() {
            return self
                .parked_publication
                .is_some_and(|(_, resumable)| resumable);
        }
        (0..self.target_count()).any(|index| {
            self.continuation(index)
                .is_some_and(CloseContinuation::is_resumable)
        })
    }

    fn prepare_resumes(
        &self,
        now: Instant,
    ) -> Result<Vec<(usize, PreparedCloseContinuationResume)>, CloseContinuationResumeError> {
        if self.has_unknown() {
            return Err(CloseContinuationResumeError::NotResumable);
        }
        let mut prepared = Vec::new();
        for index in 0..self.target_count() {
            let Some(continuation) = self.continuation(index) else {
                continue;
            };
            if continuation.is_resumable() {
                prepared.push((index, continuation.prepare_resume(now)?));
            }
        }
        if prepared.is_empty()
            && !(self.targets.is_empty()
                && self
                    .parked_publication
                    .is_some_and(|(_, resumable)| resumable))
        {
            return Err(CloseContinuationResumeError::NotResumable);
        }
        Ok(prepared)
    }

    fn commit_resumes(&mut self, prepared: Vec<(usize, PreparedCloseContinuationResume)>) {
        for (index, prepared) in prepared {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.commit_resume(prepared);
            }
        }
        if self.targets.is_empty() {
            self.parked_publication = None;
        }
    }

    fn generation_exhausted(&mut self) {
        for index in 0..self.target_count() {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.generation_exhausted();
            }
        }
        if let Some((_, resumable)) = &mut self.parked_publication {
            *resumable = false;
        }
    }

    fn next_step(&mut self, now: Instant) -> WorkbenchTabCloseStepTarget {
        if self.workspace_retained {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Parked,
            };
        }
        if self.is_closed() {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Closed,
            };
        }
        if self.has_outstanding() {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Outstanding,
            };
        }
        if self.targets.is_empty() {
            // Zero-member scope has no lifecycle steps — a parked
            // publication waits for resume, otherwise the next scheduled step
            // is the scope commit itself.
            if self.parked_publication.is_some() {
                return WorkbenchTabCloseStepTarget {
                    target: None,
                    next: CloseContinuationNext::Parked,
                };
            }
            self.publication_attempted = true;
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Publication,
            };
        }
        let count = self.target_count();
        if self.has_unknown() {
            for offset in 0..count {
                let index = (self.scheduler_cursor + offset) % count;
                let Some(next) = self
                    .continuation_mut(index)
                    .map(|continuation| continuation.next(now))
                else {
                    continue;
                };
                if next == CloseContinuationNext::TargetCleanup {
                    self.scheduler_cursor = (index + 1) % count;
                    return WorkbenchTabCloseStepTarget {
                        target: self.target(index).cloned(),
                        next,
                    };
                }
            }
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Parked,
            };
        }
        let mut wait_until = None;
        let mut outstanding = false;
        for offset in 0..count {
            let index = (self.scheduler_cursor + offset) % count;
            let Some(next) = self
                .continuation_mut(index)
                .map(|continuation| continuation.next(now))
            else {
                continue;
            };
            match next {
                CloseContinuationNext::Lifecycle { .. } | CloseContinuationNext::TargetCleanup => {
                    self.scheduler_cursor = (index + 1) % count;
                    return WorkbenchTabCloseStepTarget {
                        target: self.target(index).cloned(),
                        next,
                    };
                }
                CloseContinuationNext::WaitUntil(until) => {
                    wait_until = Some(wait_until.map_or(until, |due: Instant| due.min(until)));
                }
                CloseContinuationNext::Publication
                | CloseContinuationNext::Parked
                | CloseContinuationNext::Closed => {}
                CloseContinuationNext::Outstanding => outstanding = true,
            }
        }
        if let Some(until) = wait_until {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::WaitUntil(until),
            };
        }
        if outstanding {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Outstanding,
            };
        }
        if (0..count).all(|index| {
            self.continuation(index)
                .is_some_and(CloseContinuation::publication_ready)
        }) {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Publication,
            };
        }
        WorkbenchTabCloseStepTarget {
            target: None,
            next: CloseContinuationNext::Parked,
        }
    }

    /// A Worktree hold defers a ready publication without letting a maintenance
    /// turn spend its attempt budget; lifecycle work keeps its own budget.
    fn held_step(&mut self, now: Instant) -> WorkbenchTabCloseStepTarget {
        if !self.targets.is_empty() && !self.is_finished() && self.publication_ready() {
            return WorkbenchTabCloseStepTarget {
                target: None,
                next: CloseContinuationNext::Parked,
            };
        }
        let mut step = self.next_step(now);
        if step.next() == CloseContinuationNext::Publication {
            step.next = CloseContinuationNext::Parked;
        }
        step
    }

    /// The released hold restarts the budget of every publication it deferred.
    fn renew_held_publication(&mut self, now: Instant) {
        if self.is_finished() {
            return;
        }
        for continuation in &mut self.continuations {
            continuation.renew_publication(now);
        }
    }

    fn commit_closed(&mut self, revision: WorkbenchRevision) {
        self.closed_revision = Some(revision);
        for index in 0..self.target_count() {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.commit_closed(revision);
            }
        }
    }

    fn publication_ready(&self) -> bool {
        if self.targets.is_empty() {
            return self.parked_publication.is_none();
        }
        (0..self.target_count()).all(|index| {
            self.continuation(index)
                .is_some_and(CloseContinuation::publication_ready)
        })
    }

    fn publication_failed(
        &mut self,
        reason: orchbee_workbench_protocol::WorkbenchTabClosePublicationReason,
    ) {
        for index in 0..self.target_count() {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.publication_failed(reason);
            }
        }
        if self.targets.is_empty() {
            self.parked_publication = Some((
                reason,
                !matches!(
                    reason,
                    orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::AuthorityRetired
                        | orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::WorkspaceRetained
                ),
            ));
        }
    }

    fn target_requires_runtime(&self, index: usize) -> bool {
        self.continuation(index).is_some_and(|continuation| {
            !matches!(
                continuation.status(),
                WorkbenchTabCloseStatus::DisposedAwaitingPublication { .. }
                    | WorkbenchTabCloseStatus::Closed { .. }
            )
        })
    }

    fn has_target_cleanup_pending(&self) -> bool {
        (0..self.target_count()).any(|index| {
            self.continuation(index).is_some_and(|continuation| {
                matches!(
                    continuation.status(),
                    WorkbenchTabCloseStatus::DisposedAwaitingPublication {
                        reason: orchbee_workbench_protocol::WorkbenchTabClosePublicationReason::TargetCleanupPending,
                        ..
                    }
                )
            })
        })
    }
}

impl WorkbenchAuthority {
    fn close_record(&self, request_id: &WorkbenchRequestId) -> Option<&CloseRecord> {
        match self.ledger.get(request_id)? {
            RetainedRequest::TabClose(record) => Some(record),
            RetainedRequest::Settled(_)
            | RetainedRequest::PendingTabCreate(_)
            | RetainedRequest::PendingPanelCreate(_) => None,
        }
    }

    fn close_record_mut(&mut self, request_id: &WorkbenchRequestId) -> Option<&mut CloseRecord> {
        match self.ledger.get_mut(request_id)? {
            RetainedRequest::TabClose(record) => Some(record),
            RetainedRequest::Settled(_)
            | RetainedRequest::PendingTabCreate(_)
            | RetainedRequest::PendingPanelCreate(_) => None,
        }
    }

    fn outstanding_close_credits(&self) -> Result<u64, PlanFailure> {
        // The shared ledger's 4096-entry bound also bounds all Close owner/tombstone scans.
        let count = self
            .ledger
            .values()
            .filter(
                |entry| matches!(entry, RetainedRequest::TabClose(record) if !record.is_finished()),
            )
            .count();
        u64::try_from(count).map_err(|_error| PlanFailure::Internal)
    }

    /// Whether an open aggregate Close on `tab_id` under `context_id` fences `panel.split`
    /// and `panel.move` admission. The shared ledger's bounded scan is the same one
    /// `outstanding_close_credits` performs.
    pub(super) fn aggregate_close_fences_tab(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        self.ledger.values().any(|entry| {
            matches!(entry, RetainedRequest::TabClose(record)
                if record.fences_tab_structure(context_id, tab_id))
        })
    }

    /// Every terminal an accepted Close is still removing, before its publication lands.
    pub(super) fn close_removing_terminals(
        &self,
    ) -> impl Iterator<Item = &orchbee_workbench_protocol::PresentationBinding> {
        self.ledger
            .values()
            .filter_map(|entry| match entry {
                RetainedRequest::TabClose(record) if record.removing() => Some(&**record),
                RetainedRequest::TabClose(_)
                | RetainedRequest::Settled(_)
                | RetainedRequest::PendingTabCreate(_)
                | RetainedRequest::PendingPanelCreate(_) => None,
            })
            .flat_map(CloseRecord::members)
            .map(WorkbenchTerminalTab::binding)
    }

    /// Relocation cannot move an addressed closing Panel, including an unbound one.
    pub(super) fn open_close_covers_panel(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
        panel_id: &WorkbenchPanelId,
    ) -> bool {
        self.ledger.values().any(|entry| {
            matches!(entry, RetainedRequest::TabClose(record)
                if !record.is_finished() && record.overlaps_tab_scope(context_id, tab_id, Some(panel_id)))
        })
    }

    /// Whether an open Close covers any part of this Tab: its own, one of its Panels' or its
    /// Workspace's.
    pub(super) fn open_close_covers_tab(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &WorkbenchTabId,
    ) -> bool {
        self.ledger.values().any(|entry| {
            matches!(entry, RetainedRequest::TabClose(record)
                if !record.is_finished() && record.overlaps_tab_scope(context_id, tab_id, None))
        })
    }

    /// Whether an open Context Close fences structural admission anywhere in
    /// this Terminal Context — `tab.create` appends membership the captured
    /// re-proof never saw, so it refuses `terminal-busy` until publication.
    pub(super) fn open_close_fences_context(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> bool {
        self.ledger.values().any(|entry| {
            matches!(entry, RetainedRequest::TabClose(record)
                if record.fences_context_structure(context_id))
        })
    }

    /// Tab configuration can reorder several groups at once. Wait for every captured
    /// Close in this context, including a single Panel/Tab, before changing its metadata.
    pub(super) fn open_close_covers_context(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> bool {
        self.ledger.values().any(|entry| {
            matches!(entry, RetainedRequest::TabClose(record)
                if !record.is_finished() && &record.terminal_context_id == context_id)
        })
    }

    /// Whether an unsettled terminal create/split settlement owes this
    /// Context a member — a Workspace Close fences those pending records
    /// instead of capturing a membership their outcomes would mutate.
    pub(super) fn pending_terminal_covers_context(
        &self,
        context_id: &WorkbenchTerminalContextId,
    ) -> bool {
        self.ledger.values().any(|entry| match entry {
            RetainedRequest::PendingTabCreate(entry) => {
                entry.metadata().terminal_context_id() == context_id
            }
            RetainedRequest::PendingPanelCreate(entry) => {
                entry.metadata().terminal_context_id() == context_id
            }
            _ => false,
        })
    }

    pub(super) fn next_revision(&self) -> Result<WorkbenchRevision, PlanFailure> {
        let credits = self.outstanding_close_credits()?;
        self.projection
            .snapshot()
            .revision
            .value()
            .checked_add(1)
            .filter(|revision| revision.checked_add(credits).is_some())
            .map(WorkbenchRevision::new)
            .ok_or(PlanFailure::Rejected(
                orchbee_workbench_protocol::WorkbenchRejectionReason::RevisionExhausted,
            ))
    }
}

#[cfg(test)]
mod tests;
