//! Per-connection Agent Drain queue.
//!
//! One `AgentDrain` exists for the lifetime of an admitted Worker-to-Server
//! connection and is created before any Terminal Host starts. Both the eager
//! and the lazy path hand a shared `Rc<RefCell<AgentDrain>>` handle to the
//! Host broker so `HostBrokerConnection::receive_event` can append
//! `TerminalApplicationSignal` events for terminals with no viewer attached.
//! The serialized operations loop drains the same state on kind-17 requests;
//! `offered_hi` only advances after a captured page has been completely
//! written to the Server socket.
//!
//! Coverage is the contiguous interval `confirmed_hi + 1 ..= minted_hi`: every
//! sequence in it is either a queued EVENT or accounted by a leading GAP that
//! records evicted coverage. A GAP consumes both an entry slot and its encoded
//! bytes inside the same two bounds as real entries.

use std::{collections::VecDeque, io, num::NonZeroU64};

use orchbee_capability_node_protocol::{
    AgentDrainEntry, AgentDrainReply, GAP_ENTRY_BYTES, HostTerminalIdentity,
    MAXIMUM_SIGNAL_PAYLOAD_BYTES, REPLY_ENTRY_BYTES,
};

pub(crate) const MAXIMUM_QUEUED_ENTRIES: usize = 1024;
pub(crate) const MAXIMUM_QUEUED_ENTRY_BYTES: usize = 4 * 1024 * 1024;

pub(crate) struct AgentDrain {
    entries: VecDeque<AgentDrainEntry>,
    entry_bytes: usize,
    minted_hi: u64,
    offered_hi: u64,
    confirmed_hi: u64,
}

