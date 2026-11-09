//! Managed lifetime shutdown is separate from per-terminal lifecycle grants.
use crate::{HostEpoch, OperationId, ProtocolError, WorkerGeneration, WorkerId};

/// Cooperative per-attempt cleanup budget, bounded to 1..=[`ShutdownBudget::MAXIMUM_MILLISECONDS`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShutdownBudget(u32);

impl ShutdownBudget {
    /// The longest budget a shutdown may carry: one whole Server stop attempt.
    pub const MAXIMUM_MILLISECONDS: u32 = 60_000;

    /// Rejects zero and budgets beyond the protocol maximum.
    pub fn new(milliseconds: u32) -> Result<Self, ProtocolError> {
        if !(1..=Self::MAXIMUM_MILLISECONDS).contains(&milliseconds) {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self(milliseconds))
    }

    /// Returns the admitted budget without renewing its caller-owned deadline.
    pub const fn milliseconds(self) -> u32 {
        self.0
    }
}

/// What a shutdown does to the Worker's Host and its terminals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownTerminals {
    /// Contain and dispose every terminal, then wait for the owned Host to exit.
    Dispose,
    /// Leave the Host and every terminal running; the Worker only releases its Host connection.
    Keep,
}

/// Explicit shutdown fenced to one admitted Worker generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerShutdownRequest {
    pub operation_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub budget: ShutdownBudget,
    pub terminals: ShutdownTerminals,
}

/// Cleanup stage at which an attempt stopped without confirming completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownStage {
    BackgroundJobs,
    Inventory,
    Force,
    Dispose,
    HostExit,
}

/// Failure facts retained without inferring rollback or absence of effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownFailure {
    Deadline,
    Transport,
    Protocol,
    HostRefused,
    ProcessWait,
}

/// Admission refusal before cleanup effects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownRefusal {
    WrongWorker,
    Unmanaged,
}

/// Stopped proves empty Host inventory and retained Host-child exit, not Worker exit; Released
/// proves only that a `Keep` shutdown closed the Host connection without touching a terminal.
/// Disposal counts include only acknowledged disposals in this attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerShutdownOutcome {
    NoHost,
    Stopped {
        host_epoch: HostEpoch,
        disposed_terminals: u32,
    },
    Refused(ShutdownRefusal),
    Released {
        host_epoch: HostEpoch,
    },
    Incomplete {
        host_epoch: Option<HostEpoch>,
        disposed_terminals: u32,
        stage: ShutdownStage,
        failure: ShutdownFailure,
    },
}

/// Exact request echo and known outcome; there is no implicit replay ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerShutdownResult {
    pub request: WorkerShutdownRequest,
    pub outcome: WorkerShutdownOutcome,
}

#[cfg(test)]
mod tests;
