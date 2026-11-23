//! The move turn: the Server's periodic request that lets its Worker move one terminal from an
//! older Terminal Host to its own, and the answer naming what moved. It carries no choice of
//! terminal and no move step; the Worker owns both, and the Server owns the rebinding.

use crate::{HostTerminalIdentity, OperationId, TerminalId, WorkerGeneration, WorkerId};

/// One move turn. Sending it also tells the Worker that the Server recorded every rebinding the
/// Worker answered before it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalMoveTurnRequest {
    pub request_id: OperationId,
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
}

/// What a move turn did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminalMoveTurnOutcome {
    /// The product terminal now runs as `to` in the Worker's own Host; it ran as `from`.
    Moved {
        terminal_id: TerminalId,
        from: HostTerminalIdentity,
        to: HostTerminalIdentity,
    },
    /// An older Host still runs a terminal that may move, none of them due yet.
    Waiting,
    /// No terminal is left that can move.
    Done,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalMoveTurnResult {
    pub request: TerminalMoveTurnRequest,
    pub outcome: TerminalMoveTurnOutcome,
}
