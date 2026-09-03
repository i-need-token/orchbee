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

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAXIMUM_FRAME_PAYLOAD_BYTES: usize = 256;
pub const MAXIMUM_COHORT_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientHello {
    cohort: String,
}

impl ClientHello {
    pub fn new(cohort: &str) -> Result<Self, ProtocolError> {
        if cohort.is_empty() || cohort.len() > MAXIMUM_COHORT_BYTES || !cohort.as_bytes().is_ascii()
        {
            return Err(ProtocolError::InvalidCohort);
        }
        Ok(Self {
            cohort: cohort.to_owned(),
        })
    }

    #[must_use]
    pub fn cohort(&self) -> &str {
        &self.cohort
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    IncompatibleProtocol,
    IncompatibleCohort,
    CapacityExceeded,
    WrongMessage,
    MalformedFrame,
    HandshakeTimeout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalClientMessage {
    ClientHello(ClientHello),
    ClientAccepted { generation: NonZeroU64 },
    Rejected { reason: RejectionReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    InvalidMagic,
    IncompatibleVersion { received: u16 },
    PayloadTooLarge { actual: usize, maximum: usize },
    InvalidPayload,
    InvalidCohort,
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
            Self::InvalidCohort => formatter.write_str("invalid local Client executable cohort"),
        }
    }
}

impl std::error::Error for ProtocolError {}

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
        LocalClientMessage::ClientHello(hello) => {
            let cohort_length =
                u8::try_from(hello.cohort.len()).map_err(|_error| ProtocolError::InvalidCohort)?;
            let mut payload = Vec::with_capacity(2 + hello.cohort.len());
            payload.push(CLIENT_HELLO_KIND);
            payload.push(cohort_length);
            payload.extend_from_slice(hello.cohort.as_bytes());
            payload
        }
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
        Some(CLIENT_HELLO_KIND) => decode_client_hello(payload),
        Some(CLIENT_ACCEPTED_KIND) => decode_client_accepted(payload),
        Some(REJECTED_KIND) => decode_rejected(payload),
        Some(_) | None => Err(ProtocolError::InvalidPayload),
    }
}

fn decode_client_hello(payload: &[u8]) -> Result<LocalClientMessage, ProtocolError> {
    let cohort_length = usize::from(
        payload
            .get(1)
            .copied()
            .ok_or(ProtocolError::InvalidPayload)?,
    );
    if payload.len() != 2 + cohort_length {
        return Err(ProtocolError::InvalidPayload);
    }
    let cohort_bytes = payload.get(2..).ok_or(ProtocolError::InvalidPayload)?;
    let cohort =
        std::str::from_utf8(cohort_bytes).map_err(|_error| ProtocolError::InvalidCohort)?;
    Ok(LocalClientMessage::ClientHello(ClientHello::new(cohort)?))
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
        RejectionReason::IncompatibleProtocol => 1,
        RejectionReason::IncompatibleCohort => 2,
        RejectionReason::CapacityExceeded => 3,
        RejectionReason::WrongMessage => 4,
        RejectionReason::MalformedFrame => 5,
        RejectionReason::HandshakeTimeout => 6,
    }
}

const fn decode_reason(encoded: u8) -> Option<RejectionReason> {
    match encoded {
        1 => Some(RejectionReason::IncompatibleProtocol),
        2 => Some(RejectionReason::IncompatibleCohort),
        3 => Some(RejectionReason::CapacityExceeded),
        4 => Some(RejectionReason::WrongMessage),
        5 => Some(RejectionReason::MalformedFrame),
        6 => Some(RejectionReason::HandshakeTimeout),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
