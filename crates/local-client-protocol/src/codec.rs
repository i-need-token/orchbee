//! Strict OBLC frame encoding and decoding over caller-owned byte streams. Bounds and payload
//! validation belong here; peer authentication, admission policy, deadlines and recovery do not.

use std::{
    fmt,
    io::{self, Read, Write},
    num::NonZeroU64,
};

const FRAME_MAGIC: [u8; 4] = *b"OBLC";
const FRAME_HEADER_BYTES: usize = 8;
const CLIENT_HELLO_KIND: u8 = 1;
const CLIENT_ACCEPTED_KIND: u8 = 2;
const REJECTED_KIND: u8 = 3;

/// Frame version: the `client-server` generation the handshake negotiated. Only generation 1
/// exists, so every frame carries it.
pub const PROTOCOL_VERSION: u16 = 1;
/// Maximum payload length admitted before a reader allocates its payload buffer.
pub const MAXIMUM_FRAME_PAYLOAD_BYTES: usize = 256;

/// Closed refusal reasons after the handshake answer; a refusal grants no Client Connection
/// authority. Version and build compatibility are the handshake's, never an OBLC refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    CapacityExceeded,
    WrongMessage,
    MalformedFrame,
    HandshakeTimeout,
    Stopping,
}

/// One validated admission or lifecycle-control payload, without a Connection state transition.
/// Every message follows a handshake answer on the same connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalClientMessage {
    /// Enters the Client phase.
    ClientHello,
    ClientAccepted {
        generation: NonZeroU64,
    },
    Rejected {
        reason: RejectionReason,
    },
    SessionStopRequest(crate::SessionStopRequest),
    SessionStopResult(crate::SessionStopResult),
}

/// Distinguishes framing, version, length and payload rejection from transport failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidMagic,
    IncompatibleVersion { received: u16 },
    PayloadTooLarge { actual: usize, maximum: usize },
    InvalidPayload,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMagic => formatter.write_str("invalid local Client frame magic"),
            Self::IncompatibleVersion { received } => {
                write!(
                    formatter,
                    "incompatible local Client protocol version {received}"
                )
            }
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "local Client frame payload has {actual} bytes; maximum is {maximum}"
            ),
            Self::InvalidPayload => formatter.write_str("invalid local Client frame payload"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// A failed read preserves whether byte transport or protocol admission rejected the frame.
#[derive(Debug)]
pub enum ReadMessageError {
    Io(io::Error),
    Protocol(ProtocolError),
}

impl fmt::Display for ReadMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReadMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

impl From<io::Error> for ReadMessageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for ReadMessageError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// A failed write preserves protocol rejection or the underlying write/flush failure.
#[derive(Debug)]
pub enum WriteMessageError {
    Io(io::Error),
    Protocol(ProtocolError),
}

impl fmt::Display for WriteMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WriteMessageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

impl From<io::Error> for WriteMessageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ProtocolError> for WriteMessageError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// Reads and validates exactly one bounded frame, leaving any following bytes unread.
/// The caller supplies timeouts and decides whether a failed stream can be retained.
///
/// # Errors
/// Returns [`ReadMessageError::Io`] for incomplete or failed reads, or
/// [`ReadMessageError::Protocol`] for a rejected header or payload. Failure may consume bytes.
pub fn read_message(reader: &mut impl Read) -> Result<LocalClientMessage, ReadMessageError> {
    let mut header = [0_u8; FRAME_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if header.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice()) {
        return Err(ProtocolError::InvalidMagic.into());
    }

    let version = u16::from_be_bytes([header[4], header[5]]);
    if version != PROTOCOL_VERSION {
        return Err(ProtocolError::IncompatibleVersion { received: version }.into());
    }
    let payload_length = usize::from(u16::from_be_bytes([header[6], header[7]]));
    if payload_length > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload_length,
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        }
        .into());
    }

    let mut payload = vec![0_u8; payload_length];
    reader.read_exact(&mut payload)?;
    decode_payload(&payload).map_err(Into::into)
}

