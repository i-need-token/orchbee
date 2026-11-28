//! The move turn: the Server's periodic request that lets its Worker move one terminal from an
//! older Terminal Host to its own, and the answer naming what moved. It carries no choice of
//! terminal and no move step; the Worker owns both, and the Server owns the rebinding.

use std::fmt;

use crate::{
    HostTerminalIdentity, OperationId, ProtocolError, TerminalId, WorkerGeneration, WorkerId,
};

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
    /// A move failed at `step` for `reason`, which stops moves for the rest of the Worker's life:
    /// no later turn moves any, and the terminals left keep running on their Hosts.
    Stopped {
        step: TerminalMoveToken,
        reason: TerminalMoveToken,
    },
}

/// A step or a reason of a failed move as the Worker's `terminal-move-failed` diagnostic names it:
/// 1 to [`TerminalMoveToken::MAXIMUM_BYTES`] bytes of lowercase ASCII letters, digits and `-`.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TerminalMoveToken {
    length: u8,
    bytes: [u8; Self::MAXIMUM_BYTES],
}

impl TerminalMoveToken {
    pub const MAXIMUM_BYTES: usize = 32;

    /// The token `value` names, or `InvalidPayload` outside the token grammar.
    pub fn new(value: &str) -> Result<Self, ProtocolError> {
        let source = value.as_bytes();
        let length = u8::try_from(source.len()).map_err(|_| ProtocolError::InvalidPayload)?;
        if source.is_empty()
            || source.len() > Self::MAXIMUM_BYTES
            || !source
                .iter()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
        {
            return Err(ProtocolError::InvalidPayload);
        }
        let mut bytes = [0; Self::MAXIMUM_BYTES];
        bytes[..source.len()].copy_from_slice(source);
        Ok(Self { length, bytes })
    }

    pub fn as_str(&self) -> &str {
        // The constructor admits ASCII alone.
        std::str::from_utf8(&self.bytes[..usize::from(self.length)]).unwrap_or_default()
    }
}

impl fmt::Debug for TerminalMoveToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalMoveTurnResult {
    pub request: TerminalMoveTurnRequest,
    pub outcome: TerminalMoveTurnOutcome,
}
