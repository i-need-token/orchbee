use std::time::Instant;

use orchbee_capability_node_protocol::{
    HostTerminalIdentity, TerminalFence, TerminalId, TerminalLifecycleRequest, WorkerGeneration,
    WorkerId,
};
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseResume, WorkbenchTabCloseResumeResult,
    WorkbenchTabCloseStatus, WorkbenchTerminalContextId,
};

use super::{PlanFailure, RetainedRequest, WorkbenchAuthority, WorkbenchTerminalTab};

mod admission;
mod continuation;
mod control;
mod coordination;
mod publication;
mod reservation;

pub use admission::{
    PreparedWorkbenchTabClose, WorkbenchTabCloseAdmission, WorkbenchTabCloseBegin,
    WorkbenchTabClosePreparation, WorkbenchTabClosePrimaryAdmission, WorkbenchTabCloseResolution,
    WorkbenchTabCloseRuntimeUnavailableReason,
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
};
pub use publication::{PreparedWorkbenchTabClosePublication, WorkbenchTabClosePublicationError};

/// Exact retained runtime association supplied by the Site/Terminal owner under its gate.
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseTarget {
    terminal_context_id: WorkbenchTerminalContextId,
    member: WorkbenchTerminalTab,
    runtime: WorkbenchTabCloseRuntimeTarget,
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
    pub const fn runtime(&self) -> WorkbenchTabCloseRuntimeTarget {
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

pub(super) struct CloseRecord {
    pub(super) command: WorkbenchCommand,
    pub(super) result: WorkbenchCommandResult,
    target: WorkbenchTabCloseTarget,
    additional_targets: Vec<WorkbenchTabCloseTarget>,
    generation: WorkbenchTabCloseAttemptGeneration,
    resume_receipt: Option<AcceptedResumeReceipt>,
    reserved_bytes: usize,
    continuation: CloseContinuation,
    additional_continuations: Vec<CloseContinuation>,
    scheduler_cursor: usize,
    closed_revision: Option<WorkbenchRevision>,
}

struct AcceptedResumeReceipt {
    request: WorkbenchTabCloseResume,
    result: WorkbenchTabCloseResumeResult,
}

impl CloseRecord {
    fn is_closed(&self) -> bool {
        self.closed_revision.is_some()
    }

    fn target_count(&self) -> usize {
        1 + self.additional_targets.len()
    }

    fn target(&self, index: usize) -> Option<&WorkbenchTabCloseTarget> {
        if index == 0 {
            Some(&self.target)
        } else {
            self.additional_targets.get(index - 1)
        }
    }

    fn continuation(&self, index: usize) -> Option<&CloseContinuation> {
        if index == 0 {
            Some(&self.continuation)
        } else {
            self.additional_continuations.get(index - 1)
        }
    }

    fn continuation_mut(&mut self, index: usize) -> Option<&mut CloseContinuation> {
        if index == 0 {
            Some(&mut self.continuation)
        } else {
            self.additional_continuations.get_mut(index - 1)
        }
    }

    fn target_index_for_binding(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<usize> {
        (0..self.target_count()).find(|index| {
            self.target(*index)
                .is_some_and(|target| target.member.binding() == binding)
        })
    }

    fn target_index_for_runtime(&self, runtime: WorkbenchTabCloseRuntimeTarget) -> Option<usize> {
        (0..self.target_count()).find(|index| {
            self.target(*index)
                .is_some_and(|target| target.runtime == runtime)
        })
    }

    fn target_index_for_request(&self, request: &TerminalLifecycleRequest) -> Option<usize> {
        (0..self.target_count()).find(|index| {
            self.target(*index)
                .is_some_and(|target| target.runtime.matches_request(request))
        })
    }

    fn members(&self) -> impl Iterator<Item = &WorkbenchTerminalTab> {
        std::iter::once(&self.target.member)
            .chain(self.additional_targets.iter().map(|target| &target.member))
    }

    fn targets(&self) -> impl Iterator<Item = &WorkbenchTabCloseTarget> {
        std::iter::once(&self.target).chain(self.additional_targets.iter())
    }

    fn target_for_binding(
        &self,
        binding: &orchbee_workbench_protocol::PresentationBinding,
    ) -> Option<&WorkbenchTabCloseTarget> {
        self.target_index_for_binding(binding)
            .and_then(|index| self.target(index))
    }

    fn matches_context_tab(
        &self,
        context_id: &WorkbenchTerminalContextId,
        tab_id: &orchbee_workbench_protocol::WorkbenchTabId,
    ) -> bool {
        self.targets().all(|target| {
            target.terminal_context_id() == context_id && target.member().tab_id() == tab_id
        })
    }

    fn matches_members(&self, members: &[WorkbenchTerminalTab]) -> bool {
        self.members().eq(members.iter())
    }

    fn status(&self) -> WorkbenchTabCloseStatus {
        if let Some(revision) = self.closed_revision {
            return WorkbenchTabCloseStatus::Closed { revision };
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
        statuses
            .into_iter()
            .next()
            .unwrap_or(WorkbenchTabCloseStatus::Closed {
                revision: WorkbenchRevision::new(0),
            })
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
        !self.has_unknown()
            && !self.has_outstanding()
            && (0..self.target_count()).any(|index| {
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
        if prepared.is_empty() {
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
    }

    fn generation_exhausted(&mut self) {
        for index in 0..self.target_count() {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.generation_exhausted();
            }
        }
    }

    fn next_step(&mut self, now: Instant) -> WorkbenchTabCloseStepTarget {
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

    fn commit_closed(&mut self, revision: WorkbenchRevision) {
        self.closed_revision = Some(revision);
        for index in 0..self.target_count() {
            if let Some(continuation) = self.continuation_mut(index) {
                continuation.commit_closed(revision);
            }
        }
    }

    fn publication_ready(&self) -> bool {
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
            | RetainedRequest::PendingPanelSplit(_) => None,
        }
    }

    fn close_record_mut(&mut self, request_id: &WorkbenchRequestId) -> Option<&mut CloseRecord> {
        match self.ledger.get_mut(request_id)? {
            RetainedRequest::TabClose(record) => Some(record),
            RetainedRequest::Settled(_)
            | RetainedRequest::PendingTabCreate(_)
            | RetainedRequest::PendingPanelSplit(_) => None,
        }
    }

    fn outstanding_close_credits(&self) -> Result<u64, PlanFailure> {
        // The shared ledger's 4096-entry bound also bounds all Close owner/tombstone scans.
        let count = self
            .ledger
            .values()
            .filter(
                |entry| matches!(entry, RetainedRequest::TabClose(record) if !record.is_closed()),
            )
            .count();
        u64::try_from(count).map_err(|_error| PlanFailure::Internal)
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
