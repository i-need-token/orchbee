//! Explicit managed-Worker transport transfer, independent of normal enrollment.
use crate::{OperationId, ProtocolError, WorkerGeneration, WorkerHandoffId, WorkerId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A current Server's explicit delegation, not ordinary cohort enrollment.
pub struct WorkerHandoffTarget {
    pub handoff_id: WorkerHandoffId,
    pub cohort_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Prepare a standby, commit its transport ownership, or cancel it before commit.
pub enum WorkerHandoffAction {
    Prepare,
    Commit,
    Abort,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One exact current-Worker operation; every result echoes all fields.
pub struct WorkerHandoffRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub target: WorkerHandoffTarget,
    pub action: WorkerHandoffAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact ticket and owner echoed by the standby handshake and ownership receipt.
pub struct WorkerHandoffPeer {
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub target: WorkerHandoffTarget,
}

impl WorkerHandoffRequest {
    /// The owner and ticket seal shared by Hello, Ready and Owned.
    #[must_use]
    pub const fn peer(self) -> WorkerHandoffPeer {
        WorkerHandoffPeer {
            worker_id: self.worker_id,
            worker_generation: self.worker_generation,
            target: self.target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Closed precommit refusals without diagnostic or executable-path disclosure.
pub enum WorkerHandoffRefusal {
    WrongWorker,
    Unmanaged,
    Busy,
    TicketMismatch,
    NotPrepared,
    ReusedTicket,
    Capacity,
    TargetUnavailable,
    Authentication,
    AttachmentsBusy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// A known transport phase; Committed is not the final public Server result.
pub enum WorkerHandoffOutcome {
    Prepared,
    Committed,
    Aborted,
    Refused(WorkerHandoffRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// An action-consistent receipt; construction excludes contradictory success phases.
pub struct WorkerHandoffResult {
    request: WorkerHandoffRequest,
    outcome: WorkerHandoffOutcome,
}

impl WorkerHandoffResult {
    /// Refuses a success outcome belonging to a different action.
    pub fn new(
        request: WorkerHandoffRequest,
        outcome: WorkerHandoffOutcome,
    ) -> Result<Self, ProtocolError> {
        if !matches!(
            (request.action, outcome),
            (_, WorkerHandoffOutcome::Refused(_))
                | (WorkerHandoffAction::Prepare, WorkerHandoffOutcome::Prepared)
                | (WorkerHandoffAction::Commit, WorkerHandoffOutcome::Committed)
                | (WorkerHandoffAction::Abort, WorkerHandoffOutcome::Aborted)
        ) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self { request, outcome })
    }

    #[must_use]
    /// The complete original operation, including the handoff target.
    pub const fn request(self) -> WorkerHandoffRequest {
        self.request
    }

    #[must_use]
    /// The known transport phase or precommit refusal.
    pub const fn outcome(self) -> WorkerHandoffOutcome {
        self.outcome
    }
}

#[cfg(test)]
mod tests;
