//! Encodes and decodes Agent Drain requests and bounded event or gap pages with exact sequence
//! and watermark fields. Queue eviction, acknowledgement advancement, and Agent interpretation
//! remain with runtime owners.

use std::num::NonZeroU64;

use super::{PayloadReader, decode_host_identity, encode_host_identity};
use crate::{
    AGENT_DRAIN_REPLY_KIND, AGENT_DRAIN_REQUEST_KIND, AgentDrainEntry, AgentDrainReply,
    AgentDrainRequest, MAXIMUM_REPLY_ENTRIES, MAXIMUM_SIGNAL_PAYLOAD_BYTES, ProtocolError,
    REPLY_ENTRY_BYTES, WorkerGeneration, WorkerId,
};

const EVENT_TAG: u8 = 1;
const GAP_TAG: u8 = 2;

pub(super) fn encode_request(request: &AgentDrainRequest) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(crate::REQUEST_PAYLOAD_BYTES);
    bytes.push(AGENT_DRAIN_REQUEST_KIND);
    bytes.extend(request.worker_id.as_bytes());
    bytes.extend_from_slice(&request.worker_generation.get().to_be_bytes());
    bytes.extend_from_slice(&request.token.get().to_be_bytes());
    bytes.extend_from_slice(&request.acknowledged.to_be_bytes());
    bytes
}

pub(super) fn decode_request(bytes: &[u8]) -> Result<AgentDrainRequest, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, AGENT_DRAIN_REQUEST_KIND)?;
    let request = AgentDrainRequest {
        worker_id: WorkerId::from_bytes(reader.read_array()?)?,
        worker_generation: WorkerGeneration::new(
            NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        ),
        token: NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?,
        acknowledged: reader.read_u64()?,
    };
    reader.finish()?;
    Ok(request)
}

/// Reply validity is a construction invariant of `AgentDrainReply`; the
/// encoder re-checks the bounds anyway because the public entry enum is
/// shared with the producing queue.
pub(super) fn encode_reply(reply: &AgentDrainReply) -> Result<Vec<u8>, ProtocolError> {
    let mut bytes = Vec::with_capacity(crate::REPLY_HEADER_BYTES + REPLY_ENTRY_BYTES);
    bytes.push(AGENT_DRAIN_REPLY_KIND);
    bytes.extend_from_slice(&reply.token_echo().get().to_be_bytes());
    bytes.extend_from_slice(&reply.acknowledged_echo().to_be_bytes());
    bytes.extend_from_slice(&reply.queue_tail().to_be_bytes());
    let count =
        u16::try_from(reply.entries().len()).map_err(|_error| ProtocolError::PayloadTooLarge {
            actual: reply.entries().len(),
            maximum: MAXIMUM_REPLY_ENTRIES,
        })?;
    bytes.extend_from_slice(&count.to_be_bytes());
    let mut entries_bytes = 0_usize;
    for entry in reply.entries() {
        match entry {
            AgentDrainEntry::Event {
                sequence,
                identity,
                payload,
            } => {
                bytes.push(EVENT_TAG);
                bytes.extend_from_slice(&sequence.get().to_be_bytes());
                encode_host_identity(&mut bytes, *identity);
                let length =
                    u16::try_from(payload.len()).map_err(|_error| ProtocolError::InvalidPayload)?;
                bytes.extend_from_slice(&length.to_be_bytes());
                bytes.extend_from_slice(payload.as_bytes());
            }
            AgentDrainEntry::Gap { low, high } => {
                bytes.push(GAP_TAG);
                bytes.extend_from_slice(&low.get().to_be_bytes());
                bytes.extend_from_slice(&high.to_be_bytes());
            }
        }
        entries_bytes = entries_bytes
            .checked_add(entry.encoded_bytes())
            .ok_or(ProtocolError::InvalidPayload)?;
    }
    if entries_bytes > REPLY_ENTRY_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: entries_bytes,
            maximum: REPLY_ENTRY_BYTES,
        });
    }
    Ok(bytes)
}

/// Structural checks belong to the decoder: the entry count is rejected
/// before any allocation, the page byte budget is enforced while reading,
/// coverage must be contiguous and non-overlapping within the page, every
/// entry ends at or below the queue tail, and the frame must finish exactly.
/// Connection, token echo, acknowledgment echo and the watermark fold are the
/// consumer's responsibility, so a stale first entry is legal here.
pub(super) fn decode_reply(bytes: &[u8]) -> Result<AgentDrainReply, ProtocolError> {
    let mut reader = PayloadReader::after_kind(bytes, AGENT_DRAIN_REPLY_KIND)?;
    let token_echo = NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
    let acknowledged_echo = reader.read_u64()?;
    let queue_tail = reader.read_u64()?;
    let count = usize::from(reader.read_u16()?);
    if count > MAXIMUM_REPLY_ENTRIES {
        return Err(ProtocolError::InvalidPayload);
    }
    let mut entries = Vec::with_capacity(count);
    let mut entries_bytes = 0_usize;
    let mut covered_through: Option<u64> = None;
    for _ in 0..count {
        let entry = match reader.read_u8()? {
            EVENT_TAG => {
                let sequence =
                    NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
                let identity = decode_host_identity(&mut reader)?;
                let length = usize::from(reader.read_u16()?);
                if length > MAXIMUM_SIGNAL_PAYLOAD_BYTES {
                    return Err(ProtocolError::InvalidPayload);
                }
                let payload = std::str::from_utf8(reader.read_slice(length)?)
                    .map_err(|_error| ProtocolError::InvalidPayload)?
                    .to_owned();
                AgentDrainEntry::Event {
                    sequence,
                    identity,
                    payload,
                }
            }
            GAP_TAG => {
                let low =
                    NonZeroU64::new(reader.read_u64()?).ok_or(ProtocolError::InvalidPayload)?;
                let high = reader.read_u64()?;
                if low.get() > high {
                    return Err(ProtocolError::InvalidPayload);
                }
                AgentDrainEntry::Gap { low, high }
            }
            _ => return Err(ProtocolError::InvalidPayload),
        };
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
        entries_bytes = entries_bytes
            .checked_add(entry.encoded_bytes())
            .filter(|bytes| *bytes <= REPLY_ENTRY_BYTES)
            .ok_or(ProtocolError::InvalidPayload)?;
        covered_through = Some(entry.end());
        entries.push(entry);
    }
    reader.finish()?;
    AgentDrainReply::new(token_echo, acknowledged_echo, queue_tail, entries)
}
