//! Defines bounded replay pages of physical terminal application signals and explicit gaps,
//! validating contiguous coverage and queue watermarks. It carries the connection-owned
//! queue's observations without retaining that queue or interpreting Agent sessions.

use std::num::NonZeroU64;

use crate::{HostTerminalIdentity, ProtocolError, WorkerGeneration, WorkerId};

/// Agent Drain is the bounded Server->Worker replay of terminal application
/// signals the Worker observed on its Host connection. The queue is owned by
/// one admitted Worker connection; this module only carries its wire shape.
pub const AGENT_DRAIN_REQUEST_KIND: u8 = 17;
pub const AGENT_DRAIN_REPLY_KIND: u8 = 18;

/// Fixed request body: kind, Worker ID (16), Worker generation, token and the
/// acknowledged watermark (`u64` each).
pub const REQUEST_PAYLOAD_BYTES: usize = 41;
/// Fixed reply header: kind, token echo, acknowledgment echo, queue tail and
/// the entry count (`u16`).
pub const REPLY_HEADER_BYTES: usize = 27;
/// Reply entry bytes share the frame payload with the fixed header.
pub const REPLY_ENTRY_BYTES: usize = 8165;
/// The decoder must reject more entries before allocating.
pub const MAXIMUM_REPLY_ENTRIES: usize = 480;
/// One EVENT entry carries tag, sequence, the 48-byte Host identity, the
/// `u16` payload length and the payload itself.
pub const EVENT_ENTRY_OVERHEAD_BYTES: usize = 59;
/// One GAP entry carries tag plus the inclusive low/high bounds.
pub const GAP_ENTRY_BYTES: usize = 17;
/// One EVENT payload inherits the terminal application signal bound.
pub const MAXIMUM_SIGNAL_PAYLOAD_BYTES: usize = 4096;

/// Server request for the next contiguous drain page. `token` correlates one
/// in-flight page and `acknowledged` is the highest confirmed sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentDrainRequest {
    pub worker_id: WorkerId,
    pub worker_generation: WorkerGeneration,
    pub token: NonZeroU64,
    pub acknowledged: u64,
}

/// One contiguous coverage entry. Coverage is always reported left to right;
/// a GAP accounts for an evicted range the queue can no longer replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentDrainEntry {
    Event {
        sequence: NonZeroU64,
        identity: HostTerminalIdentity,
        payload: String,
    },
    Gap {
        low: NonZeroU64,
        high: u64,
    },
}

impl AgentDrainEntry {
    /// Encoded size as it appears inside a reply page.
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        match self {
            Self::Event { payload, .. } => EVENT_ENTRY_OVERHEAD_BYTES + payload.len(),
            Self::Gap { .. } => GAP_ENTRY_BYTES,
        }
    }

    /// First covered sequence (event sequence or inclusive GAP low bound).
    #[must_use]
    pub fn start(&self) -> u64 {
        match self {
            Self::Event { sequence, .. } => sequence.get(),
            Self::Gap { low, .. } => low.get(),
        }
    }

    /// Last covered sequence (event sequence or inclusive GAP high bound).
    #[must_use]
    pub fn end(&self) -> u64 {
        match self {
            Self::Event { sequence, .. } => sequence.get(),
            Self::Gap { high, .. } => *high,
        }
    }
}

/// Worker reply carrying one contiguous coverage page. The constructor
/// validates the same invariants the decoder enforces so an invalid page is
/// unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentDrainReply {
    token_echo: NonZeroU64,
    acknowledged_echo: u64,
    queue_tail: u64,
    entries: Vec<AgentDrainEntry>,
}

impl AgentDrainReply {
    pub fn new(
        token_echo: NonZeroU64,
        acknowledged_echo: u64,
        queue_tail: u64,
        entries: Vec<AgentDrainEntry>,
    ) -> Result<Self, ProtocolError> {
        validate_page(queue_tail, &entries)?;
        Ok(Self {
            token_echo,
            acknowledged_echo,
            queue_tail,
            entries,
        })
    }

    #[must_use]
    pub const fn token_echo(&self) -> NonZeroU64 {
        self.token_echo
    }

    #[must_use]
    pub const fn acknowledged_echo(&self) -> u64 {
        self.acknowledged_echo
    }

    #[must_use]
    pub const fn queue_tail(&self) -> u64 {
        self.queue_tail
    }

    #[must_use]
    pub fn entries(&self) -> &[AgentDrainEntry] {
        &self.entries
    }
}

/// Page invariants shared by construction and decoding: the count bound, the
/// entry-byte budget, per-entry shape, contiguous non-overlapping coverage and
/// coverage never exceeding the advertised queue tail. A stale or empty page
/// is legal, so the first entry is not compared against the echoed
/// acknowledgment.
fn validate_page(queue_tail: u64, entries: &[AgentDrainEntry]) -> Result<(), ProtocolError> {
    if entries.len() > MAXIMUM_REPLY_ENTRIES {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut bytes = 0_usize;
    let mut covered_through: Option<u64> = None;
    for entry in entries {
        if let Some(previous_end) = covered_through {
            // A successor after u64::MAX cannot exist: no start continues it.
            let expected = previous_end
                .checked_add(1)
                .ok_or(ProtocolError::InvalidPayload)?;
            if entry.start() != expected {
                return Err(ProtocolError::InvalidPayload);
            }
        }
        if entry.end() > queue_tail {
            return Err(ProtocolError::InvalidPayload);
        }
        match entry {
            AgentDrainEntry::Event { payload, .. } => {
                if payload.len() > MAXIMUM_SIGNAL_PAYLOAD_BYTES {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
            AgentDrainEntry::Gap { low, high } => {
                if low.get() > *high {
                    return Err(ProtocolError::InvalidPayload);
                }
            }
        }
        bytes = bytes
            .checked_add(entry.encoded_bytes())
            .filter(|bytes| *bytes <= REPLY_ENTRY_BYTES)
            .ok_or(ProtocolError::InvalidPayload)?;
        covered_through = Some(entry.end());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
