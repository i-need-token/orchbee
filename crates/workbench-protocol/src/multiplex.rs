//! OBWP v1 connection-local streams. Inner records retain their own semantic codecs.
use std::{
    io::{self, Read, Write},
    num::NonZeroU64,
};

use crate::WORKBENCH_MAGIC;

pub const MULTIPLEX_OPEN_KIND: u16 = 64;
pub const MAXIMUM_MULTIPLEX_STREAMS: usize = 64;
pub const MAXIMUM_MULTIPLEX_LANES: usize = 64;
pub const MAXIMUM_MULTIPLEX_BYTES: usize = 32 * 1024 * 1024;
pub const MAXIMUM_STREAM_RECORD_BYTES: usize = 24 * 1024 * 1024 + 12;
const HEADER_BYTES: usize = 12;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MultiplexStreamId(NonZeroU64);
impl MultiplexStreamId {
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
    pub const fn value(self) -> u64 {
        self.0.get()
    }
}

/// A logical resource owner inside one admitted carrier; never an admitted Client identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MultiplexLaneId(NonZeroU64);
impl MultiplexLaneId {
    pub const ROOT: Self = Self(NonZeroU64::MIN);
    pub const fn new(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
    pub const fn value(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamMode {
    Request,
    State,
    Events,
    /// An initial semantic snapshot, followed by coalesced commit revision hints.
    Invalidations,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamFrameKind {
    Result,
    Snapshot,
    Update,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamCloseReason {
    Complete,
    Cancelled,
    Unsupported,
    Capacity,
    InvalidRequest,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiplexDisconnectReason {
    SlowConsumer,
    Capacity,
    InvalidStream,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiplexMessage {
    StreamOpen {
        stream_id: MultiplexStreamId,
        lane_id: MultiplexLaneId,
        mode: StreamMode,
        request: Vec<u8>,
    },
    StreamFrame {
        stream_id: MultiplexStreamId,
        sequence: NonZeroU64,
        kind: StreamFrameKind,
        record: Vec<u8>,
    },
    StreamClose {
        stream_id: MultiplexStreamId,
        reason: StreamCloseReason,
    },
    Gap {
        stream_id: MultiplexStreamId,
        through: NonZeroU64,
    },
    /// Discards lost updates and captures a fresh snapshot for an existing subscription.
    Resnapshot {
        stream_id: MultiplexStreamId,
    },
    LaneOpen {
        lane_id: MultiplexLaneId,
    },
    LaneClose {
        lane_id: MultiplexLaneId,
    },
    Disconnect {
        reason: MultiplexDisconnectReason,
    },
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid OBWP stream record")
}

pub fn encode_multiplex_message(message: &MultiplexMessage) -> io::Result<Vec<u8>> {
    let (kind, length) = match message {
        MultiplexMessage::StreamOpen { request, .. } => {
            if request.is_empty() || request.len() > MAXIMUM_STREAM_RECORD_BYTES {
                return Err(invalid());
            }
            (64u16, 17 + request.len())
        }
        MultiplexMessage::StreamFrame { record, .. } => {
            if record.is_empty() || record.len() > MAXIMUM_STREAM_RECORD_BYTES {
                return Err(invalid());
            }
            (65, 17 + record.len())
        }
        MultiplexMessage::StreamClose { .. } => (66, 9),
        MultiplexMessage::Gap { .. } => (67, 16),
        MultiplexMessage::Disconnect { .. } => (68, 1),
        MultiplexMessage::Resnapshot { .. } => (69, 8),
        MultiplexMessage::LaneOpen { .. } => (70, 8),
        MultiplexMessage::LaneClose { .. } => (71, 8),
    };
    let mut bytes = Vec::with_capacity(HEADER_BYTES + length);
    bytes.extend_from_slice(&WORKBENCH_MAGIC);
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&kind.to_be_bytes());
    bytes.extend_from_slice(&u32::try_from(length).map_err(|_| invalid())?.to_be_bytes());
    match message {
        MultiplexMessage::StreamOpen {
            stream_id,
            lane_id,
            mode,
            request,
        } => {
            bytes.extend_from_slice(&stream_id.value().to_be_bytes());
            bytes.extend_from_slice(&lane_id.value().to_be_bytes());
            bytes.push(match mode {
                StreamMode::Request => 0,
                StreamMode::State => 1,
                StreamMode::Events => 2,
                StreamMode::Invalidations => 3,
            });
            bytes.extend_from_slice(request);
        }
        MultiplexMessage::StreamFrame {
            stream_id,
            sequence,
            kind,
            record,
        } => {
            bytes.extend_from_slice(&stream_id.value().to_be_bytes());
            bytes.extend_from_slice(&sequence.get().to_be_bytes());
            bytes.push(match kind {
                StreamFrameKind::Result => 0,
                StreamFrameKind::Snapshot => 1,
                StreamFrameKind::Update => 2,
            });
            bytes.extend_from_slice(record);
        }
        MultiplexMessage::StreamClose { stream_id, reason } => {
            bytes.extend_from_slice(&stream_id.value().to_be_bytes());
            bytes.push(match reason {
                StreamCloseReason::Complete => 0,
                StreamCloseReason::Cancelled => 1,
                StreamCloseReason::Unsupported => 2,
                StreamCloseReason::Capacity => 3,
                StreamCloseReason::InvalidRequest => 4,
            });
        }
        MultiplexMessage::Gap { stream_id, through } => {
            bytes.extend_from_slice(&stream_id.value().to_be_bytes());
            bytes.extend_from_slice(&through.get().to_be_bytes());
        }
        MultiplexMessage::Disconnect { reason } => bytes.push(match reason {
            MultiplexDisconnectReason::SlowConsumer => 0,
            MultiplexDisconnectReason::Capacity => 1,
            MultiplexDisconnectReason::InvalidStream => 2,
            MultiplexDisconnectReason::Unavailable => 3,
        }),
        MultiplexMessage::Resnapshot { stream_id } => {
            bytes.extend_from_slice(&stream_id.value().to_be_bytes())
        }
        MultiplexMessage::LaneOpen { lane_id } | MultiplexMessage::LaneClose { lane_id } => {
            bytes.extend_from_slice(&lane_id.value().to_be_bytes())
        }
    }
    Ok(bytes)
}

pub fn write_multiplex_message(
    writer: &mut impl Write,
    message: &MultiplexMessage,
) -> io::Result<()> {
    writer.write_all(&encode_multiplex_message(message)?)?;
    writer.flush()
}

fn payload_length(header: &[u8]) -> io::Result<usize> {
    if header.len() != HEADER_BYTES
        || header[..4] != WORKBENCH_MAGIC
        || header[4..6] != 1u16.to_be_bytes()
    {
        return Err(invalid());
    }
    let kind = u16::from_be_bytes([header[6], header[7]]);
    let length = u32::from_be_bytes(header[8..12].try_into().map_err(|_| invalid())?) as usize;
    let valid = match kind {
        64 => (18..=MAXIMUM_STREAM_RECORD_BYTES + 17).contains(&length),
        65 => (18..=MAXIMUM_STREAM_RECORD_BYTES + 17).contains(&length),
        66 => length == 9,
        67 => length == 16,
        68 => length == 1,
        69..=71 => length == 8,
        _ => false,
    };
    if valid { Ok(length) } else { Err(invalid()) }
}

pub fn read_multiplex_message(reader: &mut impl Read) -> io::Result<MultiplexMessage> {
    let mut header = [0; HEADER_BYTES];
    reader.read_exact(&mut header)?;
    let length = payload_length(&header)?;
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    decode(u16::from_be_bytes([header[6], header[7]]), &payload)
}

fn nonzero(bytes: &[u8]) -> io::Result<NonZeroU64> {
    NonZeroU64::new(u64::from_be_bytes(bytes.try_into().map_err(|_| invalid())?))
        .ok_or_else(invalid)
}
fn decode(kind: u16, payload: &[u8]) -> io::Result<MultiplexMessage> {
    if kind == 68 {
        return Ok(MultiplexMessage::Disconnect {
            reason: match payload[0] {
                0 => MultiplexDisconnectReason::SlowConsumer,
                1 => MultiplexDisconnectReason::Capacity,
                2 => MultiplexDisconnectReason::InvalidStream,
                3 => MultiplexDisconnectReason::Unavailable,
                _ => return Err(invalid()),
            },
        });
    }
    let stream_id = MultiplexStreamId(nonzero(&payload[..8])?);
    Ok(match kind {
        70 => MultiplexMessage::LaneOpen {
            lane_id: MultiplexLaneId(nonzero(payload)?),
        },
        71 => MultiplexMessage::LaneClose {
            lane_id: MultiplexLaneId(nonzero(payload)?),
        },
        69 => MultiplexMessage::Resnapshot { stream_id },
        64 => MultiplexMessage::StreamOpen {
            stream_id,
            lane_id: MultiplexLaneId(nonzero(&payload[8..16])?),
            mode: match payload[16] {
                0 => StreamMode::Request,
                1 => StreamMode::State,
                2 => StreamMode::Events,
                3 => StreamMode::Invalidations,
                _ => return Err(invalid()),
            },
            request: payload[17..].to_vec(),
        },
        65 => MultiplexMessage::StreamFrame {
            stream_id,
            sequence: nonzero(&payload[8..16])?,
            kind: match payload[16] {
                0 => StreamFrameKind::Result,
                1 => StreamFrameKind::Snapshot,
                2 => StreamFrameKind::Update,
                _ => return Err(invalid()),
            },
            record: payload[17..].to_vec(),
        },
        66 => MultiplexMessage::StreamClose {
            stream_id,
            reason: match payload[8] {
                0 => StreamCloseReason::Complete,
                1 => StreamCloseReason::Cancelled,
                2 => StreamCloseReason::Unsupported,
                3 => StreamCloseReason::Capacity,
                4 => StreamCloseReason::InvalidRequest,
                _ => return Err(invalid()),
            },
        },
        67 => MultiplexMessage::Gap {
            stream_id,
            through: nonzero(&payload[8..16])?,
        },
        _ => return Err(invalid()),
    })
}

/// Partial network reads never block the connection actor's command/cancellation queue.
#[derive(Default, Debug)]
pub struct MultiplexDecoder {
    bytes: Vec<u8>,
}
impl MultiplexDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        // A read may contain the end of one maximum record and the next header/record.
        if self.bytes.len().saturating_add(bytes.len()) > MAXIMUM_MULTIPLEX_BYTES {
            return Err(invalid());
        }
        self.bytes.extend_from_slice(bytes);
        if self.bytes.len() >= HEADER_BYTES {
            payload_length(&self.bytes[..HEADER_BYTES])?;
        }
        Ok(())
    }
    pub fn next_message(&mut self) -> io::Result<Option<MultiplexMessage>> {
        if self.bytes.len() < HEADER_BYTES {
            return Ok(None);
        }
        let length = HEADER_BYTES + payload_length(&self.bytes[..HEADER_BYTES])?;
        if self.bytes.len() < length {
            return Ok(None);
        }
        let kind = u16::from_be_bytes([self.bytes[6], self.bytes[7]]);
        let message = decode(kind, &self.bytes[HEADER_BYTES..length])?;
        self.bytes.drain(..length);
        Ok(Some(message))
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests;