impl AgentDrain {
    /// A fresh queue starts empty with every watermark at zero.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            entry_bytes: 0,
            minted_hi: 0,
            offered_hi: 0,
            confirmed_hi: 0,
        }
    }

    /// Mint the next queue sequence for an admitted Host application signal.
    /// The Host-side signal sequence is intentionally not carried over; drain
    /// sequence space is owned by this queue. Sequence exhaustion and an
    /// over-bound payload end the connection epoch.
    pub(crate) fn append(
        &mut self,
        identity: HostTerminalIdentity,
        payload: String,
    ) -> io::Result<()> {
        if payload.len() > MAXIMUM_SIGNAL_PAYLOAD_BYTES {
            return Err(io::Error::other(
                "agent drain signal payload exceeds its wire bound",
            ));
        }
        let sequence = self
            .minted_hi
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| io::Error::other("agent drain sequence space exhausted"))?;
        let entry = AgentDrainEntry::Event {
            sequence,
            identity,
            payload,
        };
        self.entry_bytes = self
            .entry_bytes
            .checked_add(entry.encoded_bytes())
            .ok_or_else(|| io::Error::other("agent drain byte accounting overflow"))?;
        self.entries.push_back(entry);
        self.minted_hi = sequence.get();
        self.evict_until_fit();
        Ok(())
    }

    /// Validate the acknowledgment against the offered watermark, trim
    /// confirmed coverage, then capture the longest contiguous page that fits
    /// the reply entry budget. Returns the reply and the highest covered
    /// sequence; `None` means the page is empty and nothing is committable.
    ///
    /// `ack > offered_hi` is a protocol violation: the caller must end the
    /// connection rather than answer. `ack <= confirmed_hi` is a legal stale
    /// acknowledgment and still produces a fresh page.
    pub(crate) fn respond(
        &mut self,
        token: NonZeroU64,
        acknowledged: u64,
    ) -> io::Result<(AgentDrainReply, Option<u64>)> {
        if acknowledged > self.offered_hi {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "agent drain acknowledgment exceeds the offered watermark",
            ));
        }
        if acknowledged > self.confirmed_hi {
            self.confirmed_hi = acknowledged;
            self.trim_confirmed();
        }
        let mut page = Vec::new();
        let mut page_bytes = 0_usize;
        let mut covered_hi = None;
        for entry in &self.entries {
            let Some(total) = page_bytes
                .checked_add(entry.encoded_bytes())
                .filter(|total| *total <= REPLY_ENTRY_BYTES)
            else {
                break;
            };
            page_bytes = total;
            covered_hi = Some(entry.end());
            page.push(entry.clone());
        }
        let reply = AgentDrainReply::new(token, acknowledged, self.minted_hi, page)
            .map_err(io::Error::other)?;
        Ok((reply, covered_hi))
    }

    /// Commit the captured watermark after the page was completely written.
    /// Called only on a successful `send`; a failed write leaves the offered
    /// watermark untouched so evicted coverage is never reported as offered.
    pub(crate) fn commit_offered(&mut self, covered_hi: u64) {
        self.offered_hi = self.offered_hi.max(covered_hi);
    }

    /// Evict the oldest unconfirmed prefix until the queue fits both bounds.
    /// The evicted range merges into one leading GAP which itself consumes a
    /// slot and its bytes, so the fit check accounts for it before it exists.
    fn evict_until_fit(&mut self) {
        let mut evicted_low = None;
        let mut evicted_high = 0_u64;
        loop {
            let gap_entries = usize::from(evicted_low.is_some());
            let gap_bytes = if evicted_low.is_some() {
                GAP_ENTRY_BYTES
            } else {
                0
            };
            if self.entries.len() + gap_entries <= MAXIMUM_QUEUED_ENTRIES
                && self.entry_bytes + gap_bytes <= MAXIMUM_QUEUED_ENTRY_BYTES
            {
                break;
            }
            match self.entries.pop_front() {
                Some(AgentDrainEntry::Event {
                    sequence, payload, ..
                }) => {
                    self.entry_bytes -= entry_cost(payload.len());
                    if evicted_low.is_none() {
                        evicted_low = Some(sequence.get());
                    }
                    evicted_high = sequence.get();
                }
                Some(AgentDrainEntry::Gap { low, high }) => {
                    self.entry_bytes -= GAP_ENTRY_BYTES;
                    if evicted_low.is_none() {
                        evicted_low = Some(low.get());
                    }
                    evicted_high = high;
                }
                None => break,
            }
        }
        if let Some(low) = evicted_low {
            self.entries.push_front(AgentDrainEntry::Gap {
                low: NonZeroU64::new(low).expect("coverage starts positive"),
                high: evicted_high,
            });
            self.entry_bytes += GAP_ENTRY_BYTES;
        }
    }

    /// Drop entries fully below the confirmed watermark and clip a partially
    /// confirmed leading GAP.
    fn trim_confirmed(&mut self) {
        while self
            .entries
            .front()
            .is_some_and(|entry| entry.end() <= self.confirmed_hi)
        {
            match self.entries.pop_front() {
                Some(AgentDrainEntry::Event { payload, .. }) => {
                    self.entry_bytes -= entry_cost(payload.len());
                }
                Some(AgentDrainEntry::Gap { .. }) => {
                    self.entry_bytes -= GAP_ENTRY_BYTES;
                }
                None => break,
            }
        }
        if let Some(AgentDrainEntry::Gap { low, high }) = self.entries.front_mut()
            && low.get() <= self.confirmed_hi
            && *high > self.confirmed_hi
        {
            *low = NonZeroU64::new(self.confirmed_hi + 1)
                .expect("confirmed watermark plus one is positive");
        }
    }

    /// Test-only state seeding: puts the mint watermark at `value` so the
    /// sequence-exhaustion boundary is reachable without 2^64 admissions.
    #[cfg(test)]
    pub(crate) fn seed_minted_for_test(&mut self, value: u64) {
        self.minted_hi = value;
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> (Vec<AgentDrainEntry>, u64, u64, u64) {
        (
            self.entries.iter().cloned().collect(),
            self.minted_hi,
            self.offered_hi,
            self.confirmed_hi,
        )
    }
}

const fn entry_cost(payload_bytes: usize) -> usize {
    orchbee_capability_node_protocol::EVENT_ENTRY_OVERHEAD_BYTES + payload_bytes
}

impl Default for AgentDrain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