/// Validates and encodes one complete frame before writing it, then flushes the caller's stream.
///
/// # Errors
/// Returns [`WriteMessageError::Protocol`] before writing for an invalid message, or
/// [`WriteMessageError::Io`] if writing or flushing fails. An I/O error may follow partial delivery
/// and does not authorize replay.
pub fn write_message(
    writer: &mut impl Write,
    message: &LocalClientMessage,
) -> Result<(), WriteMessageError> {
    let payload = encode_payload(message)?;
    let payload_length =
        u16::try_from(payload.len()).map_err(|_error| ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        })?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    frame.extend_from_slice(&payload_length.to_be_bytes());
    frame.extend_from_slice(&payload);
    writer.write_all(&frame)?;
    writer.flush().map_err(Into::into)
}

fn encode_payload(message: &LocalClientMessage) -> Result<Vec<u8>, ProtocolError> {
    let payload = match message {
        LocalClientMessage::SessionStopRequest(request) => {
            crate::session_stop::encode_request(request)
        }
        LocalClientMessage::SessionStopResult(result) => crate::session_stop::encode_result(result),
        LocalClientMessage::ClientHello => vec![CLIENT_HELLO_KIND],
        LocalClientMessage::ClientAccepted { generation } => {
            let mut payload = Vec::with_capacity(9);
            payload.push(CLIENT_ACCEPTED_KIND);
            payload.extend_from_slice(&generation.get().to_be_bytes());
            payload
        }
        LocalClientMessage::Rejected { reason } => vec![REJECTED_KIND, encode_reason(*reason)],
    };
    if payload.len() > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        });
    }
    Ok(payload)
}

fn decode_payload(payload: &[u8]) -> Result<LocalClientMessage, ProtocolError> {
    match payload.first().copied() {
        Some(4) => {
            crate::session_stop::decode_request(payload).map(LocalClientMessage::SessionStopRequest)
        }
        Some(5) => {
            crate::session_stop::decode_result(payload).map(LocalClientMessage::SessionStopResult)
        }
        Some(CLIENT_HELLO_KIND) if payload.len() == 1 => Ok(LocalClientMessage::ClientHello),
        Some(CLIENT_ACCEPTED_KIND) => decode_client_accepted(payload),
        Some(REJECTED_KIND) => decode_rejected(payload),
        Some(_) | None => Err(ProtocolError::InvalidPayload),
    }
}

fn decode_client_accepted(payload: &[u8]) -> Result<LocalClientMessage, ProtocolError> {
    let encoded_generation: [u8; 8] = payload
        .get(1..)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let generation = NonZeroU64::new(u64::from_be_bytes(encoded_generation))
        .ok_or(ProtocolError::InvalidPayload)?;
    Ok(LocalClientMessage::ClientAccepted { generation })
}

fn decode_rejected(payload: &[u8]) -> Result<LocalClientMessage, ProtocolError> {
    if payload.len() != 2 {
        return Err(ProtocolError::InvalidPayload);
    }
    let encoded_reason = payload
        .get(1)
        .copied()
        .ok_or(ProtocolError::InvalidPayload)?;
    let reason = decode_reason(encoded_reason).ok_or(ProtocolError::InvalidPayload)?;
    Ok(LocalClientMessage::Rejected { reason })
}

const fn encode_reason(reason: RejectionReason) -> u8 {
    match reason {
        RejectionReason::CapacityExceeded => 3,
        RejectionReason::WrongMessage => 4,
        RejectionReason::MalformedFrame => 5,
        RejectionReason::HandshakeTimeout => 6,
        RejectionReason::Stopping => 7,
    }
}

const fn decode_reason(encoded: u8) -> Option<RejectionReason> {
    match encoded {
        3 => Some(RejectionReason::CapacityExceeded),
        4 => Some(RejectionReason::WrongMessage),
        5 => Some(RejectionReason::MalformedFrame),
        6 => Some(RejectionReason::HandshakeTimeout),
        7 => Some(RejectionReason::Stopping),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
