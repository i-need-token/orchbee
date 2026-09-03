use std::{
    fmt,
    io::{self, Read, Write},
    num::NonZeroU64,
};

const FRAME_MAGIC: [u8; 4] = *b"OBLS";
const FRAME_HEADER_BYTES: usize = 8;
const WORKER_ID_BYTES: usize = 16;
const WORKER_HELLO_KIND: u8 = 1;
const WORKER_ACCEPTED_KIND: u8 = 2;
const REJECTED_KIND: u8 = 3;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAXIMUM_FRAME_PAYLOAD_BYTES: usize = 256;
pub const MAXIMUM_COHORT_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkerId([u8; WORKER_ID_BYTES]);

impl WorkerId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; WORKER_ID_BYTES]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; WORKER_ID_BYTES] {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerHello {
    cohort: String,
    worker_id: WorkerId,
}

impl WorkerHello {
    pub fn new(cohort: &str, worker_id: WorkerId) -> Result<Self, ProtocolError> {
        if cohort.is_empty() || cohort.len() > MAXIMUM_COHORT_BYTES || !cohort.as_bytes().is_ascii()
        {
            return Err(ProtocolError::InvalidCohort);
        }
        Ok(Self {
            cohort: cohort.to_owned(),
            worker_id,
        })
    }

    #[must_use]
    pub fn cohort(&self) -> &str {
        &self.cohort
    }

    #[must_use]
    pub const fn worker_id(&self) -> WorkerId {
        self.worker_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    IncompatibleProtocol,
    IncompatibleCohort,
    DuplicateWorker,
    WrongMessage,
    MalformedFrame,
    HandshakeTimeout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalSiteMessage {
    WorkerHello(WorkerHello),
    WorkerAccepted { generation: NonZeroU64 },
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
            Self::InvalidMagic => formatter.write_str("invalid local Site frame magic"),
            Self::IncompatibleVersion { received } => {
                write!(
                    formatter,
                    "incompatible local Site protocol version {received}"
                )
            }
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "local Site frame payload has {actual} bytes; maximum is {maximum}"
            ),
            Self::InvalidPayload => formatter.write_str("invalid local Site frame payload"),
            Self::InvalidCohort => formatter.write_str("invalid local Site executable cohort"),
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

pub fn read_message(reader: &mut impl Read) -> Result<LocalSiteMessage, ReadMessageError> {
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
    message: &LocalSiteMessage,
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

fn encode_payload(message: &LocalSiteMessage) -> Result<Vec<u8>, ProtocolError> {
    let payload = match message {
        LocalSiteMessage::WorkerHello(hello) => {
            let cohort_length =
                u8::try_from(hello.cohort.len()).map_err(|_error| ProtocolError::InvalidCohort)?;
            let mut payload = Vec::with_capacity(2 + WORKER_ID_BYTES + hello.cohort.len());
            payload.push(WORKER_HELLO_KIND);
            payload.push(cohort_length);
            payload.extend_from_slice(&hello.worker_id.as_bytes());
            payload.extend_from_slice(hello.cohort.as_bytes());
            payload
        }
        LocalSiteMessage::WorkerAccepted { generation } => {
            let mut payload = Vec::with_capacity(9);
            payload.push(WORKER_ACCEPTED_KIND);
            payload.extend_from_slice(&generation.get().to_be_bytes());
            payload
        }
        LocalSiteMessage::Rejected { reason } => vec![REJECTED_KIND, encode_reason(*reason)],
    };
    if payload.len() > MAXIMUM_FRAME_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge {
            actual: payload.len(),
            maximum: MAXIMUM_FRAME_PAYLOAD_BYTES,
        });
    }
    Ok(payload)
}

fn decode_payload(payload: &[u8]) -> Result<LocalSiteMessage, ProtocolError> {
    match payload.first().copied() {
        Some(WORKER_HELLO_KIND) => decode_worker_hello(payload),
        Some(WORKER_ACCEPTED_KIND) => decode_worker_accepted(payload),
        Some(REJECTED_KIND) => decode_rejected(payload),
        Some(_) | None => Err(ProtocolError::InvalidPayload),
    }
}

fn decode_worker_hello(payload: &[u8]) -> Result<LocalSiteMessage, ProtocolError> {
    let cohort_length = usize::from(
        payload
            .get(1)
            .copied()
            .ok_or(ProtocolError::InvalidPayload)?,
    );
    let expected_length = 2 + WORKER_ID_BYTES + cohort_length;
    if payload.len() != expected_length {
        return Err(ProtocolError::InvalidPayload);
    }
    let worker_id_bytes: [u8; WORKER_ID_BYTES] = payload
        .get(2..2 + WORKER_ID_BYTES)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let cohort_bytes = payload
        .get(2 + WORKER_ID_BYTES..)
        .ok_or(ProtocolError::InvalidPayload)?;
    let cohort =
        std::str::from_utf8(cohort_bytes).map_err(|_error| ProtocolError::InvalidCohort)?;
    Ok(LocalSiteMessage::WorkerHello(WorkerHello::new(
        cohort,
        WorkerId::from_bytes(worker_id_bytes),
    )?))
}

fn decode_worker_accepted(payload: &[u8]) -> Result<LocalSiteMessage, ProtocolError> {
    let encoded_generation: [u8; 8] = payload
        .get(1..)
        .ok_or(ProtocolError::InvalidPayload)?
        .try_into()
        .map_err(|_error| ProtocolError::InvalidPayload)?;
    let generation = NonZeroU64::new(u64::from_be_bytes(encoded_generation))
        .ok_or(ProtocolError::InvalidPayload)?;
    Ok(LocalSiteMessage::WorkerAccepted { generation })
}

fn decode_rejected(payload: &[u8]) -> Result<LocalSiteMessage, ProtocolError> {
    if payload.len() != 2 {
        return Err(ProtocolError::InvalidPayload);
    }
    let encoded_reason = payload
        .get(1)
        .copied()
        .ok_or(ProtocolError::InvalidPayload)?;
    let reason = decode_reason(encoded_reason).ok_or(ProtocolError::InvalidPayload)?;
    Ok(LocalSiteMessage::Rejected { reason })
}

const fn encode_reason(reason: RejectionReason) -> u8 {
    match reason {
        RejectionReason::IncompatibleProtocol => 1,
        RejectionReason::IncompatibleCohort => 2,
        RejectionReason::DuplicateWorker => 3,
        RejectionReason::WrongMessage => 4,
        RejectionReason::MalformedFrame => 5,
        RejectionReason::HandshakeTimeout => 6,
    }
}

const fn decode_reason(encoded: u8) -> Option<RejectionReason> {
    match encoded {
        1 => Some(RejectionReason::IncompatibleProtocol),
        2 => Some(RejectionReason::IncompatibleCohort),
        3 => Some(RejectionReason::DuplicateWorker),
        4 => Some(RejectionReason::WrongMessage),
        5 => Some(RejectionReason::MalformedFrame),
        6 => Some(RejectionReason::HandshakeTimeout),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
