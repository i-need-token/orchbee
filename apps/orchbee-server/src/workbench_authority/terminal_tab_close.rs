use orchbee_capability_node_protocol::{
    HostTerminalIdentity, TerminalFence, TerminalId, WorkerGeneration, WorkerId,
};
use orchbee_workbench_protocol::{
    WorkbenchCommand, WorkbenchCommandResult, WorkbenchRequestId, WorkbenchRevision,
    WorkbenchTabCloseAttemptGeneration, WorkbenchTabCloseResume, WorkbenchTabCloseResumeResult,
    WorkbenchTabCloseStatus, WorkbenchTerminalContextId,
};

use super::{PlanFailure, RetainedRequest, WorkbenchAuthority, WorkbenchStandaloneTab};

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

    fn matches_member(self, member: &WorkbenchStandaloneTab) -> bool {
        u128::from_str_radix(member.binding().worker_id.as_str(), 16)
            .is_ok_and(|value| value.to_be_bytes() == self.worker_id.as_bytes())
            && u128::from_str_radix(member.binding().terminal_id.as_str(), 16)
                .is_ok_and(|value| value.to_be_bytes() == self.terminal_id.as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkbenchTabCloseTarget {
    terminal_context_id: WorkbenchTerminalContextId,
    member: WorkbenchStandaloneTab,
    runtime: WorkbenchTabCloseRuntimeTarget,
}

impl WorkbenchTabCloseTarget {
    #[must_use]
    pub fn terminal_context_id(&self) -> &WorkbenchTerminalContextId {
        &self.terminal_context_id
    }
    #[must_use]
    pub fn member(&self) -> &WorkbenchStandaloneTab {
        &self.member
    }
    #[must_use]
    pub const fn runtime(&self) -> WorkbenchTabCloseRuntimeTarget {
        self.runtime
    }
}

pub(super) struct CloseRecord {
    pub(super) command: WorkbenchCommand,
    pub(super) result: WorkbenchCommandResult,
    target: WorkbenchTabCloseTarget,
    generation: WorkbenchTabCloseAttemptGeneration,
    resume_receipt: Option<AcceptedResumeReceipt>,
    reserved_bytes: usize,
    continuation: CloseContinuation,
}

struct AcceptedResumeReceipt {
    request: WorkbenchTabCloseResume,
    result: WorkbenchTabCloseResumeResult,
}

impl CloseRecord {
    fn is_closed(&self) -> bool {
        matches!(
            self.continuation.status(),
            WorkbenchTabCloseStatus::Closed { .. }
        )
    }
}

impl WorkbenchAuthority {
    fn close_record(&self, request_id: &WorkbenchRequestId) -> Option<&CloseRecord> {
        match self.ledger.get(request_id)? {
            RetainedRequest::TabClose(record) => Some(record),
            RetainedRequest::Settled(_) | RetainedRequest::PendingTabCreate(_) => None,
        }
    }

    fn close_record_mut(&mut self, request_id: &WorkbenchRequestId) -> Option<&mut CloseRecord> {
        match self.ledger.get_mut(request_id)? {
            RetainedRequest::TabClose(record) => Some(record),
            RetainedRequest::Settled(_) | RetainedRequest::PendingTabCreate(_) => None,
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
