//! The `OBHS` frame — the bytes `OBHS`, a big-endian `u32` payload length from 1 to 1,048,576, and
//! one strict UTF-8 JSON object — and the direction-specific message codec. Deadlines, transports,
//! and what a peer does with a decoded message stay with the caller.

use std::{
    io::{self, Read},
    num::NonZeroU32,
    time::Duration,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::{
    AcceptorMessage, Answer, Build, BuildEnvironment, CapabilitySet, ConnectionEntry, ContractName,
    ControlRequest, DecodeError, DrainResult, EncodeError, EnvironmentName, ExitResult, Extensions,
    Fingerprint, Generation, GenerationRange, InitiatorMessage, Offer, ProcessEntry, ProcessRole,
    ProcessState, ProductVersion, QueryResult, ReadError, Rejection, RejectionReason, RequestName,
    UnknownRequest, Unsupported, identity::present, message::MessageType, name::is_name,
    strict_json,
};

/// The four bytes that open every frame.
pub const FRAME_MAGIC: [u8; 4] = *b"OBHS";
/// The magic and the payload length.
pub const FRAME_HEADER_BYTES: usize = 8;
/// The largest payload a frame carries.
pub const MAXIMUM_PAYLOAD_BYTES: usize = 1_048_576;
/// How long an acceptor waits for a candidate's complete frame before a best-effort `timeout`
/// rejection.
pub const CANDIDATE_DEADLINE: Duration = Duration::from_secs(2);

/// Encodes one frame an initiator sends.
///
/// # Errors
///
/// Returns [`EncodeError::PayloadTooLarge`] when the payload exceeds [`MAXIMUM_PAYLOAD_BYTES`].
pub fn encode_initiator_frame(message: &InitiatorMessage) -> Result<Vec<u8>, EncodeError> {
    match message {
        InitiatorMessage::Offer(offer) => frame(&WireOffer::from(offer)),
        InitiatorMessage::Request(request) => frame(&WireRequest {
            kind: request.kind.as_str().to_owned(),
            environment: request.environment.clone(),
            extensions: request.extensions.clone(),
        }),
        InitiatorMessage::UnknownRequest(request) => frame(&WireRequest {
            kind: request.request().as_str().to_owned(),
            environment: request.environment().clone(),
            extensions: Extensions::new(),
        }),
    }
}

/// Encodes one frame an acceptor sends.
///
/// # Errors
///
/// Returns [`EncodeError::PayloadTooLarge`] when the payload exceeds [`MAXIMUM_PAYLOAD_BYTES`].
pub fn encode_acceptor_frame(message: &AcceptorMessage) -> Result<Vec<u8>, EncodeError> {
    match message {
        AcceptorMessage::Answer(answer) => frame(&WireAnswer::from(answer)),
        AcceptorMessage::Rejection(rejection) => frame(&WireRejection::from(rejection)),
        AcceptorMessage::QueryResult(result) => frame(&WireQueryResult::from(result)),
        AcceptorMessage::DrainResult(result) => frame(&WireDrainResult {
            kind: MessageType::DrainResult.as_str().to_owned(),
            state: result.state,
            extensions: result.extensions.clone(),
        }),
        AcceptorMessage::ExitResult(result) => frame(&WireExitResult {
            kind: MessageType::ExitResult.as_str().to_owned(),
            extensions: result.extensions.clone(),
        }),
        AcceptorMessage::Unsupported(reply) => frame(&WireUnsupported {
            kind: MessageType::Unsupported.as_str().to_owned(),
            request: reply.request.clone(),
            extensions: reply.extensions.clone(),
        }),
    }
}

/// Decodes exactly one complete in-memory frame an initiator sends.
///
/// # Errors
///
/// Returns the first [`DecodeError`] the frame and then its payload violate.
pub fn decode_initiator_frame(frame: &[u8]) -> Result<InitiatorMessage, DecodeError> {
    decode_initiator_payload(complete_payload(frame)?)
}

/// Decodes exactly one complete in-memory frame an acceptor sends.
///
/// # Errors
///
/// Returns the first [`DecodeError`] the frame and then its payload violate.
pub fn decode_acceptor_frame(frame: &[u8]) -> Result<AcceptorMessage, DecodeError> {
    decode_acceptor_payload(complete_payload(frame)?)
}

/// Reads exactly one frame an initiator sends, leaving any later bytes unread. It reports a
/// foreign protocol as soon as a received byte differs from `OBHS` and refuses an out-of-bounds
/// length before reading or allocating the payload.
///
/// # Errors
///
/// Returns [`ReadError`] for a stream failure, an early end, foreign bytes, or a malformed frame.
pub fn read_initiator_message(reader: &mut impl Read) -> Result<InitiatorMessage, ReadError> {
    decode_initiator_payload(&read_payload(reader)?).map_err(ReadError::Malformed)
}

/// Reads exactly one frame an acceptor sends, as [`read_initiator_message`] does.
///
/// # Errors
///
/// Returns [`ReadError`] for a stream failure, an early end, foreign bytes — from a peer that
/// predates the handshake — or a malformed frame.
pub fn read_acceptor_message(reader: &mut impl Read) -> Result<AcceptorMessage, ReadError> {
    decode_acceptor_payload(&read_payload(reader)?).map_err(ReadError::Malformed)
}

fn frame(message: &impl Serialize) -> Result<Vec<u8>, EncodeError> {
    let payload = serde_json::to_vec(message)
        .expect("wire structs serialize: every map key is a string and every number is finite");
    if !(1..=MAXIMUM_PAYLOAD_BYTES).contains(&payload.len()) {
        return Err(EncodeError::PayloadTooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| EncodeError::PayloadTooLarge)?;
    let mut frame = Vec::with_capacity(FRAME_HEADER_BYTES + payload.len());
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn payload_length(header: [u8; 4]) -> Option<usize> {
    usize::try_from(u32::from_be_bytes(header))
        .ok()
        .filter(|length| (1..=MAXIMUM_PAYLOAD_BYTES).contains(length))
}

fn complete_payload(frame: &[u8]) -> Result<&[u8], DecodeError> {
    let magic = frame.get(..FRAME_MAGIC.len()).unwrap_or(frame);
    if FRAME_MAGIC.get(..magic.len()) != Some(magic) {
        return Err(DecodeError::Magic);
    }
    let (Some(header), Some(payload)) = (
        frame
            .get(FRAME_MAGIC.len()..FRAME_HEADER_BYTES)
            .and_then(|length| <[u8; 4]>::try_from(length).ok()),
        frame.get(FRAME_HEADER_BYTES..),
    ) else {
        return Err(DecodeError::Truncated);
    };
    let length = payload_length(header).ok_or(DecodeError::Length)?;
    match payload.len().cmp(&length) {
        std::cmp::Ordering::Less => Err(DecodeError::Truncated),
        std::cmp::Ordering::Greater => Err(DecodeError::TrailingBytes),
        std::cmp::Ordering::Equal => Ok(payload),
    }
}

fn read_payload(reader: &mut impl Read) -> Result<Vec<u8>, ReadError> {
    let mut header = [0; FRAME_HEADER_BYTES];
    let mut filled = 0;
    while let Some(unfilled) = header
        .get_mut(filled..)
        .filter(|unfilled| !unfilled.is_empty())
    {
        let received = read_some(reader, unfilled)?;
        filled += received;
        let magic = header
            .get(..filled.min(FRAME_MAGIC.len()))
            .unwrap_or_default();
        if FRAME_MAGIC.get(..magic.len()) != Some(magic) {
            return Err(ReadError::ForeignProtocol);
        }
    }
    let [_, _, _, _, length @ ..] = header;
    let length = payload_length(length).ok_or(ReadError::Malformed(DecodeError::Length))?;
    let mut payload = vec![0; length];
    let mut filled = 0;
    while let Some(unfilled) = payload
        .get_mut(filled..)
        .filter(|unfilled| !unfilled.is_empty())
    {
        filled += read_some(reader, unfilled)?;
    }
    Ok(payload)
}

/// Reads at least one byte into `buffer`, retrying interruptions.
fn read_some(reader: &mut impl Read, buffer: &mut [u8]) -> Result<usize, ReadError> {
    loop {
        match reader.read(buffer) {
            Ok(0) => return Err(ReadError::Closed),
            Ok(received) => return Ok(received.min(buffer.len())),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(ReadError::Io(error)),
        }
    }
}

fn decode_initiator_payload(payload: &[u8]) -> Result<InitiatorMessage, DecodeError> {
    let (name, object) = message_object(payload)?;
    match MessageType::from_name(&name) {
        Some(MessageType::Offer) => typed::<WireOffer>(object)?
            .try_into()
            .map(InitiatorMessage::Offer),
        Some(MessageType::Request(kind)) => {
            let request = typed::<WireRequest>(object)?;
            Ok(InitiatorMessage::Request(ControlRequest {
                kind,
                environment: request.environment,
                extensions: request.extensions,
            }))
        }
        Some(_) => Err(DecodeError::Message),
        None => {
            let request = RequestName::new(name).map_err(|_| DecodeError::Message)?;
            let environment = object
                .get("environment")
                .and_then(Value::as_str)
                .and_then(|environment| EnvironmentName::new(environment).ok())
                .ok_or(DecodeError::Message)?;
            UnknownRequest::new(request, environment)
                .map(InitiatorMessage::UnknownRequest)
                .map_err(|_| DecodeError::Message)
        }
    }
}

fn decode_acceptor_payload(payload: &[u8]) -> Result<AcceptorMessage, DecodeError> {
    let (name, object) = message_object(payload)?;
    match MessageType::from_name(&name).ok_or(DecodeError::Message)? {
        MessageType::Answer => Ok(AcceptorMessage::Answer(typed::<WireAnswer>(object)?.into())),
        MessageType::Rejection => Ok(AcceptorMessage::Rejection(
            typed::<WireRejection>(object)?.into(),
        )),
        MessageType::QueryResult => typed::<WireQueryResult>(object)?
            .try_into()
            .map(AcceptorMessage::QueryResult),
        MessageType::DrainResult => {
            let result = typed::<WireDrainResult>(object)?;
            Ok(AcceptorMessage::DrainResult(DrainResult {
                state: result.state,
                extensions: result.extensions,
            }))
        }
        MessageType::ExitResult => Ok(AcceptorMessage::ExitResult(ExitResult {
            extensions: typed::<WireExitResult>(object)?.extensions,
        })),
        MessageType::Unsupported => {
            let reply = typed::<WireUnsupported>(object)?;
            Ok(AcceptorMessage::Unsupported(Unsupported {
                request: reply.request,
                extensions: reply.extensions,
            }))
        }
        MessageType::Offer | MessageType::Request(_) => Err(DecodeError::Message),
    }
}

/// Parses the strict object and its grammatical `type`.
fn message_object(payload: &[u8]) -> Result<(String, Map<String, Value>), DecodeError> {
    let object = strict_json::parse_strict_object(payload).ok_or(DecodeError::Encoding)?;
    match object.get("type") {
        Some(Value::String(name)) if is_name(name) => Ok((name.clone(), object)),
        _ => Err(DecodeError::Message),
    }
}

fn typed<Wire: DeserializeOwned>(object: Map<String, Value>) -> Result<Wire, DecodeError> {
    serde_json::from_value(Value::Object(object)).map_err(|_| DecodeError::Message)
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireOffer {
    #[serde(rename = "type")]
    kind: String,
    connection: ContractName,
    generations: GenerationRange,
    capabilities: CapabilitySet,
    product_version: ProductVersion,
    build: Build,
    environment: EnvironmentName,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    fingerprint: Option<Fingerprint>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

impl From<&Offer> for WireOffer {
    fn from(offer: &Offer) -> Self {
        Self {
            kind: MessageType::Offer.as_str().to_owned(),
            connection: offer.connection.clone(),
            generations: offer.generations,
            capabilities: offer.capabilities.clone(),
            product_version: offer.product_version.clone(),
            build: offer.build.clone(),
            environment: offer.environment.name().clone(),
            fingerprint: offer.environment.fingerprint().copied(),
            extensions: offer.extensions.clone(),
        }
    }
}

impl TryFrom<WireOffer> for Offer {
    type Error = DecodeError;

    fn try_from(offer: WireOffer) -> Result<Self, Self::Error> {
        Ok(Self {
            connection: offer.connection,
            generations: offer.generations,
            capabilities: offer.capabilities,
            product_version: offer.product_version,
            build: offer.build,
            environment: BuildEnvironment::from_wire(offer.environment, offer.fingerprint)
                .map_err(|_| DecodeError::Message)?,
            extensions: offer.extensions,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRequest {
    #[serde(rename = "type")]
    kind: String,
    environment: EnvironmentName,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireAnswer {
    #[serde(rename = "type")]
    kind: String,
    generation: Generation,
    capabilities: CapabilitySet,
    product_version: ProductVersion,
    build: Build,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

impl From<&Answer> for WireAnswer {
    fn from(answer: &Answer) -> Self {
        Self {
            kind: MessageType::Answer.as_str().to_owned(),
            generation: answer.generation,
            capabilities: answer.capabilities.clone(),
            product_version: answer.product_version.clone(),
            build: answer.build.clone(),
            extensions: answer.extensions.clone(),
        }
    }
}

impl From<WireAnswer> for Answer {
    fn from(answer: WireAnswer) -> Self {
        Self {
            generation: answer.generation,
            capabilities: answer.capabilities,
            product_version: answer.product_version,
            build: answer.build,
            extensions: answer.extensions,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireRejection {
    #[serde(rename = "type")]
    kind: String,
    reason: RejectionReason,
    generations: GenerationRange,
    product_version: ProductVersion,
    environment: EnvironmentName,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

impl From<&Rejection> for WireRejection {
    fn from(rejection: &Rejection) -> Self {
        Self {
            kind: MessageType::Rejection.as_str().to_owned(),
            reason: rejection.reason,
            generations: rejection.generations,
            product_version: rejection.product_version.clone(),
            environment: rejection.environment.clone(),
            extensions: rejection.extensions.clone(),
        }
    }
}

impl From<WireRejection> for Rejection {
    fn from(rejection: WireRejection) -> Self {
        Self {
            reason: rejection.reason,
            generations: rejection.generations,
            product_version: rejection.product_version,
            environment: rejection.environment,
            extensions: rejection.extensions,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireQueryResult {
    #[serde(rename = "type")]
    kind: String,
    processes: Vec<WireProcess>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    restart_keeps_terminals: Option<bool>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireProcess {
    role: ProcessRole,
    product_version: ProductVersion,
    build: Build,
    environment: EnvironmentName,
    pid: NonZeroU32,
    state: ProcessState,
    terminals: u32,
    connections: Vec<WireConnection>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireConnection {
    kind: ContractName,
    generations: GenerationRange,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    capabilities: Option<CapabilitySet>,
}

impl From<&QueryResult> for WireQueryResult {
    fn from(result: &QueryResult) -> Self {
        Self {
            kind: MessageType::QueryResult.as_str().to_owned(),
            processes: result
                .processes()
                .iter()
                .map(|process| WireProcess {
                    role: process.role,
                    product_version: process.product_version.clone(),
                    build: process.build.clone(),
                    environment: process.environment.clone(),
                    pid: process.pid,
                    state: process.state,
                    terminals: process.terminals,
                    connections: process
                        .connections
                        .iter()
                        .map(|connection| WireConnection {
                            kind: connection.kind.clone(),
                            generations: connection.generations,
                            capabilities: connection.capabilities.clone(),
                        })
                        .collect(),
                })
                .collect(),
            restart_keeps_terminals: result.restart_keeps_terminals(),
            extensions: result.extensions().clone(),
        }
    }
}

impl TryFrom<WireQueryResult> for QueryResult {
    type Error = DecodeError;

    fn try_from(result: WireQueryResult) -> Result<Self, Self::Error> {
        let processes = result
            .processes
            .into_iter()
            .map(|process| ProcessEntry {
                role: process.role,
                product_version: process.product_version,
                build: process.build,
                environment: process.environment,
                pid: process.pid,
                state: process.state,
                terminals: process.terminals,
                connections: process
                    .connections
                    .into_iter()
                    .map(|connection| ConnectionEntry {
                        kind: connection.kind,
                        generations: connection.generations,
                        capabilities: connection.capabilities,
                    })
                    .collect(),
            })
            .collect();
        Self::new(processes, result.restart_keeps_terminals, result.extensions)
            .map_err(|_| DecodeError::Message)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireDrainResult {
    #[serde(rename = "type")]
    kind: String,
    state: ProcessState,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireExitResult {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireUnsupported {
    #[serde(rename = "type")]
    kind: String,
    request: RequestName,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    extensions: Extensions,
}
